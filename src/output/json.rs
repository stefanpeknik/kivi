use crate::output::{apply_value_mode, TreeNode, ValueMode, MASK};
use crate::vault::types::{FullMetadataData, KvReadData};
use serde_json::{json, Value};
use std::collections::HashMap;

fn to_pretty(val: &Value) -> String {
    serde_json::to_string_pretty(val).unwrap_or_else(|_| format!("{val:?}"))
}

pub fn print_secret(path: &str, secret: &KvReadData, mode: ValueMode) {
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
        "delete_version_after": meta.delete_version_after,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_tree() -> TreeNode {
        let mut tree = TreeNode::new();
        let mut secret = HashMap::new();
        secret.insert("user".to_string(), Value::String("admin".into()));
        secret.insert("pass".to_string(), Value::String("s3cret".into()));
        tree.insert_path(&["app", "db"], Some(secret));

        let mut api_secret = HashMap::new();
        api_secret.insert("token".to_string(), Value::String("abc123".into()));
        tree.insert_path(&["app", "api"], Some(api_secret));

        let mut root_secret = HashMap::new();
        root_secret.insert("key".to_string(), Value::String("val".into()));
        tree.insert_path(&["root-cred"], Some(root_secret));

        tree
    }

    #[test]
    fn node_to_json_revealed_preserves_values() {
        let tree = make_tree();
        let json = node_to_json(&tree, ValueMode::Revealed);
        assert_eq!(json["app"]["db"]["user"], "admin");
        assert_eq!(json["app"]["db"]["pass"], "s3cret");
        assert_eq!(json["app"]["api"]["token"], "abc123");
        assert_eq!(json["root-cred"]["key"], "val");
    }

    #[test]
    fn node_to_json_masked_hides_values() {
        let tree = make_tree();
        let json = node_to_json(&tree, ValueMode::Masked);
        assert_eq!(json["app"]["db"]["user"], MASK);
        assert_eq!(json["app"]["db"]["pass"], MASK);
        assert_eq!(json["root-cred"]["key"], MASK);
    }

    #[test]
    fn node_to_json_keys_only_returns_key_arrays() {
        let tree = make_tree();
        let json = node_to_json(&tree, ValueMode::KeysOnly);

        let db_keys = json["app"]["db"].as_array().unwrap();
        let key_strings: Vec<&str> = db_keys.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(key_strings.contains(&"user"));
        assert!(key_strings.contains(&"pass"));

        let api_keys = json["app"]["api"].as_array().unwrap();
        assert_eq!(api_keys.len(), 1);
    }

    #[test]
    fn node_to_json_empty_tree() {
        let tree = TreeNode::new();
        let json = node_to_json(&tree, ValueMode::Revealed);
        assert_eq!(json, Value::Object(serde_json::Map::new()));
    }

    #[test]
    fn node_to_json_nested_structure_is_object() {
        let tree = make_tree();
        let json = node_to_json(&tree, ValueMode::Revealed);
        assert!(json["app"].is_object());
        assert!(json["app"]["db"].is_object());
    }
}

