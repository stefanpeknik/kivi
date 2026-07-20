pub mod config;
pub mod delete;
pub mod diff;
pub mod edit;
pub mod get;
pub mod list;
pub mod login;
pub mod metadata;
pub mod patch;
pub mod put;
pub mod tree;
pub mod undelete;

use crate::output::OutputFormat;
use clap::{Args, Parser, Subcommand};

/// Vault connection settings — applied before the subcommand, like `git -C` or `kubectl -n`.
/// Normally these come from ~/.config/kivi/config.toml; use flags to override per-invocation.
#[derive(Args, Debug, Clone)]
pub struct VaultArgs {
    /// Vault server address
    #[arg(short = 'a', long, env = "VAULT_ADDR")]
    pub address: Option<String>,

    /// Vault token
    #[arg(short = 't', long, env = "VAULT_TOKEN", hide_env_values = true)]
    pub token: Option<String>,

    /// KV engine mount point
    #[arg(short = 'm', long)]
    pub mount: Option<String>,

    /// Vault namespace (Vault Enterprise)
    #[arg(short = 'n', long, env = "VAULT_NAMESPACE")]
    pub namespace: Option<String>,
}

/// Output format flag — per command since different commands may use different formats.
#[derive(Args, Debug, Clone)]
pub struct OutputArgs {
    /// Output format
    #[arg(short = 'o', long, value_enum, default_value = "human")]
    pub output: OutputFormat,
}

#[derive(Parser, Debug)]
#[command(name = "kivi", about = "HashiCorp Vault KV secret manager", version)]
pub struct Cli {
    /// Vault connection settings (override config file values)
    #[command(flatten)]
    pub vault: VaultArgs,

    /// Disable colored output
    #[arg(long)]
    pub no_color: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Read a secret at a given path
    Get(get::GetArgs),
    /// List secrets/directories at a path
    #[command(alias = "ls")]
    List(list::ListArgs),
    /// Recursively list the full secret tree from a path
    Tree(tree::TreeArgs),
    /// Create or overwrite a secret
    #[command(alias = "set", alias = "write")]
    Put(put::PutArgs),
    /// Update specific keys in a secret without overwriting others
    Patch(patch::PatchArgs),
    /// Open a secret in $EDITOR for interactive editing
    Edit(edit::EditArgs),
    /// Compare two versions of a secret
    Diff(diff::DiffArgs),
    /// Delete a secret or recursively delete a subtree
    #[command(alias = "rm")]
    Delete(delete::DeleteArgs),
    /// Recover a soft-deleted secret version
    Undelete(undelete::UndeleteArgs),
    /// View or manage secret metadata and versions
    Metadata(metadata::MetadataArgs),
    /// Manage persistent CLI configuration
    Config(config::ConfigArgs),
    /// Authenticate via OIDC and store the token
    Login(login::LoginArgs),
}
