use reqwest::Client;
use serde::Deserialize;
use std::fs::File;
use std::io::Read;

#[derive(Deserialize, Debug)]
struct UploadResponse {
    file: GeminiFile,
}

#[derive(Deserialize, Debug)]
pub struct GeminiFile {
    pub uri: String,
}

pub async fn upload_file(api_key: &str, file_path: &str, mime_type: &str) -> Result<GeminiFile, Box<dyn std::error::Error + Send + Sync>> {
    let client = Client::new();

    // 1. Initial resumable upload request
    let url = format!("https://generativelanguage.googleapis.com/upload/v1beta/files?uploadType=resumable&key={}", api_key);

    let payload = serde_json::json!({
        "file": {
            "display_name": file_path
        }
    });

    let init_res = client.post(&url)
        .header("X-Goog-Upload-Protocol", "resumable")
        .header("X-Goog-Upload-Command", "start")
        .header("X-Goog-Upload-Header-Content-Length", std::fs::metadata(file_path)?.len())
        .header("X-Goog-Upload-Header-Content-Type", mime_type)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await?;

    let upload_url = init_res.headers().get("X-Goog-Upload-URL")
        .ok_or("Missing upload URL")?
        .to_str()?
        .to_string();

    // 2. Upload the bytes
    let mut file = File::open(file_path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    let upload_res = client.post(&upload_url)
        .header("Content-Length", buffer.len().to_string())
        .header("X-Goog-Upload-Offset", "0")
        .header("X-Goog-Upload-Command", "upload, finalize")
        .body(buffer)
        .send()
        .await?;

    let upload_data: UploadResponse = upload_res.json().await?;

    Ok(upload_data.file)
}
