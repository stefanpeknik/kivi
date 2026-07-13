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

pub async fn run(
    args: &PutArgs,
    client: &VaultClient,
    mount: &str,
    path: &str,
) -> Result<(), KiviError> {
    let data = parse_input(args).await?;

    if args.dry_run {
        let keys = data.keys().cloned().collect::<Vec<_>>().join(", ");
        eprintln!(
            "{}",
            style(format!(
                "[dry-run] Would write to {}/{} — keys: {}",
                mount, path, keys
            ))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_kv_basic() {
        let entries = vec!["HOST=localhost".to_string()];
        let result = parse_kv_args(&entries).unwrap();
        assert_eq!(result["HOST"], Value::String("localhost".into()));
    }

    #[test]
    fn parse_kv_multiple_pairs() {
        let entries = vec![
            "HOST=localhost".to_string(),
            "PORT=5432".to_string(),
            "DB=mydb".to_string(),
        ];
        let result = parse_kv_args(&entries).unwrap();
        assert_eq!(result.len(), 3);
        assert_eq!(result["PORT"], Value::String("5432".into()));
    }

    #[test]
    fn parse_kv_value_containing_equals() {
        let entries = vec!["CONN=host=db;port=5432".to_string()];
        let result = parse_kv_args(&entries).unwrap();
        assert_eq!(result["CONN"], Value::String("host=db;port=5432".into()));
    }

    #[test]
    fn parse_kv_empty_value() {
        let entries = vec!["KEY=".to_string()];
        let result = parse_kv_args(&entries).unwrap();
        assert_eq!(result["KEY"], Value::String("".into()));
    }

    #[test]
    fn parse_kv_empty_entries() {
        let result = parse_kv_args(&[]).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn parse_kv_missing_equals_is_error() {
        let entries = vec!["NOEQUALS".to_string()];
        assert!(parse_kv_args(&entries).is_err());
    }

    #[test]
    fn parse_kv_later_entry_overwrites_earlier() {
        let entries = vec!["KEY=first".to_string(), "KEY=second".to_string()];
        let result = parse_kv_args(&entries).unwrap();
        assert_eq!(result["KEY"], Value::String("second".into()));
    }

    #[test]
    fn parse_kv_all_values_are_strings() {
        let entries = vec![
            "NUM=42".to_string(),
            "BOOL=true".to_string(),
            "NULL=null".to_string(),
        ];
        let result = parse_kv_args(&entries).unwrap();
        assert!(result["NUM"].is_string());
        assert!(result["BOOL"].is_string());
        assert!(result["NULL"].is_string());
    }
}
