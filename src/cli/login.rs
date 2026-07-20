use crate::config::Config;
use crate::error::KiviError;
use clap::Args;

#[derive(Args, Debug)]
pub struct LoginArgs {
    /// OIDC auth method mount path in Vault
    #[arg(long)]
    pub oidc_mount: Option<String>,

    /// OIDC role to request
    #[arg(long)]
    pub role: Option<String>,

    /// Port for the localhost callback listener
    #[arg(long, default_value_t = 8250)]
    pub port: u16,
}

pub async fn run(args: &LoginArgs, address: &str, cfg: &Config) -> Result<(), KiviError> {
    let oidc_mount = args
        .oidc_mount
        .as_deref()
        .or(cfg.oidc_mount.as_deref())
        .unwrap_or("oidc");

    eprintln!("Opening browser for OIDC login...");
    eprintln!("Waiting for callback on http://localhost:{}...", args.port);

    let result =
        crate::vault::oidc::oidc_login(address, oidc_mount, args.role.as_deref(), args.port)
            .await?;

    crate::vault::token::write_token_file(&result.client_token)?;

    let token_path = crate::vault::token::token_file_path()
        .map_or_else(|| "~/.vault-token".to_string(), |p| p.display().to_string());

    eprintln!(
        "{} Login successful. Token written to {}",
        console::style("ok").bold().green(),
        token_path,
    );
    eprintln!(
        "Token is valid for {} (renewable: {})",
        format_duration(result.lease_duration),
        result.renewable,
    );

    Ok(())
}

fn format_duration(secs: u64) -> String {
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {mins}m")
    } else if mins > 0 {
        format!("{mins}m")
    } else {
        format!("{secs}s")
    }
}
