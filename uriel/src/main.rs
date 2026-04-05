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
mod rag;

use std::sync::Arc;
use tokio::sync::RwLock;

struct Handler {
    thread_cache: Cache<u64, Vec<Message>>,
    entity_cache: Arc<RwLock<Vec<String>>>,
}

impl Handler {
    fn new(entity_cache: Arc<RwLock<Vec<String>>>) -> Self {
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
        let mut is_discuss = false;

        if channel_id == DROPBOX_CHANNEL_ID {
            is_dropbox = true;
        } else if channel_id == DISCUSS_CHANNEL_ID {
            is_discuss = true;
        } else {
            // Check if it's a thread under DROPBOX_CHANNEL_ID or DISCUSS_CHANNEL_ID
            if let Ok(channel) = msg.channel_id.to_channel(&ctx).await {
                if let Some(guild_channel) = channel.guild() {
                    if let Some(parent_id) = guild_channel.parent_id {
                        if parent_id.get() == DROPBOX_CHANNEL_ID {
                            is_dropbox = true;
                        } else if parent_id.get() == DISCUSS_CHANNEL_ID {
                            is_discuss = true;
                        }
                    }
                }
            }
        }

        if !is_dropbox && !is_discuss && channel_id != DIGEST_CHANNEL_ID {
            return;
        }

        if msg.author.bot {
            return;
        }

        if is_dropbox || is_discuss {
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

            // Orichalcum orchestration
            let entities = self.entity_cache.read().await.clone();
            let mut initial_state: HashMap<String, JsonValue> = HashMap::new();
            initial_state.insert("input_text".to_string(), json!(final_content));
            initial_state.insert("known_entities".to_string(), json!(entities));
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
                    use crate::gemini::IntentEnum;
                    if matches!(classification.intent, IntentEnum::Disambiguate) {
                        println!("Intent is Disambiguate, halting write and asking for clarification.");
                        let builder = serenity::builder::CreateThread::new("Clarification Needed");
                        match msg.channel_id.create_thread_from_message(&ctx.http, msg.id, builder).await {
                            Ok(thread_channel) => {
                                let _ = thread_channel.send_message(&ctx.http, serenity::builder::CreateMessage::new().content("Could you clarify which entity you mean?")).await;
                                success = true; // Handled successfully as an interruption
                            },
                            Err(e) => {
                                println!("Failed to create disambiguation thread: {:?}", e);
                            }
                        }
                    } else if matches!(classification.intent, IntentEnum::Query) {
                        println!("Intent is Query, initiating RAG pipeline.");
                        let search_term = &classification.formatted_content;
                        let search_results = rag::search_vault(search_term, &vault_path).await;

                        let prompt = format!(
                            "User asked: {}\n\nSearch Results:\n{}\n\nSynthesize a helpful answer based on the search results. If the results are empty or irrelevant, say so.",
                            final_content, search_results
                        );

                        // Fallback to basic API call if Orichalcum doesn't support complex conversational chains easily yet
                        if let Ok(api_key) = env::var("GEMINI_API_KEY") {
                            let client = reqwest::Client::new();
                            let url = format!("https://generativelanguage.googleapis.com/v1beta/models/gemini-3.1-flash-lite-preview:generateContent?key={}", api_key);
                            let payload = serde_json::json!({
                                "contents": [{
                                    "parts": [{"text": prompt}]
                                }]
                            });

                            match client.post(&url).json(&payload).send().await {
                                Ok(response) => {
                                    if let Ok(body) = response.json::<serde_json::Value>().await {
                                        if let Some(generated_text) = body.get("candidates")
                                            .and_then(|c| c.get(0))
                                            .and_then(|c| c.get("content"))
                                            .and_then(|c| c.get("parts"))
                                            .and_then(|p| p.get(0))
                                            .and_then(|p| p.get("text"))
                                            .and_then(|t| t.as_str()) {

                                            // Send answer back to Discord
                                            let _ = msg.channel_id.say(&ctx.http, generated_text).await;
                                            success = true;
                                        }
                                    }
                                }
                                Err(e) => println!("Failed to query Gemini for answer: {:?}", e),
                            }
                        } else {
                            println!("GEMINI_API_KEY missing for Query intent.");
                        }
                    } else if matches!(classification.intent, IntentEnum::Crawl) {
                        println!("Intent is Crawl, initiating obsidian connection crawling.");
                        let file_name = &classification.formatted_content;
                        let crawl_results = rag::crawl_connections(file_name, &vault_path).await;

                        let prompt = format!(
                            "User asked to explore connections for: {}\n\nConnection Crawl Results:\n{}\n\nSynthesize a helpful answer based on these connections. Describe what this note links to and what links back to it.",
                            final_content, crawl_results
                        );

                        if let Ok(api_key) = env::var("GEMINI_API_KEY") {
                            let client = reqwest::Client::new();
                            let url = format!("https://generativelanguage.googleapis.com/v1beta/models/gemini-3.1-flash-lite-preview:generateContent?key={}", api_key);
                            let payload = serde_json::json!({
                                "contents": [{
                                    "parts": [{"text": prompt}]
                                }]
                            });

                            match client.post(&url).json(&payload).send().await {
                                Ok(response) => {
                                    if let Ok(body) = response.json::<serde_json::Value>().await {
                                        if let Some(generated_text) = body.get("candidates")
                                            .and_then(|c| c.get(0))
                                            .and_then(|c| c.get("content"))
                                            .and_then(|c| c.get("parts"))
                                            .and_then(|p| p.get(0))
                                            .and_then(|p| p.get("text"))
                                            .and_then(|t| t.as_str()) {

                                            // Send answer back to Discord
                                            let _ = msg.channel_id.say(&ctx.http, generated_text).await;
                                            success = true;
                                        }
                                    }
                                }
                                Err(e) => println!("Failed to query Gemini for crawl synthesis: {:?}", e),
                            }
                        } else {
                            println!("GEMINI_API_KEY missing for Crawl intent.");
                        }
                    } else {
                        let mut log_content = classification.formatted_content.clone();

                        // Phase 3 Regex Auto-Backlinker
                        for entity in &classification.entities_found {
                            let escaped_entity = regex::escape(entity);

                            // First, un-wrap any already-wrapped entities to normalize.
                            // e.g., turn [[Alice]] or [[[[Alice]]]] into Alice.
                            // This regex is slightly over-permissive by stripping any sequence of '[' and ']'
                            // immediately bounding the word, ensuring a clean slate.
                            if let Ok(strip_re) = regex::Regex::new(&format!(r"\[+{}\]+", escaped_entity)) {
                                log_content = strip_re.replace_all(&log_content, entity.as_str()).to_string();
                            }

                            // Now, safely wrap with exactly one set of [[ ]]
                            if let Ok(re) = regex::Regex::new(&format!(r"(?i)\b{}\b", escaped_entity)) {
                                log_content = re.replace_all(&log_content, format!("[[{}]]", entity)).to_string();
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

    let entity_cache = Arc::new(RwLock::new(Vec::new()));

    tokio::spawn(indexer::start_indexer(entity_cache.clone()));

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler::new(entity_cache))
        .await
        .expect("Err creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
