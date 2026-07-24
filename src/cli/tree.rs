use crate::cli::OutputArgs;
use crate::error::KiviError;
use crate::output::TreeNode;
use crate::output::{OutputFormat, resolve_value_mode};
use crate::vault::client::VaultClient;
use crate::vault::kv;
use clap::Args;
use std::sync::Arc;
use tokio::task::JoinSet;

const MAX_CONCURRENT: usize = 16;

#[derive(Args, Debug)]
pub struct TreeArgs {
    /// Root path for the tree (defaults to configured default_path)
    #[arg(add = clap_complete::engine::ArgValueCompleter::new(crate::completion::complete_vault_path))]
    pub path: Option<String>,

    /// Show only key names, no values
    #[arg(long)]
    pub keys_only: bool,

    /// Reveal actual secret values
    #[arg(long)]
    pub show_values: bool,

    /// Limit recursion depth
    #[arg(long)]
    pub depth: Option<usize>,

    /// Include soft-deleted secrets in output
    #[arg(long)]
    pub deleted: bool,

    #[command(flatten)]
    pub display: OutputArgs,
}

pub async fn run(
    args: &TreeArgs,
    client: Arc<VaultClient>,
    mount: &str,
    show_values_global: bool,
    path: &str,
) -> Result<(), KiviError> {
    let show = args.show_values || show_values_global;
    let mode = resolve_value_mode(show, args.keys_only);

    let root = build_tree(Arc::clone(&client), mount, path, args.depth).await?;
    let mut root = if args.keys_only {
        root
    } else {
        fetch_secrets(Arc::clone(&client), mount, root, path, args.deleted).await?
    };
    root.prune_empty();

    match args.display.output {
        OutputFormat::Human => crate::output::human::print_tree(path, &root, mode),
        OutputFormat::Json => crate::output::json::print_tree(&root, mode),
    }
    Ok(())
}

async fn build_tree(
    client: Arc<VaultClient>,
    mount: &str,
    path: &str,
    max_depth: Option<usize>,
) -> Result<TreeNode, KiviError> {
    let mut root = TreeNode::new();
    // (vault_path, tree_path_parts, current_depth)
    let mut queue: Vec<(String, Vec<String>, usize)> =
        vec![(path.trim_end_matches('/').to_string(), vec![], 0)];

    while !queue.is_empty() {
        // Fix: drain only up to MAX_CONCURRENT entries — drain(..).take(N) silently
        // drops the remaining unconsumed items when the Drain iterator is dropped.
        let n = queue.len().min(MAX_CONCURRENT);
        let batch: Vec<_> = queue.drain(..n).collect();

        type ListResult = Result<(Vec<String>, usize, Vec<String>), KiviError>;
        let mut set: JoinSet<ListResult> = JoinSet::new();
        for (vault_path, tree_parts, depth) in batch {
            let client = Arc::clone(&client);
            let mount = mount.to_string();
            set.spawn(async move {
                let keys = kv::list_keys(&client, &mount, &vault_path).await?;
                Ok((tree_parts, depth, keys))
            });
        }

        while let Some(res) = set.join_next().await {
            let (tree_parts, depth, keys) =
                res.map_err(|e| KiviError::TaskPanic(e.to_string()))??;

            for key in keys {
                let mut child_parts = tree_parts.clone();
                if key.ends_with('/') {
                    let dir_name = key.trim_end_matches('/').to_string();
                    child_parts.push(dir_name.clone());
                    if max_depth.is_none_or(|d| depth < d) {
                        let child_vault_path =
                            format!("{}/{}", path.trim_end_matches('/'), child_parts.join("/"));
                        queue.push((child_vault_path, child_parts, depth + 1));
                    }
                } else {
                    let mut leaf_parts = child_parts.clone();
                    leaf_parts.push(key.clone());
                    root.insert_path(
                        &leaf_parts.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                        Some(std::collections::HashMap::new()),
                    );
                }
            }
        }
    }

    Ok(root)
}

enum FetchResult {
    Live(crate::vault::types::KvReadData),
    NotFound,
}

async fn fetch_secrets(
    client: Arc<VaultClient>,
    mount: &str,
    mut root: TreeNode,
    base_path: &str,
    include_deleted: bool,
) -> Result<TreeNode, KiviError> {
    let mut leaf_paths: Vec<Vec<String>> = Vec::new();
    collect_leaves(&root, &mut vec![], &mut leaf_paths);

    let mut all_results: Vec<(Vec<String>, FetchResult)> = Vec::new();
    let mut remaining = leaf_paths;

    while !remaining.is_empty() {
        let n = remaining.len().min(MAX_CONCURRENT);
        let batch: Vec<_> = remaining.drain(..n).collect();

        let mut set: JoinSet<Result<(Vec<String>, FetchResult), KiviError>> = JoinSet::new();
        for parts in batch {
            let client = Arc::clone(&client);
            let mount = mount.to_string();
            let base = base_path.trim_end_matches('/').to_string();
            set.spawn(async move {
                let vault_path = format!("{}/{}", base, parts.join("/"));
                match kv::read(&client, &mount, &vault_path, None).await {
                    Ok(secret) => Ok((parts, FetchResult::Live(secret))),
                    Err(KiviError::NotFound { .. }) => Ok((parts, FetchResult::NotFound)),
                    Err(e) => Err(e),
                }
            });
        }

        while let Some(res) = set.join_next().await {
            let result = res.map_err(|e| KiviError::TaskPanic(e.to_string()))??;
            all_results.push(result);
        }
    }

    for (parts, result) in all_results {
        let refs: Vec<&str> = parts.iter().map(|s| s.as_str()).collect();
        match result {
            FetchResult::Live(secret) => {
                root.insert_path(&refs, Some(secret.data));
            }
            FetchResult::NotFound => {
                if include_deleted {
                    root.insert_deleted(&refs);
                } else {
                    root.remove_secret(&refs);
                }
            }
        }
    }

    Ok(root)
}

fn collect_leaves(node: &TreeNode, current: &mut Vec<String>, out: &mut Vec<Vec<String>>) {
    for (name, child) in &node.children {
        current.push(name.clone());
        collect_leaves(child, current, out);
        current.pop();
    }
    for name in node.secrets.keys() {
        let mut path = current.clone();
        path.push(name.clone());
        out.push(path);
    }
}
