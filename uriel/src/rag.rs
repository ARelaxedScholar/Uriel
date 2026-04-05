use tokio::process::Command;

pub async fn search_vault(query: &str, vault_path: &str) -> String {
    let output = Command::new("rg")
        .arg("-i")        // case insensitive
        .arg("-C")        // include context
        .arg("2")         // 2 lines before and after
        .arg("-m")        // max matches
        .arg("5")         // 5 matches per file
        .arg("--")        // prevent flag injection
        .arg(query)
        .arg(vault_path)
        .output()
        .await;

    match output {
        Ok(out) => {
            if out.status.success() {
                String::from_utf8_lossy(&out.stdout).into_owned()
            } else {
                // Return stderr if there was an error running rg
                let err_msg = String::from_utf8_lossy(&out.stderr).into_owned();
                if err_msg.is_empty() && out.status.code() == Some(1) {
                    // ripgrep exits with 1 when no matches are found
                    "No matches found.".to_string()
                } else {
                    format!("Error running ripgrep: {}", err_msg)
                }
            }
        }
        Err(e) => format!("Failed to execute ripgrep command: {}", e),
    }
}

pub async fn crawl_connections(file_name: &str, vault_path: &str) -> String {
    let mut results = String::new();

    // Get backlinks
    let backlinks_output = Command::new("obsidian")
        .current_dir(vault_path)
        .arg("backlinks")
        .arg(format!("file={}", file_name))
        .output()
        .await;

    results.push_str("=== Backlinks ===\n");
    match backlinks_output {
        Ok(out) => {
            if out.status.success() {
                results.push_str(&String::from_utf8_lossy(&out.stdout));
            } else {
                results.push_str("No backlinks found or error retrieving them.\n");
            }
        }
        Err(_) => results.push_str("Failed to execute obsidian CLI for backlinks.\n"),
    }

    // Get outgoing links
    let links_output = Command::new("obsidian")
        .current_dir(vault_path)
        .arg("links")
        .arg(format!("file={}", file_name))
        .output()
        .await;

    results.push_str("\n=== Outgoing Links ===\n");
    match links_output {
        Ok(out) => {
            if out.status.success() {
                results.push_str(&String::from_utf8_lossy(&out.stdout));
            } else {
                results.push_str("No outgoing links found or error retrieving them.\n");
            }
        }
        Err(_) => results.push_str("Failed to execute obsidian CLI for outgoing links.\n"),
    }

    results
}
