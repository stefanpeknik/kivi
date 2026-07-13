use crate::error::KiviError;
use crate::vault::client::VaultClient;
use crate::vault::kv;
use clap::Args;
use console::style;
use similar::{ChangeTag, InlineChange, TextDiff};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

#[derive(Args, Debug)]
pub struct DiffArgs {
    /// Path to the secret (uses default_path from config if omitted)
    #[arg(add = clap_complete::engine::ArgValueCompleter::new(crate::completion::complete_vault_path))]
    pub path: Option<String>,

    /// Compare from this version (default: previous version)
    #[arg(long)]
    pub from: Option<u64>,

    /// Compare to this version (default: latest version)
    #[arg(long)]
    pub to: Option<u64>,

    /// Show actual secret values and character-level diffs
    #[arg(long)]
    pub show_values: bool,
}

pub async fn run(
    args: &DiffArgs,
    client: &VaultClient,
    mount: &str,
    show_values_global: bool,
    path: &str,
) -> Result<(), KiviError> {
    let show = args.show_values || show_values_global;

    let (from_version, to_version) = resolve_versions(args, client, mount, path).await?;

    if from_version == to_version {
        println!("No diff: both versions are the same (v{from_version}).");
        return Ok(());
    }

    let (from_secret, to_secret) = tokio::try_join!(
        kv::read(client, mount, path, Some(from_version)),
        kv::read(client, mount, path, Some(to_version)),
    )?;

    println!(
        "{}",
        style(format!("{path}  (v{from_version} → v{to_version})")).bold()
    );
    println!();

    if show {
        print_text_diff(&from_secret.data, &to_secret.data);
    } else {
        print_structural_diff(&from_secret.data, &to_secret.data);
    }

    Ok(())
}

/// Default mode: show which keys changed, no values.
/// Colors degrade gracefully when piped — +/~/- carry the meaning.
fn print_structural_diff(from: &HashMap<String, Value>, to: &HashMap<String, Value>) {
    let mut added: Vec<&str> = vec![];
    let mut removed: Vec<&str> = vec![];
    let mut changed: Vec<&str> = vec![];

    let mut all_keys: Vec<&str> = from.keys().chain(to.keys()).map(|s| s.as_str()).collect();
    all_keys.sort();
    all_keys.dedup();

    for key in &all_keys {
        match (from.get(*key), to.get(*key)) {
            (None, Some(_))                             => added.push(key),
            (Some(_), None)                             => removed.push(key),
            (Some(f), Some(t)) if f != t                => changed.push(key),
            _                                           => {} // unchanged — not shown
        }
    }

    if added.is_empty() && removed.is_empty() && changed.is_empty() {
        println!("{}", style("No changes.").dim());
        return;
    }

    for key in &added   { println!("  {}  {}", style("+").bold().green(),  style(key).green()); }
    for key in &changed { println!("  {}  {}", style("~").bold().yellow(), style(key).yellow()); }
    for key in &removed { println!("  {}  {}", style("-").bold().red(),    style(key).red()); }

    println!();
    println!(
        "{}",
        style(format!(
            "{} addition{}, {} change{}, {} removal{}",
            added.len(),   if added.len()   == 1 { "" } else { "s" },
            changed.len(), if changed.len() == 1 { "" } else { "s" },
            removed.len(), if removed.len() == 1 { "" } else { "s" },
        )).dim()
    );
}

/// --show-values mode: full git-style text diff of the JSON with character-level emphasis.
/// Keys are sorted (BTreeMap) so the diff is stable across invocations.
fn print_text_diff(from: &HashMap<String, Value>, to: &HashMap<String, Value>) {
    let to_sorted_json = |data: &HashMap<String, Value>| -> String {
        let sorted: BTreeMap<_, _> = data.iter().collect();
        serde_json::to_string_pretty(&sorted).unwrap_or_default()
    };

    let from_json = to_sorted_json(from);
    let to_json   = to_sorted_json(to);

    if from_json == to_json {
        println!("{}", style("No changes.").dim());
        return;
    }

    let diff = TextDiff::from_lines(&from_json, &to_json);

    // 3 lines of context around each change — same as git diff default
    for group in diff.grouped_ops(3) {
        for op in &group {
            for change in diff.iter_inline_changes(op) {
                print_inline_change(&change);
            }
        }
        println!("{}", style("---").dim()); // separator between hunks
    }
}

fn print_inline_change(change: &InlineChange<str>) {
    let (sign, line_style): (&str, fn(&str) -> String) = match change.tag() {
        ChangeTag::Delete => ("-", |s| style(s).red().to_string()),
        ChangeTag::Insert => ("+", |s| style(s).green().to_string()),
        ChangeTag::Equal  => (" ", |s| s.to_string()),
    };

    print!("{sign}");
    for (emphasized, text) in change.iter_strings_lossy() {
        if emphasized {
            // Character-level change within the line — bold + underline
            let highlighted = match change.tag() {
                ChangeTag::Delete => style(text.as_ref()).red().bold().underlined().to_string(),
                ChangeTag::Insert => style(text.as_ref()).green().bold().underlined().to_string(),
                ChangeTag::Equal  => text.into_owned(),
            };
            print!("{highlighted}");
        } else {
            print!("{}", line_style(text.as_ref()));
        }
    }
    if change.missing_newline() {
        println!();
    }
}

async fn resolve_versions(
    args: &DiffArgs,
    client: &VaultClient,
    mount: &str,
    path: &str,
) -> Result<(u64, u64), KiviError> {
    // If both --from and --to are explicit, no API call needed
    if let (Some(from), Some(to)) = (args.from, args.to) {
        return Ok((from, to));
    }

    // Otherwise fetch the current version to resolve the defaults
    let current = kv::read(client, mount, path, None).await?;
    let latest = current.metadata.version;

    let to = args.to.unwrap_or(latest);
    let from = match args.from {
        Some(f) => f,
        None => {
            if to < 2 {
                return Err(KiviError::Config(
                    "No previous version to compare — this secret only has one version.".to_string(),
                ));
            }
            to - 1
        }
    };

    Ok((from, to))
}
