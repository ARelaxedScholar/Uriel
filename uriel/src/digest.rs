use std::env;
use std::fs;
use std::path::PathBuf;
use chrono::{Local, Duration, NaiveDate};
use reqwest::Client;
use serde_json::json;
use serenity::all::ChannelId;
use serenity::all::Http;

pub async fn run_digest(http: std::sync::Arc<Http>) {
    let vault_path = env::var("VAULT_PATH").unwrap_or_else(|_| ".".to_string());
    let log_dir = PathBuf::from(vault_path).join("Log");

    if !log_dir.exists() {
        println!("Log directory does not exist. Skipping digest.");
        return;
    }

    let today = Local::now().naive_local().date();
    let mut log_contents = String::new();

    for i in 0..=7 {
        let target_date = today - Duration::days(i);
        let file_path = log_dir.join(format!("{}.md", target_date.format("%Y-%m-%d")));

        if file_path.exists() {
            if let Ok(content) = fs::read_to_string(&file_path) {
                log_contents.push_str(&format!("--- Log for {} ---\n{}\n\n", target_date, content));
            }
        }
    }

    if log_contents.is_empty() {
        println!("No logs found for the last 7 days. Skipping digest.");
        return;
    }

    let prompt = format!(
        "Here are my logs for the past 7 days:\n\n{}\n\nIdentify 3 core patterns and 1 crucial objective for today. Format nicely.",
        log_contents
    );

    let api_key = match env::var("GEMINI_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            println!("GEMINI_API_KEY not found. Skipping digest.");
            return;
        }
    };

    let client = Client::new();
    // Prompt specifies Gemini 1.5 Pro
    let url = format!("https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-pro:generateContent?key={}", api_key);

    let payload = json!({
        "contents": [{
            "parts": [{"text": prompt}]
        }]
    });

    let mut generated_text = String::from("Failed to generate digest.");

    match client.post(&url).json(&payload).send().await {
        Ok(response) => {
            if let Ok(body) = response.json::<serde_json::Value>().await {
                if let Some(text) = body.get("candidates")
                    .and_then(|c| c.get(0))
                    .and_then(|c| c.get("content"))
                    .and_then(|c| c.get("parts"))
                    .and_then(|p| p.get(0))
                    .and_then(|p| p.get("text"))
                    .and_then(|t| t.as_str()) {
                    generated_text = text.to_string();
                } else {
                     println!("Could not parse digest from Gemini response.");
                }
            }
        }
        Err(e) => println!("API call to Gemini failed: {}", e),
    }

    let digest_channel_id_str = match env::var("DIGEST_CHANNEL_ID") {
        Ok(id) => id,
        Err(_) => {
             println!("DIGEST_CHANNEL_ID not set. Skipping digest.");
             return;
        }
    };

    if let Ok(channel_id_u64) = digest_channel_id_str.parse::<u64>() {
        let channel_id = ChannelId::new(channel_id_u64);
        if let Err(e) = channel_id.say(&http, generated_text).await {
            println!("Failed to send digest to Discord: {}", e);
        }
    } else {
         println!("Invalid DIGEST_CHANNEL_ID format.");
    }
}