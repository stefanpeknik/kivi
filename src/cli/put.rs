use crate::error::KiviError;
use crate::vault::client::VaultClient;
use crate::vault::kv;
use clap::Args;
use console::style;
use serde_json::Value;
use std::collections::HashMap;
use tokio::io::AsyncReadExt;

#[derive(Args, Debug)]
pub struct PutArgs {
    /// Path to write the secret to
    #[arg(add = clap_complete::engine::ArgValueCompleter::new(crate::completion::complete_vault_path))]
    pub path: Option<String>,

    /// Key=value pairs to write (e.g. DB_HOST=localhost)
    pub entries: Vec<String>,

    /// Read key-value pairs from stdin as JSON
    #[arg(long)]
    pub stdin: bool,

    /// Read key-value pairs from a JSON file
    #[arg(long)]
    pub file: Option<std::path::PathBuf>,

    /// Check-And-Set: only write if current version matches
    #[arg(long)]
    pub cas: Option<u64>,

    /// Show what would happen without writing
    #[arg(long)]
    pub dry_run: bool,

}

pub async fn run(args: &PutArgs, client: &VaultClient, mount: &str, path: &str) -> Result<(), KiviError> {
    let data = parse_input(args).await?;

    if args.dry_run {
        let keys = data.keys().cloned().collect::<Vec<_>>().join(", ");
        eprintln!(
            "{}",
            style(format!("[dry-run] Would write to {}/{} — keys: {}", mount, path, keys))
                .yellow()
        );
        return Ok(());
    }

    let version = kv::write(client, mount, path, data, args.cas).await?;
    println!("Written: {}/{} (version {})", mount, path, version);
    Ok(())
}

pub async fn parse_input(args: &PutArgs) -> Result<HashMap<String, Value>, KiviError> {
    if args.stdin {
        let mut input = String::new();
        tokio::io::stdin().read_to_string(&mut input).await?;
        return parse_json(&input);
    }
    if let Some(ref file) = args.file {
        let contents = tokio::fs::read_to_string(file).await?;
        return parse_json(&contents);
    }
    if args.entries.is_empty() {
        return Err(KiviError::Usage(
            "No key=value pairs provided. Use positional args, --stdin, or --file.".to_string(),
        ));
    }
    parse_kv_args(&args.entries)
}

fn parse_json(s: &str) -> Result<HashMap<String, Value>, KiviError> {
    serde_json::from_str(s).map_err(|e| KiviError::Usage(format!("Invalid JSON: {e}")))
}

pub fn parse_kv_args(entries: &[String]) -> Result<HashMap<String, Value>, KiviError> {
    let mut map = HashMap::new();
    for entry in entries {
        let (k, v) = entry.split_once('=').ok_or_else(|| {
            KiviError::Usage(format!(
                "Invalid key=value pair: '{entry}'. Expected format: KEY=value"
            ))
        })?;
        map.insert(k.to_string(), Value::String(v.to_string()));
    }
    Ok(map)
}
