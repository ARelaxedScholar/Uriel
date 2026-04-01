use chrono::NaiveDate;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

pub fn append_log(content: &str, date: NaiveDate) -> std::io::Result<()> {
    let vault_path = std::env::var("VAULT_PATH").unwrap_or_else(|_| ".".to_string());

    // Example: [VAULT_PATH]/Log/2023-10-25.md
    let log_dir = PathBuf::from(vault_path).join("Log");

    std::fs::create_dir_all(&log_dir)?;

    let file_path = log_dir.join(format!("{}.md", date.format("%Y-%m-%d")));

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(file_path)?;

    writeln!(file, "{}", content)?;

    Ok(())
}
