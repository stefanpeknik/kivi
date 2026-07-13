use crate::error::KiviError;
use reqwest::{Client, Method, StatusCode};
use serde_json::Value;
use std::time::Duration;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

pub struct VaultClient {
    client: Client,
    address: String,
    token: String,
}

impl VaultClient {
    /// Create a client. Pass `namespace = Some("ns")` for Vault Enterprise namespaces.
    pub fn new(
        address: impl Into<String>,
        token: impl Into<String>,
        namespace: Option<&str>,
    ) -> Self {
        let mut headers = reqwest::header::HeaderMap::new();
        if let Some(ns) = namespace {
            if let Ok(val) = reqwest::header::HeaderValue::from_str(ns) {
                headers.insert("X-Vault-Namespace", val);
            }
        }
        Self {
            client: Client::builder()
                .timeout(DEFAULT_TIMEOUT)
                .default_headers(headers)
                .build()
                .expect("failed to build HTTP client"),
            address: address.into().trim_end_matches('/').to_string(),
            token: token.into(),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}/v1/{}", self.address, path.trim_start_matches('/'))
    }

    pub async fn get(&self, path: &str) -> Result<Value, KiviError> {
        let resp = self
            .client
            .get(self.url(path))
            .header("X-Vault-Token", &self.token)
            .send()
            .await?;
        self.handle_response(resp, path).await
    }

    pub async fn list(&self, path: &str) -> Result<Value, KiviError> {
        let resp = self
            .client
            .request(Method::from_bytes(b"LIST").unwrap(), self.url(path))
            .header("X-Vault-Token", &self.token)
            .send()
            .await?;
        self.handle_response(resp, path).await
    }

    pub async fn post(&self, path: &str, body: &Value) -> Result<Value, KiviError> {
        let resp = self
            .client
            .post(self.url(path))
            .header("X-Vault-Token", &self.token)
            .json(body)
            .send()
            .await?;
        self.handle_response(resp, path).await
    }

    pub async fn patch(&self, path: &str, body: &Value) -> Result<Value, KiviError> {
        // .json() must come before the Content-Type override — reqwest's .json() sets
        // Content-Type: application/json via insert(), so we override it after.
        let resp = self
            .client
            .patch(self.url(path))
            .header("X-Vault-Token", &self.token)
            .json(body)
            .header("Content-Type", "application/merge-patch+json")
            .send()
            .await?;
        self.handle_response(resp, path).await
    }

    pub async fn delete(&self, path: &str) -> Result<(), KiviError> {
        let resp = self
            .client
            .delete(self.url(path))
            .header("X-Vault-Token", &self.token)
            .send()
            .await?;
        match resp.status() {
            StatusCode::NO_CONTENT | StatusCode::OK => Ok(()),
            StatusCode::FORBIDDEN => Err(KiviError::Forbidden { path: path.to_string() }),
            StatusCode::NOT_FOUND => Err(KiviError::NotFound { path: path.to_string() }),
            status => {
                let message = resp.text().await.unwrap_or_default();
                Err(KiviError::Api { status: status.as_u16(), message })
            }
        }
    }

    async fn handle_response(
        &self,
        resp: reqwest::Response,
        path: &str,
    ) -> Result<Value, KiviError> {
        match resp.status() {
            StatusCode::OK => Ok(resp.json::<Value>().await?),
            StatusCode::NO_CONTENT => Ok(Value::Null),
            StatusCode::FORBIDDEN => Err(KiviError::Forbidden { path: path.to_string() }),
            StatusCode::NOT_FOUND => Err(KiviError::NotFound { path: path.to_string() }),
            status => {
                let body: Value = resp.json().await.unwrap_or(Value::Null);
                let message = body
                    .get("errors")
                    .and_then(|e| e.as_array())
                    .and_then(|a| a.first())
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown error")
                    .to_string();
                Err(KiviError::Api { status: status.as_u16(), message })
            }
        }
    }
}
