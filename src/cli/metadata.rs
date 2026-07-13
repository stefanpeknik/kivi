use crate::cli::OutputArgs;
use crate::error::KiviError;
use crate::output::OutputFormat;
use crate::vault::client::VaultClient;
use crate::vault::kv;
use clap::{Args, Subcommand};
use dialoguer::Confirm;
use serde_json::json;

#[derive(Args, Debug)]
pub struct MetadataArgs {
    #[command(subcommand)]
    pub command: MetadataCommand,
}

#[derive(Subcommand, Debug)]
pub enum MetadataCommand {
    /// Show metadata and version history
    Show(MetadataShowArgs),
    /// Update metadata settings
    Set(MetadataSetArgs),
    /// Permanently delete all versions and metadata
    Delete(MetadataDeleteArgs),
}

#[derive(Args, Debug)]
pub struct MetadataShowArgs {
    #[arg(add = clap_complete::engine::ArgValueCompleter::new(crate::completion::complete_vault_path))]
    pub path: Option<String>,


    #[command(flatten)]
    pub display: OutputArgs,
}

#[derive(Args, Debug)]
pub struct MetadataSetArgs {
    #[arg(add = clap_complete::engine::ArgValueCompleter::new(crate::completion::complete_vault_path))]
    pub path: Option<String>,

    #[arg(long)]
    pub max_versions: Option<u64>,

    #[arg(long)]
    pub cas_required: Option<bool>,

    #[arg(long)]
    pub delete_version_after: Option<String>,

    #[arg(long)]
    pub dry_run: bool,

}

#[derive(Args, Debug)]
pub struct MetadataDeleteArgs {
    #[arg(add = clap_complete::engine::ArgValueCompleter::new(crate::completion::complete_vault_path))]
    pub path: Option<String>,

    #[arg(short = 'f', long)]
    pub force: bool,

    #[arg(long)]
    pub dry_run: bool,

}

pub async fn run(
    args: &MetadataArgs,
    client: &VaultClient,
    mount: &str,
    cfg: &crate::config::Config,
) -> Result<(), KiviError> {
    match &args.command {
        MetadataCommand::Show(a) => {
            let path = cfg.resolve_path(a.path.as_deref())?;
            let meta = kv::read_metadata(client, mount, &path).await?;
            match a.display.output {
                OutputFormat::Human => crate::output::human::print_metadata(&path, &meta),
                OutputFormat::Json => crate::output::json::print_metadata(&path, &meta),
            }
        }
        MetadataCommand::Set(a) => {
            let path = cfg.resolve_path(a.path.as_deref())?;
            if a.dry_run {
                eprintln!("{}", console::style(format!("[dry-run] Would update metadata for {}/{}", mount, path)).yellow());
                return Ok(());
            }
            let mut opts = serde_json::Map::new();
            if let Some(mv) = a.max_versions {
                opts.insert("max_versions".to_string(), json!(mv));
            }
            if let Some(cr) = a.cas_required {
                opts.insert("cas_required".to_string(), json!(cr));
            }
            if let Some(ref dva) = a.delete_version_after {
                opts.insert("delete_version_after".to_string(), json!(dva));
            }
            kv::write_metadata(client, mount, &path, json!(opts)).await?;
            println!("Metadata updated: {}/{}", mount, path);
        }
        MetadataCommand::Delete(a) => {
            let path = cfg.resolve_path(a.path.as_deref())?;
            if a.dry_run {
                eprintln!(
                    "{}",
                    console::style(format!(
                        "[dry-run] Would permanently delete all metadata for {}/{}",
                        mount, path
                    ))
                    .yellow()
                );
                return Ok(());
            }
            if !a.force {
                let confirmed = Confirm::new()
                    .with_prompt(format!(
                        "Permanently delete ALL versions and metadata for {}/{}? This cannot be undone.",
                        mount, path
                    ))
                    .default(false)
                    .interact()
                    .map_err(|e| KiviError::Config(e.to_string()))?;
                if !confirmed {
                    println!("Aborted.");
                    return Ok(());
                }
            }
            kv::delete_metadata(client, mount, &path).await?;
            println!("Metadata deleted: {}/{}", mount, path);
        }
    }
    Ok(())
}
