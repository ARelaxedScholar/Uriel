use std::path::PathBuf;
use walkdir::WalkDir;
use std::collections::HashSet;

pub fn scan_vault(vault_path: &str) -> Vec<String> {
    let mut entities = HashSet::new();

    let paths_to_scan = [
        PathBuf::from(vault_path).join("Person"),
        PathBuf::from(vault_path).join("Projects"),
    ];

    for path in paths_to_scan {
        if path.exists() && path.is_dir() {
            for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() {
                    let file_path = entry.path();
                    if let Some(ext) = file_path.extension() {
                        if ext == "md" {
                            if let Some(stem) = file_path.file_stem() {
                                if let Some(stem_str) = stem.to_str() {
                                    entities.insert(stem_str.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    entities.into_iter().collect()
}
