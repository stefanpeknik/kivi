use crate::output::{ValueMode, MASK};
use crate::output::TreeNode;
use crate::vault::types::{FullMetadataData, KvReadData};
use comfy_table::{Attribute, Cell, Color, Table, presets::NOTHING};
use console::style;
use ptree::TreeBuilder;
use serde_json::Value;

fn display_value(v: &Value) -> String {
    v.as_str().map(|s| s.to_string()).unwrap_or_else(|| v.to_string())
}

pub fn print_secret(path: &str, secret: &KvReadData, mode: ValueMode) {
    println!("{}", style(format!("Path:    {path}")).bold());
    println!("{}", style(format!("Version: {}", secret.metadata.version)).dim());
    println!();

    if matches!(mode, ValueMode::KeysOnly) {
        let mut keys: Vec<&str> = secret.data.keys().map(|s| s.as_str()).collect();
        keys.sort();
        println!("{}", keys.join(", "));
        return;
    }

    let mut table = Table::new();
    table.load_preset(NOTHING);

    let mut keys: Vec<&str> = secret.data.keys().map(|s| s.as_str()).collect();
    keys.sort();

    for key in keys {
        let val = match mode {
            ValueMode::Masked => style(MASK).dim().to_string(),
            ValueMode::Revealed => display_value(&secret.data[key]),
            ValueMode::KeysOnly => unreachable!(),
        };
        table.add_row(vec![
            Cell::new(key)
                .add_attribute(Attribute::Bold)
                .fg(Color::Cyan),
            Cell::new(val),
        ]);
    }

    println!("{table}");
}


pub fn print_tree(root_path: &str, node: &TreeNode, mode: ValueMode) {
    let root_label = style(format!("{root_path}/")).bold().blue().to_string();
    let mut builder = TreeBuilder::new(root_label);
    add_node_to_builder(&mut builder, node, mode);
    let tree = builder.build();
    ptree::print_tree(&tree).unwrap_or_else(|_| {
        // fallback: just print paths
        print_tree_fallback(root_path, node, mode);
    });
}

fn add_node_to_builder(builder: &mut TreeBuilder, node: &TreeNode, mode: ValueMode) {
    for (name, child) in &node.children {
        let label = style(format!("{name}/")).bold().blue().to_string();
        builder.begin_child(label);
        add_node_to_builder(builder, child, mode);
        builder.end_child();
    }

    for (name, data) in &node.secrets {
        match mode {
            ValueMode::KeysOnly => {
                let mut keys: Vec<&str> = data.keys().map(|s| s.as_str()).collect();
                keys.sort();
                let label = format!(
                    "{}  {}",
                    style(name).bold(),
                    style(format!("[{}]", keys.join(", "))).dim()
                );
                builder.add_empty_child(label);
            }
            _ => {
                builder.begin_child(style(name).bold().to_string());
                let mut keys: Vec<&str> = data.keys().map(|s| s.as_str()).collect();
                keys.sort();
                for key in keys {
                    let val = match mode {
                        ValueMode::Masked => style(MASK).dim().to_string(),
                        ValueMode::Revealed => display_value(&data[key]),
                        ValueMode::KeysOnly => unreachable!(),
                    };
                    builder.add_empty_child(format!("{}: {}", style(key).cyan(), val));
                }
                builder.end_child();
            }
        }
    }
}

fn print_tree_fallback(root_path: &str, node: &TreeNode, mode: ValueMode) {
    println!("{}/", root_path);
    print_node_fallback(node, mode, "");
}

fn print_node_fallback(node: &TreeNode, mode: ValueMode, prefix: &str) {
    for (name, child) in &node.children {
        println!("{prefix}  {name}/");
        print_node_fallback(child, mode, &format!("{prefix}  "));
    }
    for (name, data) in &node.secrets {
        let mut keys: Vec<&str> = data.keys().map(|s| s.as_str()).collect();
        keys.sort();
        match mode {
            ValueMode::KeysOnly => println!("{prefix}  {name} [{}]", keys.join(", ")),
            _ => {
                println!("{prefix}  {name}");
                for key in keys {
                    let val = match mode {
                        ValueMode::Masked => MASK.to_string(),
                        ValueMode::Revealed => display_value(&data[key]),
                        ValueMode::KeysOnly => unreachable!(),
                    };
                    println!("{prefix}    {key}: {val}");
                }
            }
        }
    }
}

pub fn print_metadata(path: &str, meta: &FullMetadataData) {
    println!("{}", style(format!("Path:            {path}")).bold());
    println!("Current version: {}", meta.current_version);
    println!("Max versions:    {}", meta.max_versions);
    println!("CAS required:    {}", meta.cas_required);
    println!("Created:         {}", meta.created_time);
    println!("Updated:         {}", meta.updated_time);
    if let Some(ref dva) = meta.delete_version_after {
        if !dva.is_empty() && dva != "0s" {
            println!("Delete after:    {}", dva);
        }
    }

    if let Some(ref cm) = meta.custom_metadata {
        if !cm.is_empty() {
            println!("\nCustom metadata:");
            for (k, v) in cm {
                println!("  {}: {}", style(k).cyan(), v);
            }
        }
    }

    println!("\nVersions:");
    let mut sorted_versions: Vec<(&String, &crate::vault::types::VersionInfo)> =
        meta.versions.iter().collect();
    sorted_versions.sort_by_key(|(k, _)| k.parse::<u64>().unwrap_or(0));

    let mut table = Table::new();
    table.load_preset(NOTHING);
    table.set_header(vec![
        Cell::new("Version").add_attribute(Attribute::Bold),
        Cell::new("Created").add_attribute(Attribute::Bold),
        Cell::new("Deleted").add_attribute(Attribute::Bold),
        Cell::new("Destroyed").add_attribute(Attribute::Bold),
    ]);
    for (ver, info) in sorted_versions {
        let deleted = info.deletion_time.as_deref().filter(|s| !s.is_empty()).unwrap_or("-");
        let destroyed = if info.destroyed { style("yes").red().to_string() } else { "-".to_string() };
        table.add_row(vec![
            Cell::new(ver),
            Cell::new(&info.created_time),
            Cell::new(deleted),
            Cell::new(destroyed),
        ]);
    }
    println!("{table}");
}

