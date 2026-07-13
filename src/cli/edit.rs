use crate::error::KiviError;
use crate::vault::client::VaultClient;
use clap::Args;

#[derive(Args, Debug)]
pub struct EditArgs {
    /// Path to the secret
    #[arg(add = clap_complete::engine::ArgValueCompleter::new(crate::completion::complete_vault_path))]
    pub path: Option<String>,

    /// Skip CAS conflict detection (dangerous)
    #[arg(long)]
    pub no_cas: bool,

    /// Show a key-level diff before writing
    #[arg(long)]
    pub diff: bool,

    /// Open editor and show diff but don't write
    #[arg(long)]
    pub dry_run: bool,
}

pub async fn run(
    args: &EditArgs,
    client: &VaultClient,
    mount: &str,
    path: &str,
) -> Result<(), KiviError> {
    if let Some(version) =
        crate::editor::open_editor(client, mount, path, args.no_cas, args.diff, args.dry_run)
            .await?
    {
        println!("Written: {}/{} (version {})", mount, path, version);
    }
    Ok(())
}
