use std::process::Command;

pub fn search_vault(query: &str, vault_path: &str) -> String {
    let output = Command::new("rg")
        .arg("-i")        // case insensitive
        .arg("-C")        // include context
        .arg("2")         // 2 lines before and after
        .arg("-m")        // max matches
        .arg("5")         // 5 matches per file
        .arg(query)
        .arg(vault_path)
        .output();

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
