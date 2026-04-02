use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::model::application::{Command, Interaction};
use serenity::prelude::*;
use std::env;
use moka::future::Cache;
use std::time::Duration;
use orichalcum::{AsyncFlow, Executable, AsyncNode};
use serde_json::{json, Value as JsonValue};
use crate::gemini::UrielClassification;
use std::collections::HashMap;

mod vault_io;
mod media_handler;
mod gemini;
mod orichalcum_integration;
mod indexer;

struct Handler {
    thread_cache: Cache<u64, Vec<Message>>,
    entity_cache: Cache<String, Vec<String>>,
}

impl Handler {
    fn new(entity_cache: Cache<String, Vec<String>>) -> Self {
        let cache = Cache::builder()
            .time_to_live(Duration::from_secs(60 * 60))
            .build();
        Self {
            thread_cache: cache,
            entity_cache,
        }
    }
}

const DROPBOX_CHANNEL_ID: u64 = 1111111111111111111; // Placeholder
const DISCUSS_CHANNEL_ID: u64 = 2222222222222222222; // Placeholder
const DIGEST_CHANNEL_ID: u64 = 3333333333333333333; // Placeholder

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        let channel_id = msg.channel_id.get();

        let mut is_dropbox = false;

        if channel_id == DROPBOX_CHANNEL_ID {
            is_dropbox = true;
        } else {
            // Check if it's a thread under DROPBOX_CHANNEL_ID
            if let Ok(channel) = msg.channel_id.to_channel(&ctx).await {
                if let Some(guild_channel) = channel.guild() {
                    if let Some(parent_id) = guild_channel.parent_id {
                        if parent_id.get() == DROPBOX_CHANNEL_ID {
                            is_dropbox = true;
                        }
                    }
                }
            }
        }

        if !is_dropbox && channel_id != DISCUSS_CHANNEL_ID && channel_id != DIGEST_CHANNEL_ID {
            return;
        }

        if msg.author.bot {
            return;
        }

        if is_dropbox {
            let mut final_content = msg.content.clone();

            // Thread cache logic
            if let Ok(channel) = msg.channel_id.to_channel(&ctx).await {
                if let Some(guild_channel) = channel.guild() {
                    if guild_channel.thread_metadata.is_some() {
                        let thread_id = channel_id;

                        let mut messages = if let Some(cached_msgs) = self.thread_cache.get(&thread_id).await {
                            cached_msgs
                        } else {
                            match msg.channel_id.messages(&ctx, serenity::builder::GetMessages::new().limit(5)).await {
                                Ok(msgs) => msgs,
                                Err(_) => Vec::new(),
                            }
                        };

                        // Keep last 5 messages logic, but append current message
                        if messages.len() >= 5 {
                            messages.truncate(4);
                        }
                        messages.insert(0, msg.clone()); // newest message first

                        self.thread_cache.insert(thread_id, messages.clone()).await;

                        // Append context if available
                        if !messages.is_empty() {
                            final_content = format!("Context: {:?}\nMessage: {}", messages.iter().map(|m| m.content.clone()).collect::<Vec<_>>(), final_content);
                        }
                    }
                }
            }

            let vault_path = env::var("VAULT_PATH").unwrap_or_else(|_| ".".to_string());

            let mut file_paths = Vec::new();

            for attachment in msg.attachments.iter() {
                match media_handler::route_to_vault(&attachment.url, &vault_path, &attachment.filename).await {
                    Ok(path) => {
                        file_paths.push(path);
                    },
                    Err(e) => {
                        println!("Failed to download attachment: {:?}", e);
                    }
                }
            }

            let entities = self.entity_cache.get(&"entities".to_string()).await.unwrap_or_default();

            // Orichalcum orchestration
            let mut initial_state: HashMap<String, JsonValue> = HashMap::new();
            initial_state.insert("input_text".to_string(), json!(final_content));
            initial_state.insert("entities".to_string(), json!(entities));
            if let Some(path) = file_paths.first() {
                initial_state.insert("media_path".to_string(), json!(path));
            }

            let ingestion_node = orichalcum_integration::GeminiIngestionNode;
            let mut node = AsyncNode::new(ingestion_node);
            node.data.params = initial_state;

            let flow = AsyncFlow::new(Executable::Async(node));

            let mut shared_state = HashMap::new();
            flow.run(&mut shared_state).await;

            let mut success = false;

            if let Some(result_json) = shared_state.get("result") {
                if let Ok(classification) = serde_json::from_value::<UrielClassification>(result_json.clone()) {
                    if matches!(classification.intent, gemini::IntentEnum::Disambiguate) {
                        // Halt writing, create thread to ask for clarification
                        if let Err(e) = msg.channel_id.create_thread_from_message(&ctx.http, msg.id, serenity::builder::CreateThread::new("Disambiguation Needed")).await {
                            println!("Failed to create disambiguation thread: {:?}", e);
                        } else {
                            // Optionally send a message in the thread
                        }
                        success = true; // Count as processed
                    } else {
                        let mut log_content = classification.formatted_content.clone();

                        // Auto-Backlinker logic
                        for entity in classification.entities_found.iter() {
                            let escaped_entity = regex::escape(entity);
                            let pattern = format!(r"\b{}\b", escaped_entity);
                            if let Ok(re) = regex::Regex::new(&pattern) {
                                let replacement = format!("[[{}]]", entity);
                                log_content = re.replace_all(&log_content, replacement.as_str()).to_string();
                            }
                        }

                        for file_path in &file_paths {
                            let file_name = std::path::Path::new(file_path).file_name().unwrap_or_default().to_string_lossy();
                            log_content.push_str(&format!("\n![[{}]]", file_name));
                        }

                        let today = chrono::Local::now().naive_local().date();
                        if let Err(e) = vault_io::append_log(&log_content, today) {
                            println!("Failed to append to log: {:?}", e);
                        } else {
                            success = true;
                        }
                    }
                } else {
                     println!("Failed to deserialize UrielClassification from result: {:?}", result_json);
                }
            } else {
                 println!("Gemini logic execution returned no result.");
            }

            if !success {
                println!("Falling back to raw append due to failure.");
                let mut fallback_content = final_content.clone();
                for file_path in &file_paths {
                    let file_name = std::path::Path::new(file_path).file_name().unwrap_or_default().to_string_lossy();
                    fallback_content.push_str(&format!("\n![[{}]]", file_name));
                }
                let today = chrono::Local::now().naive_local().date();
                if let Err(e) = vault_io::append_log(&fallback_content, today) {
                    println!("Critical failure: Failed to append raw content to log: {:?}", e);
                }
            }
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);

        use serenity::builder::CreateCommand;
        let commands = vec![
            CreateCommand::new("note").description("Create a note"),
            CreateCommand::new("draft").description("Create a draft"),
            CreateCommand::new("event").description("Create an event")
        ];

        Command::set_global_commands(&ctx.http, commands).await.expect("Failed to create global commands");
    }

    async fn interaction_create(&self, _ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = interaction {
            match command.data.name.as_str() {
                "note" => {},
                "draft" => {},
                "event" => {},
                _ => {},
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;

    let entity_cache: Cache<String, Vec<String>> = Cache::builder()
        .time_to_live(Duration::from_secs(60 * 5))
        .build();

    let entity_cache_clone = entity_cache.clone();
    tokio::spawn(async move {
        loop {
            let vault_path = env::var("VAULT_PATH").unwrap_or_else(|_| ".".to_string());
            let entities = indexer::scan_vault(&vault_path);
            entity_cache_clone.insert("entities".to_string(), entities).await;
            tokio::time::sleep(Duration::from_secs(5 * 60)).await;
        }
    });

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler::new(entity_cache))
        .await
        .expect("Err creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
