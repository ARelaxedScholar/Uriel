use chrono::Datelike;
use futures_util::StreamExt;
use reqwest;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

/// Maximum allowed download size in bytes (e.g., 20 MiB).
const MAX_DOWNLOAD_SIZE: usize = 20 * 1024 * 1024;

pub async fn download(url: &str) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    let response = client.get(url).send().await?.error_for_status()?;

    let mut data: Vec<u8> = Vec::new();
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if data.len() + chunk.len() > MAX_DOWNLOAD_SIZE {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "downloaded file exceeds maximum allowed size",
            )));
        }
        data.extend_from_slice(&chunk);
    }

    Ok(data)
}

fn sanitize_filename(name: &str) -> String {
    name.replace("/", "_").replace("\\", "_")
}

pub async fn route_to_vault(url: &str, vault_path: &str, file_name: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let bytes = download(url).await?;

    let now = chrono::Local::now();
    let year = now.year();
    let month = format!("{:02}", now.month());

    let relative_dir = PathBuf::from("attachments")
        .join(year.to_string())
        .join(&month);
    let attachments_dir = PathBuf::from(vault_path).join(&relative_dir);

    fs::create_dir_all(&attachments_dir)?;

    let safe_name = sanitize_filename(file_name);
    let mut base_name = safe_name.clone();
    let mut ext = String::new();

    if let Some(idx) = safe_name.rfind('.') {
        base_name = safe_name[..idx].to_string();
        ext = safe_name[idx..].to_string();
    }

    let mut counter = 0;
    let mut final_name = safe_name.clone();
    let mut file_path = attachments_dir.join(&final_name);

    let mut file = loop {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&file_path)
        {
            Ok(f) => break f,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                counter += 1;
                final_name = format!("{}-{}{}", base_name, counter, ext);
                file_path = attachments_dir.join(&final_name);
            }
            Err(e) => return Err(Box::new(e)),
        }
    };

    file.write_all(&bytes)?;

    let vault_rel_path = relative_dir.join(&final_name);
    Ok(vault_rel_path.to_string_lossy().to_string())
}
