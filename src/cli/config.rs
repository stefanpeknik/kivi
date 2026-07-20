use crate::config::Config;
use crate::error::KiviError;
use clap::{Args, Subcommand};
use toml;

#[derive(Args, Debug)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommand {
    /// Display current configuration
    Show,
    /// Update one or more configuration values
    Set(ConfigSetArgs),
    /// Remove optional configuration values (revert to defaults)
    Unset(ConfigUnsetArgs),
    /// Print the config file path
    Path,
}

#[derive(Args, Debug)]
pub struct ConfigSetArgs {
    /// Vault server address
    #[arg(long)]
    pub address: Option<String>,

    /// KV engine mount point
    #[arg(long)]
    pub mount: Option<String>,

    /// Default path prefix for all commands
    #[arg(long)]
    pub default_path: Option<String>,

    /// Show secret values by default instead of masking
    #[arg(long)]
    pub show_values: Option<bool>,

    /// OIDC auth method mount path in Vault
    #[arg(long)]
    pub oidc_mount: Option<String>,
}

#[derive(Args, Debug)]
pub struct ConfigUnsetArgs {
    /// Remove configured Vault server address
    #[arg(long)]
    pub address: bool,

    /// Remove KV engine mount point
    #[arg(long)]
    pub mount: bool,

    /// Remove default path prefix
    #[arg(long)]
    pub default_path: bool,

    /// Remove OIDC auth method mount path
    #[arg(long)]
    pub oidc_mount: bool,
}

pub fn run(args: &ConfigArgs) -> Result<(), KiviError> {
    match &args.command {
        ConfigCommand::Show => {
            let config = Config::load()?;
            let toml =
                toml::to_string_pretty(&config).map_err(|e| KiviError::Config(e.to_string()))?;
            print!("{toml}");
        }
        ConfigCommand::Set(a) => {
            if a.address.is_none()
                && a.mount.is_none()
                && a.default_path.is_none()
                && a.show_values.is_none()
                && a.oidc_mount.is_none()
            {
                return Err(KiviError::Usage(
                    "No values provided. Use --address, --mount, --default-path, --show-values, or --oidc-mount."
                        .to_string(),
                ));
            }
            let mut config = Config::load()?;
            let to_opt = |s: &String| if s.is_empty() { None } else { Some(s.clone()) };
            if let Some(ref v) = a.address {
                config.address = to_opt(v);
            }
            if let Some(ref v) = a.mount {
                config.mount = to_opt(v);
            }
            if let Some(ref v) = a.default_path {
                config.default_path = to_opt(v);
            }
            if let Some(v) = a.show_values {
                config.show_values = v;
            }
            if let Some(ref v) = a.oidc_mount {
                config.oidc_mount = to_opt(v);
            }
            config.save()?;
            println!("Config updated: {}", Config::path().display());
        }
        ConfigCommand::Unset(a) => {
            if !a.address && !a.mount && !a.default_path && !a.oidc_mount {
                return Err(KiviError::Usage(
                    "No values specified. Use --address, --mount, --default-path, or --oidc-mount."
                        .to_string(),
                ));
            }
            let mut config = Config::load()?;
            if a.address {
                config.address = None;
            }
            if a.mount {
                config.mount = None;
            }
            if a.default_path {
                config.default_path = None;
            }
            if a.oidc_mount {
                config.oidc_mount = None;
            }
            config.save()?;
            println!("Config updated: {}", Config::path().display());
        }
        ConfigCommand::Path => {
            println!("{}", Config::path().display());
        }
    }
    Ok(())
}
