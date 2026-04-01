use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::model::application::{Command, Interaction};
use serenity::prelude::*;
use std::env;

mod vault_io;
mod media_handler;

struct Handler;

const DROPBOX_CHANNEL_ID: u64 = 1111111111111111111; // Placeholder
const DISCUSS_CHANNEL_ID: u64 = 2222222222222222222; // Placeholder
const DIGEST_CHANNEL_ID: u64 = 3333333333333333333; // Placeholder

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, _ctx: Context, msg: Message) {
        // Drop messages outside our target channels
        let channel_id = msg.channel_id.get();
        if channel_id != DROPBOX_CHANNEL_ID
            && channel_id != DISCUSS_CHANNEL_ID
            && channel_id != DIGEST_CHANNEL_ID {
            return;
        }

        if msg.author.bot {
            return;
        }

        if channel_id == DROPBOX_CHANNEL_ID {
            let mut final_content = msg.content.clone();

            let vault_path = env::var("VAULT_PATH").unwrap_or_else(|_| ".".to_string());

            for attachment in msg.attachments.iter() {
                match media_handler::route_to_vault(&attachment.url, &vault_path, &attachment.filename).await {
                    Ok(_) => {
                        final_content.push_str(&format!("\n![[{}]]", attachment.filename));
                    },
                    Err(e) => {
                        println!("Failed to download attachment: {:?}", e);
                    }
                }
            }

            let today = chrono::Local::now().naive_local().date();
            if let Err(e) = vault_io::append_log(&final_content, today) {
                println!("Failed to append to log: {:?}", e);
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
                "note" => {
                    // note command
                },
                "draft" => {
                    // draft command
                },
                "event" => {
                    // event command
                },
                _ => {},
            }
        }
    }
}

#[tokio::main]
async fn main() {
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler)
        .await
        .expect("Err creating client");

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
