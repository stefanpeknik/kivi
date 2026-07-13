pub mod human;
pub mod json;

use clap::ValueEnum;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum OutputFormat {
    #[default]
    Human,
    Json,
}

#[derive(Debug, Clone, Copy)]
pub enum ValueMode {
    Masked,
    Revealed,
    KeysOnly,
}

pub const MASK: &str = "****";

pub fn apply_value_mode(
    data: &HashMap<String, Value>,
    mode: ValueMode,
) -> HashMap<String, Value> {
    match mode {
        ValueMode::Revealed => data.clone(),
        ValueMode::Masked => data
            .keys()
            .map(|k| (k.clone(), Value::String(MASK.to_string())))
            .collect(),
        ValueMode::KeysOnly => HashMap::new(), // caller uses keys() instead
    }
}

pub fn resolve_value_mode(show_values: bool, keys_only: bool) -> ValueMode {
    if keys_only {
        ValueMode::KeysOnly
    } else if show_values {
        ValueMode::Revealed
    } else {
        ValueMode::Masked
    }
}

#[derive(Default)]
pub struct TreeNode {
    pub children: BTreeMap<String, TreeNode>,
    pub secrets: BTreeMap<String, HashMap<String, Value>>,
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
