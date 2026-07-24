use crate::cli::OutputArgs;
use crate::error::KiviError;
use crate::output::OutputFormat;
use crate::output::resolve_value_mode;
use crate::vault::client::VaultClient;
use crate::vault::kv;
use crate::vault::types::FullMetadataData;
use clap::Args;
use console::style;

#[derive(Args, Debug)]
pub struct GetArgs {
    /// Path to the secret
    #[arg(add = clap_complete::engine::ArgValueCompleter::new(crate::completion::complete_vault_path))]
    pub path: Option<String>,

    /// Read a specific version
    #[arg(long)]
    pub version: Option<u64>,

    /// Reveal secret values
    #[arg(long)]
    pub show_values: bool,

    /// Show only key names, no values
    #[arg(long)]
    pub keys_only: bool,

    /// Show soft-deleted secret metadata
    #[arg(long)]
    pub deleted: bool,

    #[command(flatten)]
    pub display: OutputArgs,
}

pub async fn run(
    args: &GetArgs,
    client: &VaultClient,
    mount: &str,
    show_values_global: bool,
    path: &str,
) -> Result<(), KiviError> {
    let show = args.show_values || show_values_global;
    let mode = resolve_value_mode(show, args.keys_only);

    match kv::read(client, mount, path, args.version).await {
        Ok(secret) => {
            match args.display.output {
                OutputFormat::Human => crate::output::human::print_secret(path, &secret, mode),
                OutputFormat::Json => crate::output::json::print_secret(path, &secret, mode),
            }
            Ok(())
        }
        Err(KiviError::NotFound { .. }) if args.deleted => {
            let meta = kv::read_metadata(client, mount, path).await?;
            match args.display.output {
                OutputFormat::Human => print_deleted_human(path, &meta),
                OutputFormat::Json => print_deleted_json(path, &meta),
            }
            Ok(())
        }
        Err(KiviError::NotFound { .. }) => Err(KiviError::Usage(format!(
            "Secret at '{path}' is soft-deleted.\n  \
             Use: kivi get --deleted {path}\n  \
             Or:  kivi undelete {path}"
        ))),
        Err(e) => Err(e),
    }
}

fn print_deleted_human(path: &str, meta: &FullMetadataData) {
    let ver = meta.current_version.to_string();
    let deletion_time = meta
        .versions
        .get(&ver)
        .and_then(|v| v.deletion_time.as_deref())
        .filter(|s| !s.is_empty())
        .unwrap_or("-");
    let destroyed = meta.versions.get(&ver).is_some_and(|v| v.destroyed);

    println!("{}", style(format!("Path:    {path}")).bold());
    if destroyed {
        println!("Status:  {}", style("destroyed").red());
    } else {
        println!("Status:  {}", style("deleted").dim());
    }
    println!("{}", style(format!("Version: {ver}")).dim());
    println!("Deleted: {deletion_time}");
    println!();
    if destroyed {
        eprintln!("This secret has been permanently destroyed and cannot be recovered.");
    } else {
        eprintln!(
            "Data is unavailable for soft-deleted secrets.\n  \
             Use: kivi undelete {path}"
        );
    }
}

fn print_deleted_json(path: &str, meta: &FullMetadataData) {
    let ver = meta.current_version.to_string();
    let info = meta.versions.get(&ver);
    let deletion_time = info
        .and_then(|v| v.deletion_time.as_deref())
        .filter(|s| !s.is_empty());
    let destroyed = info.is_some_and(|v| v.destroyed);

    let val = serde_json::json!({
        "path": path,
        "deleted": true,
        "destroyed": destroyed,
        "current_version": meta.current_version,
        "deletion_time": deletion_time,
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&val).unwrap_or_else(|_| format!("{val:?}"))
    );
}
