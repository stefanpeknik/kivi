use crate::error::KiviError;
use crate::vault::client::VaultClient;
use crate::vault::kv;
use console::style;
use serde_json::Value;
use std::collections::HashMap;

pub async fn open_editor(
    client: &VaultClient,
    mount: &str,
    path: &str,
    no_cas: bool,
    show_diff: bool,
    dry_run: bool,
) -> Result<Option<u64>, KiviError> {
    loop {
        let secret = kv::read(client, mount, path, None).await?;
        let original_json = serde_json::to_string_pretty(&secret.data)?;
        let version = secret.metadata.version;

        let tmp = tempfile::Builder::new()
            .prefix("kivi-edit-")
            .suffix(".json")
            .tempfile()?;

        tokio::fs::write(tmp.path(), &original_json).await?;

        let editor = std::env::var("EDITOR")
            .or_else(|_| std::env::var("VISUAL"))
            .unwrap_or_else(|_| "vi".to_string());

        let status = tokio::process::Command::new(&editor)
            .arg(tmp.path())
            .status()
            .await?;

        if !status.success() {
            return Err(KiviError::Usage(format!("Editor '{editor}' exited with error")));
        }

        let edited = tokio::fs::read_to_string(tmp.path()).await?;

        if edited.trim() == original_json.trim() {
            println!("No changes.");
            return Ok(None);
        }

        let new_data: HashMap<String, Value> = serde_json::from_str(&edited)
            .map_err(|e| KiviError::Usage(format!("Invalid JSON after editing: {e}")))?;

        if show_diff || dry_run {
            print_diff(&secret.data, &new_data);
        }

        if dry_run {
            eprintln!(
                "{}",
                style("[dry-run] Changes detected — would write new version (no write performed).")
                    .yellow()
            );
            return Ok(None);
        }

        let cas = if no_cas { None } else { Some(version) };

        match kv::write(client, mount, path, new_data, cas).await {
            Ok(new_version) => return Ok(Some(new_version)),
            Err(KiviError::CasConflict { .. }) => {
                use dialoguer::Select;
                let choice = Select::new()
                    .with_prompt("Secret was modified by someone else. What do you want to do?")
                    .items(&["Retry (re-fetch and re-edit)", "Force (overwrite anyway)", "Abort"])
                    .default(0)
                    .interact().map_err(|dialoguer::Error::IO(e)| e)?;

                match choice {
                    0 => continue,
                    1 => {
                        let edited_data: HashMap<String, Value> =
                            serde_json::from_str(&edited)?;
                        let v = kv::write(client, mount, path, edited_data, None).await?;
                        return Ok(Some(v));
                    }
                    _ => {
                        println!("Aborted.");
                        return Ok(None);
                    }
                }
            }
            Err(e) => return Err(e),
        }
    }
}

fn print_diff(original: &HashMap<String, Value>, edited: &HashMap<String, Value>) {
    let mut all_keys: Vec<&str> = original
        .keys()
        .chain(edited.keys())
        .map(|s| s.as_str())
        .collect();
    all_keys.sort();
    all_keys.dedup();

    println!("Changes:");
    for key in all_keys {
        match (original.get(key), edited.get(key)) {
            (None, Some(_)) => println!("  {} {}", style("+").green(), style(key).green()),
            (Some(_), None) => println!("  {} {}", style("-").red(), style(key).red()),
            (Some(old), Some(new)) if old != new => {
                println!("  {} {}", style("~").yellow(), style(key).yellow())
            }
            _ => {}
        }
    }
}
