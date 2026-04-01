use chrono::Datelike;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub async fn download(url: &str) -> Result<Vec<u8>, reqwest::Error> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;
    let response = client.get(url).send().await?.error_for_status()?;
    let bytes = response.bytes().await?;
    Ok(bytes.to_vec())
}

pub async fn route_to_vault(url: &str, vault_path: &str, file_name: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let vault_path = vault_path.to_string();
    let file_name = file_name.to_string();

    let bytes = download(url).await?;

    let now = chrono::Local::now();
    let year = now.year();
    let month = format!("{:02}", now.month());

    let result = tokio::task::spawn_blocking(move || {
        // Sanitize file_name to prevent path traversal attacks
        let sanitized_name = Path::new(&file_name)
            .file_name()
            .ok_or_else(|| {
                Box::<dyn std::error::Error + Send + Sync>::from(
                    std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid file name"),
                )
            })?
            .to_string_lossy()
            .to_string();

        let attachments_dir = PathBuf::from(vault_path)
            .join("attachments")
            .join(year.to_string())
            .join(month);

        if !attachments_dir.exists() {
            fs::create_dir_all(&attachments_dir)?;
        }

        // Prefix with a nanosecond timestamp to avoid overwriting existing files.
        // Nanosecond resolution makes collisions practically impossible even under
        // concurrent uploads.
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let unique_name = format!("{}_{}", timestamp, sanitized_name);
        let file_path = attachments_dir.join(&unique_name);

        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&file_path)
            .map_err(|e| {
                Box::<dyn std::error::Error + Send + Sync>::from(format!(
                    "failed to create file '{}': {}",
                    unique_name, e
                ))
            })?;
        file.write_all(&bytes)?;

        Ok::<String, Box<dyn std::error::Error + Send + Sync>>(unique_name)
    })
    .await??;

    Ok(result)
}
