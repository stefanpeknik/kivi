use crate::error::KiviError;
use crate::vault::client::VaultClient;
use crate::vault::kv;
use clap::Args;
use console::style;

#[derive(Args, Debug)]
pub struct UndeleteArgs {
    /// Path to the secret
    #[arg(add = clap_complete::engine::ArgValueCompleter::new(crate::completion::complete_vault_path))]
    pub path: Option<String>,

    /// Versions to restore (comma-separated, defaults to current version)
    #[arg(long, value_delimiter = ',')]
    pub versions: Option<Vec<u64>>,

    /// Show what would be restored without acting
    #[arg(long)]
    pub dry_run: bool,
}

pub async fn run(
    args: &UndeleteArgs,
    client: &VaultClient,
    mount: &str,
    path: &str,
) -> Result<(), KiviError> {
    let versions = match &args.versions {
        Some(v) => v.clone(),
        None => {
            let meta = kv::read_metadata(client, mount, path).await?;
            vec![meta.current_version]
        }
    };

    if args.dry_run {
        eprintln!(
            "{}",
            style(format!(
                "[dry-run] Would restore versions {:?} of {}/{}",
                versions, mount, path
            ))
            .yellow()
        );
        return Ok(());
    }
    kv::undelete(client, mount, path, &versions).await?;
    println!("Restored versions {:?} of {}/{}", versions, mount, path);
    Ok(())
}
