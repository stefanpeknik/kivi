use crate::error::KiviError;
use crate::vault::client::VaultClient;
use crate::vault::kv;
use clap::Args;
use console::style;
use dialoguer::Confirm;
use std::sync::Arc;
use tokio::task::JoinSet;

#[derive(Args, Debug)]
pub struct DeleteArgs {
    /// Path to delete
    #[arg(add = clap_complete::engine::ArgValueCompleter::new(crate::completion::complete_vault_path))]
    pub path: Option<String>,

    /// Recursively delete all secrets under the path
    #[arg(short = 'r', long)]
    pub recursive: bool,

    /// Skip confirmation prompt
    #[arg(short = 'f', long)]
    pub force: bool,

    /// Delete specific versions only (comma-separated)
    #[arg(long, value_delimiter = ',')]
    pub versions: Option<Vec<u64>>,

    /// Permanently destroy versions (cannot be undeleted)
    #[arg(long)]
    pub destroy: bool,

    /// Show what would be deleted without acting
    #[arg(long)]
    pub dry_run: bool,
}

pub async fn run(
    args: &DeleteArgs,
    client: Arc<VaultClient>,
    mount: &str,
    path: &str,
) -> Result<(), KiviError> {
    if args.recursive {
        run_recursive(args, client, mount, path).await
    } else {
        run_single(args, &client, mount, path).await
    }
}

async fn run_single(
    args: &DeleteArgs,
    client: &VaultClient,
    mount: &str,
    path: &str,
) -> Result<(), KiviError> {
    if args.dry_run {
        eprintln!(
            "{}",
            style(format!("[dry-run] Would delete {}/{}", mount, path)).yellow()
        );
        return Ok(());
    }
    if !args.force {
        let action = if args.destroy {
            "permanently destroy"
        } else {
            "delete"
        };
        let confirmed = Confirm::new()
            .with_prompt(format!("{} {}/{}?", action, mount, path))
            .default(false)
            .interact()
            .map_err(|dialoguer::Error::IO(e)| e)?;
        if !confirmed {
            println!("Aborted.");
            return Ok(());
        }
    }
    if args.destroy {
        if !args.force {
            let confirmed = Confirm::new()
                .with_prompt(
                    "This permanently destroys the secret and CANNOT be undone. Are you sure?",
                )
                .default(false)
                .interact()
                .map_err(|dialoguer::Error::IO(e)| e)?;
            if !confirmed {
                println!("Aborted.");
                return Ok(());
            }
        }
        let versions = if let Some(ref v) = args.versions {
            v.clone()
        } else {
            let meta = kv::read_metadata(client, mount, path).await?;
            meta.versions
                .keys()
                .filter_map(|v| v.parse().ok())
                .collect()
        };
        kv::destroy(client, mount, path, &versions).await?;
        println!("Destroyed: {}/{}", mount, path);
    } else if let Some(ref versions) = args.versions {
        kv::delete_versions(client, mount, path, versions).await?;
        println!("Deleted versions {:?} of {}/{}", versions, mount, path);
    } else {
        kv::soft_delete(client, mount, path).await?;
        println!("Deleted: {}/{}", mount, path);
    }
    Ok(())
}

async fn run_recursive(
    args: &DeleteArgs,
    client: Arc<VaultClient>,
    mount: &str,
    path: &str,
) -> Result<(), KiviError> {
    let leaves = kv::list_recursive(Arc::clone(&client), mount, path, None).await?;

    if leaves.is_empty() {
        println!("No secrets found under {}/{}", mount, path);
        return Ok(());
    }

    println!("The following secrets would be deleted:");
    for leaf in &leaves {
        println!("  {}", style(leaf).red());
    }

    if args.dry_run {
        eprintln!(
            "{}",
            style(format!(
                "[dry-run] {} secrets listed — no deletions performed.",
                leaves.len()
            ))
            .yellow()
        );
        return Ok(());
    }

    if !args.force {
        let action = if args.destroy {
            "permanently destroy"
        } else {
            "delete"
        };
        let confirmed = Confirm::new()
            .with_prompt(format!("{} {} secrets?", action, leaves.len()))
            .default(false)
            .interact()
            .map_err(|dialoguer::Error::IO(e)| e)?;
        if !confirmed {
            println!("Aborted.");
            return Ok(());
        }
        if args.destroy {
            let confirmed = Confirm::new()
                .with_prompt("This PERMANENTLY DESTROYS all versions and CANNOT be undone. Are you absolutely sure?")
                .default(false)
                .interact().map_err(|dialoguer::Error::IO(e)| e)?;
            if !confirmed {
                println!("Aborted.");
                return Ok(());
            }
        }
    }

    const MAX_CONCURRENT: usize = 16;
    let mut errors = Vec::new();
    let mut remaining = leaves;

    while !remaining.is_empty() {
        let n = remaining.len().min(MAX_CONCURRENT);
        let batch: Vec<_> = remaining.drain(..n).collect();
        let mut set: JoinSet<Result<(), KiviError>> = JoinSet::new();

        for leaf in batch {
            let client = Arc::clone(&client);
            let mount = mount.to_string();
            let destroy = args.destroy;
            set.spawn(async move {
                if destroy {
                    let meta = kv::read_metadata(&client, &mount, &leaf).await?;
                    let versions: Vec<u64> = meta
                        .versions
                        .keys()
                        .filter_map(|v| v.parse().ok())
                        .collect();
                    kv::destroy(&client, &mount, &leaf, &versions).await
                } else {
                    kv::soft_delete(&client, &mount, &leaf).await
                }
            });
        }

        while let Some(res) = set.join_next().await {
            match res {
                Ok(Ok(())) => {}
                Ok(Err(e)) => errors.push(e.to_string()),
                Err(e) => errors.push(e.to_string()),
            }
        }
    }

    if errors.is_empty() {
        println!("Done.");
        Ok(())
    } else {
        eprintln!("Completed with {} error(s):", errors.len());
        for e in &errors {
            eprintln!("  {}", style(e).red());
        }
        Err(KiviError::Usage(format!(
            "{} deletion(s) failed",
            errors.len()
        )))
    }
}
