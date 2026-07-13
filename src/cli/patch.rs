use crate::error::KiviError;
use crate::vault::client::VaultClient;
use crate::vault::kv;
use clap::Args;
use console::style;
use serde_json::Value;
use tokio::io::AsyncReadExt;

#[derive(Args, Debug)]
pub struct PatchArgs {
    /// Path to the secret
    #[arg(add = clap_complete::engine::ArgValueCompleter::new(crate::completion::complete_vault_path))]
    pub path: Option<String>,

    /// Key=value pairs to update
    pub entries: Vec<String>,

    /// Remove a specific key from the secret
    #[arg(long)]
    pub remove: Vec<String>,

    /// Read updates from stdin as JSON
    #[arg(long)]
    pub stdin: bool,

    /// Show what would happen without writing
    #[arg(long)]
    pub dry_run: bool,
}

pub async fn run(
    args: &PatchArgs,
    client: &VaultClient,
    mount: &str,
    path: &str,
) -> Result<(), KiviError> {
    let mut data = crate::cli::put::parse_kv_args(&args.entries)?;

    if args.stdin {
        let mut input = String::new();
        tokio::io::stdin().read_to_string(&mut input).await?;
        let extra: std::collections::HashMap<String, Value> = serde_json::from_str(&input)
            .map_err(|e| KiviError::Usage(format!("Invalid JSON: {e}")))?;
        data.extend(extra);
    }

    for key in &args.remove {
        data.insert(key.clone(), Value::Null);
    }

    if args.dry_run {
        eprintln!(
            "{}",
            style(format!("[dry-run] Would patch {}/{}", mount, path)).yellow()
        );
        for (k, v) in &data {
            if v.is_null() {
                eprintln!("{}", style(format!("[dry-run]   remove {k}")).yellow());
            } else {
                eprintln!("{}", style(format!("[dry-run]   set {k}")).yellow());
            }
        }
        return Ok(());
    }

    let version = kv::patch(client, mount, path, data).await?;
    println!("Patched: {}/{} (version {})", mount, path, version);
    Ok(())
}
