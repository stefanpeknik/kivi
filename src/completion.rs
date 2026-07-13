use crate::vault::kv;
use clap_complete::engine::CompletionCandidate;
use std::ffi::OsStr;
use std::time::Duration;

const COMPLETION_TIMEOUT: Duration = Duration::from_secs(10);

/// Sync completer registered on every <path> argument via ArgValueCompleter.
/// clap_complete's engine calls this on every Tab press.
/// Bridges into async via a single-shot runtime — acceptable here because this
/// path only runs during shell completion, never during normal execution.
pub fn complete_vault_path(current: &OsStr) -> Vec<CompletionCandidate> {
    let partial = current.to_str().unwrap_or("");

    // Read connection settings from env vars + config file.
    // Parsed CLI args aren't available here — we're inside the completion
    // engine, which runs before clap's normal argument parsing.
    let cfg = crate::config::Config::load().unwrap_or_default();

    let address = std::env::var("VAULT_ADDR")
        .ok()
        .filter(|s| !s.is_empty())
        .or(cfg.address.clone());

    let token = std::env::var("VAULT_TOKEN")
        .ok()
        .filter(|s| !s.is_empty());

    let mount = cfg.mount.clone();

    let (address, token, mount) = match (address, token, mount) {
        (Some(a), Some(t), Some(m)) => (a, t, m),
        _ => return vec![],
    };

    let client = crate::vault::client::VaultClient::new(&address, &token, None);

    let rt = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(_) => return vec![],
    };

    let default_path = cfg.default_path.as_deref().filter(|s| !s.is_empty());

    let entries = rt.block_on(async {
        tokio::time::timeout(COMPLETION_TIMEOUT, fetch(partial, &client, &mount, default_path))
            .await
            .unwrap_or_default()
    });

    entries.into_iter().map(CompletionCandidate::new).collect()
}

async fn fetch(
    partial: &str,
    client: &crate::vault::client::VaultClient,
    mount: &str,
    default_path: Option<&str>,
) -> Vec<String> {
    let (parent, prefix) = match partial.rfind('/') {
        Some(idx) => (&partial[..idx], &partial[idx + 1..]),
        None => ("", partial),
    };

    let api_parent = match default_path {
        Some(dp) if parent.is_empty() => dp.to_string(),
        Some(dp) => format!("{}/{}", dp.trim_end_matches('/'), parent),
        None => parent.to_string(),
    };

    let keys = match kv::list_keys(client, mount, &api_parent).await {
        Ok(k) => k,
        Err(_) => return vec![],
    };

    keys.into_iter()
        .filter(|k| k.starts_with(prefix))
        .map(|k| if parent.is_empty() { k } else { format!("{parent}/{k}") })
        .collect()
}
