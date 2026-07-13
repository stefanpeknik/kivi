use crate::output::{apply_value_mode, ValueMode, MASK};
use crate::vault::types::{FullMetadataData, SecretData};
use serde_json::{json, Value};
use std::collections::HashMap;

fn to_pretty(val: &Value) -> String {
    serde_json::to_string_pretty(val).unwrap_or_else(|_| format!("{val:?}"))
}

pub fn print_secret(path: &str, secret: &SecretData, mode: ValueMode) {
    let data = apply_value_mode(&secret.data, mode);
    println!("{}", to_pretty(&json!({
        "path": path,
        "version": secret.metadata.version,
        "data": data,
    })));
}

pub fn print_list(entries: &[String]) {
    println!("{}", to_pretty(&json!(entries)));
}

pub fn print_tree(node: &TreeNode, mode: ValueMode) {
    println!("{}", to_pretty(&node_to_json(node, mode)));
}

pub fn print_metadata(path: &str, meta: &FullMetadataData) {
    println!("{}", to_pretty(&json!({
        "path": path,
        "current_version": meta.current_version,
        "max_versions": meta.max_versions,
        "cas_required": meta.cas_required,
        "created_time": meta.created_time,
        "updated_time": meta.updated_time,
        "custom_metadata": meta.custom_metadata,
        "versions": meta.versions,
    })));
}


fn node_to_json(node: &TreeNode, mode: ValueMode) -> Value {
    let mut map = serde_json::Map::new();
    for (name, child) in &node.children {
        map.insert(name.clone(), node_to_json(child, mode));
    }
    for (name, secret) in &node.secrets {
        match mode {
            ValueMode::KeysOnly => {
                let keys: Vec<&str> = secret.keys().map(|s| s.as_str()).collect();
                map.insert(name.clone(), json!(keys));
            }
            ValueMode::Masked => {
                let masked: HashMap<_, _> =
                    secret.keys().map(|k| (k.as_str(), MASK)).collect();
                map.insert(name.clone(), json!(masked));
            }
            ValueMode::Revealed => {
                map.insert(name.clone(), json!(secret));
            }
        }
    }
    Value::Object(map)
}

#[derive(Default)]
pub struct TreeNode {
    pub children: std::collections::BTreeMap<String, TreeNode>,
    pub secrets: std::collections::BTreeMap<String, HashMap<String, Value>>,
}

impl TreeNode {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_path(&mut self, parts: &[&str], data: Option<HashMap<String, Value>>) {
        if parts.is_empty() {
            return;
        }
        if parts.len() == 1 {
            if let Some(d) = data {
                self.secrets.insert(parts[0].to_string(), d);
            }
            return;
        }
        let child = self.children.entry(parts[0].to_string()).or_default();
        child.insert_path(&parts[1..], data);
    }
}
