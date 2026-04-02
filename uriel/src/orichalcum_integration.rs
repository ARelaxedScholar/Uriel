use orichalcum::AsyncNodeLogic;
use serde_json::{json, Value as JsonValue};
use crate::gemini::UrielClassification;
use std::env;
use std::fs;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::collections::HashMap;
use async_trait::async_trait;

pub struct GeminiIngestionNode;

#[async_trait]
impl AsyncNodeLogic for GeminiIngestionNode {
    fn clone_box(&self) -> Box<dyn AsyncNodeLogic> {
        Box::new(Self)
    }

    async fn prep(&self, state: &HashMap<String, JsonValue>, _env: &HashMap<String, JsonValue>) -> JsonValue {
        json!(state)
    }

    async fn exec(&self, input_val: JsonValue) -> JsonValue {
        let input_text = input_val.get("input_text").and_then(|v| v.as_str()).unwrap_or("");

        let mut parts = vec![json!({"text": format!("Classify the following text into one of these intents: Note, Draft, Event, Disambiguate. Return JSON strictly matching this schema: {{ 'intent': '...', 'target_folder': '...', 'entities_found': [...], 'formatted_content': '...' }}. Text: {}", input_text)})];

        if let Some(media_path) = input_val.get("media_path").and_then(|v| v.as_str()) {
            if let Ok(bytes) = fs::read(media_path) {
                let base64_data = STANDARD.encode(&bytes);
                let mime_type = if media_path.ends_with(".png") {
                    "image/png"
                } else if media_path.ends_with(".ogg") {
                    "audio/ogg"
                } else if media_path.ends_with(".jpg") || media_path.ends_with(".jpeg") {
                    "image/jpeg"
                } else {
                    "application/octet-stream"
                };

                parts.push(json!({
                    "inlineData": {
                        "mimeType": mime_type,
                        "data": base64_data
                    }
                }));
            }
        }

        let prompt = parts;

        let api_key = match env::var("GEMINI_API_KEY") {
            Ok(key) => key,
            Err(_) => return json!({"error": "Missing GEMINI_API_KEY"}),
        };

        let client = reqwest::Client::new();
        let url = format!("https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash:generateContent?key={}", api_key);

        let payload = json!({
            "contents": [{
                "parts": prompt
            }],
            "generationConfig": {
                "response_mime_type": "application/json"
            }
        });

        match client.post(&url).json(&payload).send().await {
            Ok(response) => {
                match response.json::<serde_json::Value>().await {
                    Ok(body) => {
                        let generated_text = body.get("candidates")
                            .and_then(|c| c.get(0))
                            .and_then(|c| c.get("content"))
                            .and_then(|c| c.get("parts"))
                            .and_then(|p| p.get(0))
                            .and_then(|p| p.get("text"))
                            .and_then(|t| t.as_str())
                            .unwrap_or("");

                        match serde_json::from_str::<UrielClassification>(generated_text) {
                            Ok(parsed) => json!(parsed),
                            Err(e) => json!({"error": format!("Invalid JSON schema returned: {}", e)})
                        }
                    },
                    Err(e) => json!({"error": e.to_string()})
                }
            },
            Err(e) => json!({"error": e.to_string()})
        }
    }

    async fn post(&self, _state: &mut HashMap<String, JsonValue>, _env: JsonValue, result: JsonValue) -> Option<String> {
        _state.insert("result".to_string(), result);
        None
    }
}
