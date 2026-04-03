use std::env;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};
use walkdir::WalkDir;

pub async fn start_indexer(cache: Arc<RwLock<Vec<String>>>) {
    let vault_path = env::var("VAULT_PATH").unwrap_or_else(|_| ".".to_string());

    let mut interval = interval(Duration::from_secs(5 * 60));

    loop {
        interval.tick().await;

        let vault_path_clone = vault_path.clone();

        let new_entities = tokio::task::spawn_blocking(move || {
            let mut entities = Vec::new();
            for folder in &["Person", "Projects"] {
                let path = Path::new(&vault_path_clone).join(folder);

                if !path.exists() {
                    continue;
                }

                for entry in WalkDir::new(&path).into_iter().filter_map(|e| e.ok()) {
                    if entry.file_type().is_file() {
                        let file_name = entry.file_name().to_string_lossy();
                        if file_name.ends_with(".md") {
                            let base_name = file_name.trim_end_matches(".md").to_string();
                            entities.push(base_name);
                        }
                    }
                }
            }
            entities
        }).await.unwrap_or_default();

        let mut cache_lock = cache.write().await;
        *cache_lock = new_entities;
    }
}
