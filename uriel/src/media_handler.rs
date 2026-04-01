use chrono::Datelike;
use reqwest;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub async fn download(url: &str) -> Result<Vec<u8>, reqwest::Error> {
    let response = reqwest::get(url).await?.error_for_status()?;
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
        let attachments_dir = PathBuf::from(vault_path)
            .join("attachments")
            .join(year.to_string())
            .join(month);

        if !attachments_dir.exists() {
            fs::create_dir_all(&attachments_dir)?;
        }

        let file_path = attachments_dir.join(&file_name);

        let mut file = std::fs::File::create(&file_path)?;
        file.write_all(&bytes)?;

        let absolute_path = file_path.canonicalize().unwrap_or(file_path.clone());

        Ok::<String, Box<dyn std::error::Error + Send + Sync>>(absolute_path.to_string_lossy().to_string())
    })
    .await??;

    Ok(result)
}
