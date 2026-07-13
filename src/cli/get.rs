use crate::cli::OutputArgs;
use crate::error::KiviError;
use crate::output::{OutputFormat, resolve_value_mode};
use crate::vault::client::VaultClient;
use crate::vault::kv;
use clap::Args;

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
    let secret = kv::read(client, mount, path, args.version).await?;

    match args.display.output {
        OutputFormat::Human => crate::output::human::print_secret(path, &secret, mode),
        OutputFormat::Json => crate::output::json::print_secret(path, &secret, mode),
    }
    Ok(())
}
