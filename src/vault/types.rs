use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Deserialize)]
pub struct SecretData {
    pub data: HashMap<String, serde_json::Value>,
    pub metadata: SecretMetadata,
}

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

#[derive(Debug, Deserialize)]
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
    pub custom_metadata: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct VersionInfo {
    pub created_time: String,
    pub deletion_time: Option<String>,
    pub destroyed: bool,
}
