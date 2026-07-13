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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_data() -> HashMap<String, Value> {
        let mut m = HashMap::new();
        m.insert("user".into(), Value::String("admin".into()));
        m.insert("pass".into(), Value::String("s3cret".into()));
        m
    }

    #[test]
    fn apply_value_mode_revealed_returns_original_data() {
        let data = sample_data();
        let result = apply_value_mode(&data, ValueMode::Revealed);
        assert_eq!(result, data);
    }

    #[test]
    fn apply_value_mode_masked_replaces_all_values() {
        let data = sample_data();
        let result = apply_value_mode(&data, ValueMode::Masked);
        assert_eq!(result.len(), data.len());
        for val in result.values() {
            assert_eq!(val, &Value::String(MASK.to_string()));
        }
    }

    #[test]
    fn apply_value_mode_masked_preserves_keys() {
        let data = sample_data();
        let result = apply_value_mode(&data, ValueMode::Masked);
        for key in data.keys() {
            assert!(result.contains_key(key));
        }
    }

    #[test]
    fn apply_value_mode_keys_only_returns_empty() {
        let data = sample_data();
        let result = apply_value_mode(&data, ValueMode::KeysOnly);
        assert!(result.is_empty());
    }

    #[test]
    fn resolve_value_mode_default_is_masked() {
        assert!(matches!(resolve_value_mode(false, false), ValueMode::Masked));
    }

    #[test]
    fn resolve_value_mode_show_values_returns_revealed() {
        assert!(matches!(resolve_value_mode(true, false), ValueMode::Revealed));
    }

    #[test]
    fn resolve_value_mode_keys_only_takes_priority() {
        assert!(matches!(resolve_value_mode(true, true), ValueMode::KeysOnly));
        assert!(matches!(resolve_value_mode(false, true), ValueMode::KeysOnly));
    }

    #[test]
    fn tree_insert_single_segment_leaf() {
        let mut tree = TreeNode::new();
        tree.insert_path(&["db-creds"], Some(sample_data()));
        assert!(tree.secrets.contains_key("db-creds"));
        assert!(tree.children.is_empty());
    }

    #[test]
    fn tree_insert_single_segment_no_data_does_nothing() {
        let mut tree = TreeNode::new();
        tree.insert_path(&["db-creds"], None);
        assert!(tree.secrets.is_empty());
        assert!(tree.children.is_empty());
    }

    #[test]
    fn tree_insert_empty_parts_does_nothing() {
        let mut tree = TreeNode::new();
        tree.insert_path(&[], Some(sample_data()));
        assert!(tree.secrets.is_empty());
        assert!(tree.children.is_empty());
    }

    #[test]
    fn tree_insert_nested_path_creates_children() {
        let mut tree = TreeNode::new();
        tree.insert_path(&["team", "prod", "db"], Some(sample_data()));

        assert!(tree.children.contains_key("team"));
        let team = &tree.children["team"];
        assert!(team.children.contains_key("prod"));
        let prod = &team.children["prod"];
        assert!(prod.secrets.contains_key("db"));
        assert_eq!(prod.secrets["db"], sample_data());
    }

    #[test]
    fn tree_insert_multiple_paths_builds_correct_structure() {
        let mut tree = TreeNode::new();
        tree.insert_path(&["team", "db"], Some(sample_data()));
        tree.insert_path(&["team", "api"], Some(sample_data()));
        tree.insert_path(&["other"], Some(sample_data()));

        assert_eq!(tree.children.len(), 1); // "team"
        assert_eq!(tree.secrets.len(), 1); // "other"
        let team = &tree.children["team"];
        assert_eq!(team.secrets.len(), 2); // "db" and "api"
    }

    #[test]
    fn tree_insert_overwrites_existing_leaf() {
        let mut tree = TreeNode::new();
        let mut data1 = HashMap::new();
        data1.insert("v".into(), Value::String("1".into()));
        let mut data2 = HashMap::new();
        data2.insert("v".into(), Value::String("2".into()));

        tree.insert_path(&["secret"], Some(data1));
        tree.insert_path(&["secret"], Some(data2.clone()));

        assert_eq!(tree.secrets["secret"], data2);
    }
}
