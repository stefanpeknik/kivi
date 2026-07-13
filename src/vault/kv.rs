use crate::error::KiviError;
use crate::vault::client::VaultClient;
use crate::vault::types::*;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::task::JoinSet;

const MAX_CONCURRENT: usize = 16;

pub async fn read(
    client: &VaultClient,
    mount: &str,
    path: &str,
    version: Option<u64>,
) -> Result<KvReadData, KiviError> {
    let api_path = if let Some(v) = version {
        format!("{}/data/{}?version={}", mount, path, v)
    } else {
        format!("{}/data/{}", mount, path)
    };
    let raw = client.get(&api_path).await?;
    let resp: KvReadResponse = serde_json::from_value(raw)?;
    Ok(resp.data)
}

pub async fn list_keys(
    client: &VaultClient,
    mount: &str,
    path: &str,
) -> Result<Vec<String>, KiviError> {
    let api_path = format!("{}/metadata/{}", mount, path.trim_end_matches('/'));
    let raw = client.list(&api_path).await?;
    let resp: KvListResponse = serde_json::from_value(raw)?;
    Ok(resp.data.keys)
}

pub async fn write(
    client: &VaultClient,
    mount: &str,
    path: &str,
    data: HashMap<String, Value>,
    cas: Option<u64>,
) -> Result<u64, KiviError> {
    let api_path = format!("{}/data/{}", mount, path);
    let body = KvWriteRequest {
        data,
        options: cas.map(|c| KvWriteOptions { cas: c }),
    };
    let raw = client.post(&api_path, &body).await.map_err(|e| {
        // Vault returns HTTP 400 with a message containing "check-and-set" on CAS mismatch.
        // This is the only stable signal available; the Vault API has no structured error code.
        if let KiviError::Api {
            status: 400,
            ref message,
        } = e
            && (message.contains("check-and-set") || message.contains("CAS"))
        {
            return KiviError::CasConflict {
                path: path.to_string(),
                expected: cas.unwrap_or(0),
            };
        }
        e
    })?;
    let resp: KvWriteResponse = serde_json::from_value(raw)?;
    Ok(resp.data.version)
}

pub async fn patch(
    client: &VaultClient,
    mount: &str,
    path: &str,
    data: HashMap<String, Value>,
) -> Result<u64, KiviError> {
    let api_path = format!("{}/data/{}", mount, path);
    let body = json!({ "data": data });
    let raw = client.patch(&api_path, &body).await?;
    let resp: KvWriteResponse = serde_json::from_value(raw)?;
    Ok(resp.data.version)
}

pub async fn soft_delete(client: &VaultClient, mount: &str, path: &str) -> Result<(), KiviError> {
    client.delete(&format!("{}/data/{}", mount, path)).await
}

pub async fn delete_versions(
    client: &VaultClient,
    mount: &str,
    path: &str,
    versions: &[u64],
) -> Result<(), KiviError> {
    client
        .post(
            &format!("{}/delete/{}", mount, path),
            &json!({ "versions": versions }),
        )
        .await?;
    Ok(())
}

pub async fn undelete(
    client: &VaultClient,
    mount: &str,
    path: &str,
    versions: &[u64],
) -> Result<(), KiviError> {
    client
        .post(
            &format!("{}/undelete/{}", mount, path),
            &json!({ "versions": versions }),
        )
        .await?;
    Ok(())
}

pub async fn destroy(
    client: &VaultClient,
    mount: &str,
    path: &str,
    versions: &[u64],
) -> Result<(), KiviError> {
    client
        .post(
            &format!("{}/destroy/{}", mount, path),
            &json!({ "versions": versions }),
        )
        .await?;
    Ok(())
}

pub async fn read_metadata(
    client: &VaultClient,
    mount: &str,
    path: &str,
) -> Result<FullMetadataData, KiviError> {
    let raw = client.get(&format!("{}/metadata/{}", mount, path)).await?;
    let resp: FullMetadata = serde_json::from_value(raw)?;
    Ok(resp.data)
}

pub async fn write_metadata(
    client: &VaultClient,
    mount: &str,
    path: &str,
    opts: Value,
) -> Result<(), KiviError> {
    client
        .post(&format!("{}/metadata/{}", mount, path), &opts)
        .await?;
    Ok(())
}

pub async fn delete_metadata(
    client: &VaultClient,
    mount: &str,
    path: &str,
) -> Result<(), KiviError> {
    client.delete(&format!("{}/metadata/{}", mount, path)).await
}

// Recursive BFS listing with bounded concurrency. Returns all leaf paths sorted.
pub async fn list_recursive(
    client: Arc<VaultClient>,
    mount: &str,
    path: &str,
    max_depth: Option<usize>,
) -> Result<Vec<String>, KiviError> {
    let mut leaves = Vec::new();
    // (vault_path, current_depth)
    let mut queue: Vec<(String, usize)> = vec![(path.trim_end_matches('/').to_string(), 0)];

    while !queue.is_empty() {
        // Fix: only drain as many entries as we intend to process in this batch.
        // drain(..).take(N) drops the remaining unconsumed items — use drain(..n) instead.
        let n = queue.len().min(MAX_CONCURRENT);
        let batch: Vec<_> = queue.drain(..n).collect();

        type ListResult = Result<(String, usize, Vec<String>), KiviError>;
        let mut set: JoinSet<ListResult> = JoinSet::new();
        for (dir, depth) in batch {
            let client = Arc::clone(&client);
            let mount = mount.to_string();
            set.spawn(async move {
                let keys = list_keys(&client, &mount, &dir).await?;
                Ok((dir, depth, keys))
            });
        }

        while let Some(result) = set.join_next().await {
            let (dir, depth, keys) = result.map_err(|e| KiviError::TaskPanic(e.to_string()))??;
            for key in keys {
                if key.ends_with('/') {
                    let child = format!(
                        "{}/{}",
                        dir.trim_end_matches('/'),
                        key.trim_end_matches('/')
                    );
                    if max_depth.is_none_or(|d| depth < d) {
                        queue.push((child, depth + 1));
                    }
                } else {
                    leaves.push(format!("{}/{}", dir.trim_end_matches('/'), key));
                }
            }
        }
    }

    leaves.sort();
    Ok(leaves)
}
