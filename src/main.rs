mod cli;
mod completion;
mod config;
mod editor;
mod error;
mod output;
mod vault;

use crate::cli::{Cli, Commands};
use crate::config::Config;
use crate::error::KiviError;
use crate::vault::client::VaultClient;
use clap::Parser;
use std::sync::Arc;

fn main() {
    // Must run before the tokio runtime starts. CompleteEnv detects COMPLETE=<shell>
    // in the environment, outputs candidates via our ArgValueCompleter, and exits.
    // Creating a tokio runtime inside the completer (Runtime::new()) would panic if
    // called from within an already-running runtime — hence the sync main().
    clap_complete::CompleteEnv::with_factory(<Cli as clap::CommandFactory>::command).complete();

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime")
        .block_on(async {
            if let Err(e) = run().await {
                eprintln!("{}: {}", console::style("error").bold().red(), e);
                std::process::exit(1);
            }
        });
}

async fn run() -> Result<(), KiviError> {
    let cli = Cli::parse();

    if cli.no_color {
        console::set_colors_enabled(false);
    }

    if let Commands::Config(args) = &cli.command {
        return cli::config::run(args);
    }

    let cfg = Config::load()?;

    if let Commands::Login(args) = &cli.command {
        let address = cli
            .vault
            .address
            .as_deref()
            .filter(|s| !s.is_empty())
            .or(cfg.address.as_deref())
            .ok_or(KiviError::NoAddress)?
            .to_string();
        return cli::login::run(args, &address, &cfg).await;
    }
    let (address, token, mount) = resolve_vault(&cli, &cfg)?;
    let client = Arc::new(VaultClient::new(
        &address,
        &token,
        cli.vault.namespace.as_deref(),
    ));
    let show_values_global = cfg.show_values;

    match &cli.command {
        Commands::Get(args) => {
            let path = cfg.resolve_path(args.path.as_deref())?;
            cli::get::run(args, &client, &mount, show_values_global, &path).await?
        }
        Commands::Diff(args) => {
            let path = cfg.resolve_path(args.path.as_deref())?;
            cli::diff::run(args, &client, &mount, show_values_global, &path).await?
        }
        Commands::List(args) => {
            let path = cfg.resolve_path(args.path.as_deref())?;
            cli::list::run(args, Arc::clone(&client), &mount, &path).await?
        }
        Commands::Tree(args) => {
            let path = cfg.resolve_path(args.path.as_deref())?;
            cli::tree::run(args, Arc::clone(&client), &mount, show_values_global, &path).await?
        }
        Commands::Put(args) => {
            let path = cfg.resolve_path(args.path.as_deref())?;
            cli::put::run(args, &client, &mount, &path).await?
        }
        Commands::Patch(args) => {
            let path = cfg.resolve_path(args.path.as_deref())?;
            cli::patch::run(args, &client, &mount, &path).await?
        }
        Commands::Edit(args) => {
            let path = cfg.resolve_path(args.path.as_deref())?;
            cli::edit::run(args, &client, &mount, &path).await?
        }
        Commands::Delete(args) => {
            let path = cfg.resolve_path(args.path.as_deref())?;
            cli::delete::run(args, Arc::clone(&client), &mount, &path).await?
        }
        Commands::Undelete(args) => {
            let path = cfg.resolve_path(args.path.as_deref())?;
            cli::undelete::run(args, &client, &mount, &path).await?
        }
        Commands::Metadata(args) => cli::metadata::run(args, &client, &mount, &cfg).await?,
        Commands::Config(_) | Commands::Login(_) => unreachable!(),
    }

    Ok(())
}

fn resolve_vault(cli: &Cli, cfg: &Config) -> Result<(String, String, String), KiviError> {
    let address = cli
        .vault
        .address
        .as_deref()
        .filter(|s| !s.is_empty())
        .or(cfg.address.as_deref())
        .ok_or(KiviError::NoAddress)?
        .to_string();

    let token = cli
        .vault
        .token
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .or_else(vault::token::read_token_file)
        .ok_or(KiviError::NoToken)?;

    let mount = cli
        .vault
        .mount
        .as_deref()
        .filter(|s| !s.is_empty())
        .or(cfg.mount.as_deref())
        .ok_or(KiviError::NoMount)?
        .trim_end_matches('/')
        .to_string();

    Ok((address, token, mount))
}
