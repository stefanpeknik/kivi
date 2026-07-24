use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct SecretMetadata {
    pub version: u64,
    pub created_time: String,
    pub destroyed: bool,
    pub deletion_time: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct KvListResponse {
    pub data: KvListData,
}

#[derive(Debug, Deserialize)]
pub struct KvListData {
    pub keys: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct KvReadResponse {
    pub data: KvReadData,
}

#[derive(Debug, Clone, Deserialize)]
pub struct KvReadData {
    pub data: HashMap<String, serde_json::Value>,
    pub metadata: SecretMetadata,
}

#[derive(Debug, Deserialize)]
pub struct KvWriteResponse {
    pub data: KvWriteMeta,
}

#[derive(Debug, Deserialize)]
pub struct KvWriteMeta {
    pub version: u64,
}

#[derive(Debug, Serialize)]
pub struct KvWriteRequest {
    pub data: HashMap<String, serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<KvWriteOptions>,
}

#[derive(Debug, Serialize)]
pub struct KvWriteOptions {
    pub cas: u64,
}

#[derive(Debug, Deserialize)]
pub struct FullMetadata {
    pub data: FullMetadataData,
}

#[derive(Debug, Deserialize)]
pub struct FullMetadataData {
    pub versions: HashMap<String, VersionInfo>,
    pub current_version: u64,
    pub max_versions: u64,
    pub cas_required: bool,
    pub created_time: String,
    pub updated_time: String,
    pub delete_version_after: Option<String>,
    pub custom_metadata: Option<HashMap<String, String>>,
}

impl FullMetadataData {
    pub fn is_current_version_deleted(&self) -> bool {
        let key = self.current_version.to_string();
        self.versions.get(&key).is_some_and(|v| {
            v.destroyed || v.deletion_time.as_deref().is_some_and(|t| !t.is_empty())
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct VersionInfo {
    pub created_time: String,
    pub deletion_time: Option<String>,
    pub destroyed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn deserialize_kv_list_response() {
        let raw = json!({
            "data": {
                "keys": ["db-creds", "api-key", "subdir/"]
            }
        });
        let resp: KvListResponse = serde_json::from_value(raw).unwrap();
        assert_eq!(resp.data.keys, vec!["db-creds", "api-key", "subdir/"]);
    }

    #[test]
    fn deserialize_kv_read_response() {
        let raw = json!({
            "data": {
                "data": {
                    "username": "admin",
                    "password": "hunter2"
                },
                "metadata": {
                    "version": 3,
                    "created_time": "2024-01-15T10:00:00Z",
                    "destroyed": false,
                    "deletion_time": ""
                }
            }
        });
        let resp: KvReadResponse = serde_json::from_value(raw).unwrap();
        assert_eq!(resp.data.metadata.version, 3);
        assert_eq!(resp.data.data.len(), 2);
        assert_eq!(resp.data.data["username"], json!("admin"));
        assert!(!resp.data.metadata.destroyed);
    }

    #[test]
    fn deserialize_kv_read_response_with_null_deletion_time() {
        let raw = json!({
            "data": {
                "data": { "key": "val" },
                "metadata": {
                    "version": 1,
                    "created_time": "2024-01-15T10:00:00Z",
                    "destroyed": false,
                    "deletion_time": null
                }
            }
        });
        let resp: KvReadResponse = serde_json::from_value(raw).unwrap();
        assert!(resp.data.metadata.deletion_time.is_none());
    }

    #[test]
    fn deserialize_kv_write_response() {
        let raw = json!({
            "data": {
                "version": 5
            }
        });
        let resp: KvWriteResponse = serde_json::from_value(raw).unwrap();
        assert_eq!(resp.data.version, 5);
    }

    #[test]
    fn serialize_write_request_with_cas() {
        let mut data = HashMap::new();
        data.insert("key".to_string(), json!("value"));
        let req = KvWriteRequest {
            data,
            options: Some(KvWriteOptions { cas: 2 }),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["options"]["cas"], 2);
        assert_eq!(json["data"]["key"], "value");
    }

    #[test]
    fn serialize_write_request_without_cas_omits_options() {
        let mut data = HashMap::new();
        data.insert("key".to_string(), json!("value"));
        let req = KvWriteRequest {
            data,
            options: None,
        };
        let json = serde_json::to_value(&req).unwrap();
        assert!(json.get("options").is_none());
    }

    #[test]
    fn deserialize_full_metadata() {
        let raw = json!({
            "data": {
                "versions": {
                    "1": {
                        "created_time": "2024-01-01T00:00:00Z",
                        "deletion_time": "",
                        "destroyed": false
                    },
                    "2": {
                        "created_time": "2024-06-01T00:00:00Z",
                        "deletion_time": null,
                        "destroyed": true
                    }
                },
                "current_version": 2,
                "max_versions": 10,
                "cas_required": true,
                "created_time": "2024-01-01T00:00:00Z",
                "updated_time": "2024-06-01T00:00:00Z",
                "delete_version_after": "0s",
                "custom_metadata": { "owner": "team-a" }
            }
        });
        let resp: FullMetadata = serde_json::from_value(raw).unwrap();
        assert_eq!(resp.data.current_version, 2);
        assert_eq!(resp.data.max_versions, 10);
        assert!(resp.data.cas_required);
        assert_eq!(resp.data.versions.len(), 2);
        assert!(resp.data.versions["2"].destroyed);
        assert_eq!(
            resp.data.custom_metadata.as_ref().unwrap()["owner"],
            "team-a"
        );
    }

    #[test]
    fn deserialize_full_metadata_without_optional_fields() {
        let raw = json!({
            "data": {
                "versions": {},
                "current_version": 0,
                "max_versions": 0,
                "cas_required": false,
                "created_time": "2024-01-01T00:00:00Z",
                "updated_time": "2024-01-01T00:00:00Z",
                "delete_version_after": null,
                "custom_metadata": null
            }
        });
        let resp: FullMetadata = serde_json::from_value(raw).unwrap();
        assert!(resp.data.delete_version_after.is_none());
        assert!(resp.data.custom_metadata.is_none());
    }
}
