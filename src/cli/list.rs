use crate::cli::OutputArgs;
use crate::error::KiviError;
use crate::output::OutputFormat;
use crate::vault::client::VaultClient;
use crate::vault::kv;
use crate::vault::types::FullMetadataData;
use clap::Args;
use comfy_table::{Attribute, Cell, Color, Table, presets::NOTHING};
use console::style;
use std::sync::Arc;
use tokio::task::JoinSet;

const MAX_CONCURRENT: usize = 16;

#[derive(Args, Debug)]
pub struct ListArgs {
    /// Path to list (defaults to configured default_path)
    #[arg(add = clap_complete::engine::ArgValueCompleter::new(crate::completion::complete_vault_path))]
    pub path: Option<String>,

    /// Show metadata for each entry (version, created time)
    #[arg(short = 'l', long)]
    pub long: bool,

    /// Include soft-deleted secrets in output
    #[arg(long)]
    pub deleted: bool,

    #[command(flatten)]
    pub display: OutputArgs,
}

pub async fn run(
    args: &ListArgs,
    client: Arc<VaultClient>,
    mount: &str,
    path: &str,
) -> Result<(), KiviError> {
    let entries = kv::list_keys(&client, mount, path).await?;

    let need_metadata = args.long || args.deleted;

    if !need_metadata {
        match args.display.output {
            OutputFormat::Human => print_list_human(&entries),
            OutputFormat::Json => crate::output::json::print_list(&entries),
        }
        return Ok(());
    }

    let leaves: Vec<&str> = entries
        .iter()
        .filter(|e| !e.ends_with('/'))
        .map(|s| s.as_str())
        .collect();
    let dirs: Vec<&str> = entries
        .iter()
        .filter(|e| e.ends_with('/'))
        .map(|s| s.as_str())
        .collect();

    let mut meta_results = fetch_metadata(Arc::clone(&client), mount, path, &leaves).await?;
    meta_results.sort_by(|a, b| a.0.cmp(&b.0));

    if args.long {
        match args.display.output {
            OutputFormat::Human => print_list_long_human(&dirs, &meta_results, args.deleted),
            OutputFormat::Json => print_list_long_json(&dirs, &meta_results, args.deleted),
        }
    } else {
        // --deleted without --long: annotate names only
        match args.display.output {
            OutputFormat::Human => print_list_deleted_human(&dirs, &meta_results),
            OutputFormat::Json => print_list_deleted_json(&dirs, &meta_results),
        }
    }

    Ok(())
}

async fn fetch_metadata(
    client: Arc<VaultClient>,
    mount: &str,
    path: &str,
    leaves: &[&str],
) -> Result<Vec<(String, Result<FullMetadataData, KiviError>)>, KiviError> {
    let mut meta_results: Vec<(String, Result<FullMetadataData, KiviError>)> = Vec::new();
    let mut remaining: Vec<String> = leaves.iter().map(|s| s.to_string()).collect();

    while !remaining.is_empty() {
        let n = remaining.len().min(MAX_CONCURRENT);
        let batch: Vec<String> = remaining.drain(..n).collect();
        let mut set: JoinSet<(String, Result<FullMetadataData, KiviError>)> = JoinSet::new();

        for leaf in batch {
            let client = Arc::clone(&client);
            let mount = mount.to_string();
            let full_path = format!("{}/{}", path.trim_end_matches('/'), leaf);
            set.spawn(async move {
                let result = kv::read_metadata(&client, &mount, &full_path).await;
                (leaf, result)
            });
        }

        while let Some(res) = set.join_next().await {
            meta_results.push(res.map_err(|e| KiviError::TaskPanic(e.to_string()))?);
        }
    }

    Ok(meta_results)
}

fn print_list_human(entries: &[String]) {
    for entry in entries {
        if entry.ends_with('/') {
            println!("{}", style(entry).bold().blue());
        } else {
            println!("{entry}");
        }
    }
}

fn is_deleted(meta: &Result<FullMetadataData, KiviError>) -> bool {
    meta.as_ref().is_ok_and(|m| m.is_current_version_deleted())
}

fn print_list_long_human(
    dirs: &[&str],
    leaves: &[(String, Result<FullMetadataData, KiviError>)],
    show_deleted: bool,
) {
    let mut table = Table::new();
    table.load_preset(NOTHING);
    table.set_header(vec![
        Cell::new("Name").add_attribute(Attribute::Bold),
        Cell::new("Type").add_attribute(Attribute::Bold),
        Cell::new("Version").add_attribute(Attribute::Bold),
        Cell::new("Updated").add_attribute(Attribute::Bold),
    ]);

    for dir in dirs {
        table.add_row(vec![
            Cell::new(dir)
                .fg(Color::Blue)
                .add_attribute(Attribute::Bold),
            Cell::new("dir"),
            Cell::new("-"),
            Cell::new("-"),
        ]);
    }
    for (name, meta) in leaves {
        let deleted = is_deleted(meta);
        if deleted && !show_deleted {
            continue;
        }
        let (version, updated) = match meta {
            Ok(m) => (m.current_version.to_string(), m.updated_time.clone()),
            Err(_) => ("-".to_string(), "-".to_string()),
        };
        if deleted {
            table.add_row(vec![
                Cell::new(format!("{name} [deleted]")).add_attribute(Attribute::Dim),
                Cell::new("secret").add_attribute(Attribute::Dim),
                Cell::new(version).add_attribute(Attribute::Dim),
                Cell::new(updated).add_attribute(Attribute::Dim),
            ]);
        } else {
            table.add_row(vec![
                Cell::new(name),
                Cell::new("secret"),
                Cell::new(version),
                Cell::new(updated),
            ]);
        }
    }
    println!("{table}");
}

fn print_list_long_json(
    dirs: &[&str],
    leaves: &[(String, Result<FullMetadataData, KiviError>)],
    show_deleted: bool,
) {
    let mut entries = serde_json::Map::new();
    for dir in dirs {
        entries.insert(dir.to_string(), serde_json::json!({"type": "dir"}));
    }
    for (name, meta) in leaves {
        let deleted = is_deleted(meta);
        if deleted && !show_deleted {
            continue;
        }
        let val = match meta {
            Ok(m) => {
                let mut obj = serde_json::json!({
                    "type": "secret",
                    "current_version": m.current_version,
                    "updated_time": m.updated_time,
                });
                if deleted {
                    obj["deleted"] = serde_json::json!(true);
                }
                obj
            }
            Err(e) => serde_json::json!({"type": "secret", "error": e.to_string()}),
        };
        entries.insert(name.clone(), val);
    }
    let val = serde_json::Value::Object(entries);
    println!(
        "{}",
        serde_json::to_string_pretty(&val).unwrap_or_else(|_| format!("{val:?}"))
    );
}

fn print_list_deleted_human(
    dirs: &[&str],
    leaves: &[(String, Result<FullMetadataData, KiviError>)],
) {
    for dir in dirs {
        println!("{}", style(dir).bold().blue());
    }
    for (name, meta) in leaves {
        if is_deleted(meta) {
            println!("{}", style(format!("{name} [deleted]")).dim());
        } else {
            println!("{name}");
        }
    }
}

fn print_list_deleted_json(
    dirs: &[&str],
    leaves: &[(String, Result<FullMetadataData, KiviError>)],
) {
    let mut entries = serde_json::Map::new();
    for dir in dirs {
        entries.insert(dir.to_string(), serde_json::json!({"type": "dir"}));
    }
    for (name, meta) in leaves {
        let deleted = is_deleted(meta);
        entries.insert(
            name.clone(),
            serde_json::json!({"type": "secret", "deleted": deleted}),
        );
    }
    let val = serde_json::Value::Object(entries);
    println!(
        "{}",
        serde_json::to_string_pretty(&val).unwrap_or_else(|_| format!("{val:?}"))
    );
}
