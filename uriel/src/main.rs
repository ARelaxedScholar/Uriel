use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::model::application::{Command, Interaction};
use serenity::prelude::*;
use std::env;
use std::path::PathBuf;

use moka::future::Cache;
use std::time::Duration;

mod vault_io;
mod media_handler;
mod gemini_client;
mod gemini_file_api;

struct Handler {
    thread_cache: Cache<u64, bool>,
}

impl Handler {
    fn new() -> Self {
        Handler {
            thread_cache: Cache::builder()
                .time_to_live(Duration::from_secs(60 * 60)) // 1 hour TTL
                .build(),
        }
    }
}

const DROPBOX_CHANNEL_ID: u64 = 1111111111111111111; // Placeholder
const DISCUSS_CHANNEL_ID: u64 = 2222222222222222222; // Placeholder
const DIGEST_CHANNEL_ID: u64 = 3333333333333333333; // Placeholder

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot {
            return;
        }

        let channel_id = msg.channel_id.get();
        let is_dropbox = channel_id == DROPBOX_CHANNEL_ID;
        let mut is_dropbox_thread = false;

        if !is_dropbox && channel_id != DISCUSS_CHANNEL_ID && channel_id != DIGEST_CHANNEL_ID {
            // Check cache first to avoid API spam
            if let Some(cached) = self.thread_cache.get(&channel_id).await {
                is_dropbox_thread = cached;
            } else {
                // Not in cache, query Discord API
                if let Ok(channel) = msg.channel_id.to_channel(&ctx.http).await {
                    if let serenity::model::channel::Channel::Guild(guild_channel) = channel {
                        if let Some(parent_id) = guild_channel.parent_id {
                            if parent_id.get() == DROPBOX_CHANNEL_ID {
                                is_dropbox_thread = true;
                            }
                        }
                    }
                }
                // Cache the result (whether true or false) to prevent future API calls for this channel
                self.thread_cache.insert(channel_id, is_dropbox_thread).await;
            }
        }

        // Drop messages outside our target channels and threads
        if !is_dropbox && !is_dropbox_thread
            && channel_id != DISCUSS_CHANNEL_ID
            && channel_id != DIGEST_CHANNEL_ID {
            return;
        }

        if is_dropbox || is_dropbox_thread {
            let mut final_content = msg.content.clone();

            let vault_path = env::var("VAULT_PATH").unwrap_or_else(|_| ".".to_string());

            let mut media_paths = Vec::new();

            for attachment in msg.attachments.iter() {
                match media_handler::route_to_vault(&attachment.url, &vault_path, &attachment.filename).await {
                    Ok(vault_rel_path) => {
                        final_content.push_str(&format!("\n![[{}]]", vault_rel_path));
                        media_paths.push(vault_rel_path);
                    },
                    Err(e) => {
                        println!("Failed to download attachment: {:?}", e);
                    }
                }
            }

            let mut context_content = final_content.clone();

            if is_dropbox_thread {
                let thread_id = msg.channel_id;

                use serenity::builder::GetMessages;
                let msgs = thread_id.messages(&ctx.http, GetMessages::new().limit(5)).await.unwrap_or_default();

                let mut context_str_parts = Vec::new();
                for m in msgs.iter().rev() {
                    context_str_parts.push(format!("{}: {}", m.author.name, m.content));
                }

                if !context_str_parts.is_empty() {
                    context_content = format!("Recent Context:\n{}\n\nMessage: {}", context_str_parts.join("\n"), final_content);
                }
            }

            let gemini_key = match env::var("GEMINI_API_KEY") {
                Ok(key) => key,
                Err(_) => {
                    println!("Error: GEMINI_API_KEY is missing");
                    return;
                }
            };

            // Upload media to Gemini File API
            let mut media_metadata = Vec::new();
            for path in &media_paths {
                let absolute_path = PathBuf::from(&vault_path).join(path);
                if let Some(ext) = absolute_path.extension().and_then(|s| s.to_str()) {
                    let ext_str = ext.to_lowercase();
                    let mime_type = match ext_str.as_str() {
                        "png" => "image/png",
                        "jpg" | "jpeg" => "image/jpeg",
                        "webp" => "image/webp",
                        "ogg" => "audio/ogg",
                        "mp3" => "audio/mp3",
                        "wav" => "audio/wav",
                        "mp4" => "video/mp4",
                        _ => "application/octet-stream",
                    };

                    match gemini_file_api::upload_file(&gemini_key, absolute_path.to_str().unwrap_or(""), mime_type).await {
                        Ok(gemini_file) => {
                            media_metadata.push((gemini_file.uri, mime_type.to_string()));
                        },
                        Err(e) => println!("Failed to upload file to Gemini API: {:?}", e),
                    }
                }
            }

            match gemini_client::process_message(&gemini_key, &context_content, &media_metadata).await {
                Ok(classification) => {
                    println!("Classification: {:?}", classification);
                    let today = chrono::Local::now().naive_local().date();
                    // Just write to log for now as requested by phase 1 and implied, we can use target_folder later
                    if let Err(e) = vault_io::append_log(&classification.formatted_content, today) {
                        println!("Failed to append to log: {:?}", e);
                    }
                },
                Err(e) => {
                    println!("Gemini processing failed: {:?}", e);
                    // Fallback to naive saving
                    let today = chrono::Local::now().naive_local().date();
                    if let Err(e) = vault_io::append_log(&final_content, today) {
                        println!("Failed to append to log: {:?}", e);
                    }
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

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = interaction {
            let response = match command.data.name.as_str() {
                "note" => "Note command recognized (placeholder).",
                "draft" => "Draft command recognized (placeholder).",
                "event" => "Event command recognized (placeholder).",
                _ => "Unknown command.",
            };

            use serenity::builder::{CreateInteractionResponse, CreateInteractionResponseMessage};

            let data = CreateInteractionResponseMessage::new()
                .content(response)
                .ephemeral(true);
            let builder = CreateInteractionResponse::Message(data);

            if let Err(why) = command.create_response(&ctx.http, builder).await {
                println!("Cannot respond to slash command: {why}");
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler::new())
        .await
        .expect("Err creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
