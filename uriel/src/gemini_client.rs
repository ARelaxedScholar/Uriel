use orichalcum::prelude::*;
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum IntentEnum {
    Note,
    Draft,
    Event,
    Disambiguate,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct UrielClassification {
    pub intent: IntentEnum,
    pub target_folder: String,
    pub entities_found: Vec<String>,
    pub formatted_content: String,
}

#[derive(Deserialize, Debug)]
struct GeminiResponse {
    candidates: Option<Vec<Candidate>>,
}

#[derive(Deserialize, Debug)]
struct Candidate {
    content: Content,
}

#[derive(Deserialize, Debug)]
struct Content {
    parts: Vec<Part>,
}

#[derive(Deserialize, Debug)]
struct Part {
    text: String,
}

#[derive(Clone)]
pub struct GeminiClassifierNode {
    pub api_key: String,
}

#[async_trait::async_trait]
impl AsyncNodeLogic for GeminiClassifierNode {
    fn clone_box(&self) -> Box<dyn AsyncNodeLogic> {
        Box::new(self.clone())
    }

    async fn prep(&self, _params: &std::collections::HashMap<String, NodeValue>, shared: &std::collections::HashMap<String, NodeValue>) -> NodeValue {
        let content = shared.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let media_metadata = shared.get("media_metadata").cloned().unwrap_or(serde_json::Value::Null);

        serde_json::json!({
            "content": content,
            "media_metadata": media_metadata
        })
    }

    async fn exec(&self, input: NodeValue) -> NodeValue {
        let content = input.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let media_val = input.get("media_metadata").cloned().unwrap_or(serde_json::Value::Null);

        let client = Client::new();
        let url = format!("https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash:generateContent?key={}", self.api_key);

        let instruction = "Analyze the provided message. Classify its intent into one of: Note, Draft, Event, or Disambiguate.
Determine the appropriate target folder (e.g., Log, Projects, etc.).
Extract any named entities (people, projects, concepts) found in the text.
Format the content cleanly as Markdown.";

        let mut parts = Vec::new();

        if let Some(media_arr) = media_val.as_array() {
            for m in media_arr {
                if let (Some(uri), Some(mime)) = (m.get("uri").and_then(|v| v.as_str()), m.get("mime").and_then(|v| v.as_str())) {
                    parts.push(serde_json::json!({
                        "fileData": {
                            "fileUri": uri,
                            "mimeType": mime
                        }
                    }));
                }
            }
        }

        parts.push(serde_json::json!({
            "text": content
        }));

        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "intent": {
                    "type": "string",
                    "enum": ["Note", "Draft", "Event", "Disambiguate"]
                },
                "target_folder": {
                    "type": "string"
                },
                "entities_found": {
                    "type": "array",
                    "items": {
                        "type": "string"
                    }
                },
                "formatted_content": {
                    "type": "string"
                }
            },
            "required": ["intent", "target_folder", "entities_found", "formatted_content"]
        });

        let payload = serde_json::json!({
            "system_instruction": {
                "parts": [{
                    "text": instruction
                }]
            },
            "contents": [{
                "parts": parts
            }],
            "generationConfig": {
                "responseMimeType": "application/json",
                "responseSchema": schema
            }
        });

        match client.post(&url).json(&payload).send().await {
            Ok(res) => {
                match res.error_for_status() {
                    Ok(res) => {
                        if let Ok(gemini_resp) = res.json::<GeminiResponse>().await {
                            let json_text = gemini_resp.candidates
                                .and_then(|mut c| c.pop())
                                .and_then(|mut c| c.content.parts.pop())
                                .map(|p| p.text)
                                .unwrap_or_else(|| "{}".to_string());

                            serde_json::from_str(&json_text).unwrap_or(serde_json::json!({
                                "error": "Failed to parse inner JSON"
                            }))
                        } else {
                            serde_json::json!({"error": "Failed to parse GeminiResponse"})
                        }
                    }
                    Err(e) => serde_json::json!({"error": format!("API Error: {}", e)})
                }
            }
            Err(e) => serde_json::json!({"error": format!("Network Error: {}", e)})
        }
    }

    async fn post(&self, shared: &mut std::collections::HashMap<String, NodeValue>, _prep_res: NodeValue, exec_res: NodeValue) -> Option<String> {
        shared.insert("classification".to_string(), exec_res);
        None
    }
}

pub async fn process_message(api_key: &str, content: &str, media_metadata: &[(String, String)]) -> Result<UrielClassification, Box<dyn std::error::Error + Send + Sync>> {
    let mut state = std::collections::HashMap::new();
    state.insert("content".to_string(), serde_json::Value::String(content.to_string()));

    let media_json: Vec<serde_json::Value> = media_metadata.iter().map(|(uri, mime)| {
        serde_json::json!({"uri": uri, "mime": mime})
    }).collect();

    state.insert("media_metadata".to_string(), serde_json::Value::Array(media_json));

    let node = AsyncNode::new(GeminiClassifierNode { api_key: api_key.to_string() });
    let flow = AsyncFlow::new(Executable::Async(node));
    flow.run(&mut state).await;

    let class_val = state.get("classification").cloned().unwrap_or(serde_json::Value::Null);
    if class_val.get("error").is_some() {
        return Err(format!("Classification error: {:?}", class_val).into());
    }

    let classification: UrielClassification = serde_json::from_value(class_val)?;
    Ok(classification)
}
