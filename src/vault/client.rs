use crate::error::KiviError;
use log::debug;
use reqwest::{Client, Method, StatusCode};
use serde::Serialize;
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
        if let Some(ns) = namespace
            && let Ok(val) = reqwest::header::HeaderValue::from_str(ns)
        {
            headers.insert("X-Vault-Namespace", val);
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
        let url = self.url(path);
        debug!("GET {url}");
        let resp = self
            .client
            .get(&url)
            .header("X-Vault-Token", &self.token)
            .send()
            .await?;
        debug!("GET {path} -> {}", resp.status());
        self.handle_response(resp, path).await
    }

    pub async fn list(&self, path: &str) -> Result<Value, KiviError> {
        let url = self.url(path);
        debug!("LIST {url}");
        let resp = self
            .client
            .request(Method::from_bytes(b"LIST").unwrap(), &url)
            .header("X-Vault-Token", &self.token)
            .send()
            .await?;
        debug!("LIST {path} -> {}", resp.status());
        self.handle_response(resp, path).await
    }

    pub async fn post(&self, path: &str, body: &impl Serialize) -> Result<Value, KiviError> {
        let url = self.url(path);
        debug!("POST {url}");
        let resp = self
            .client
            .post(&url)
            .header("X-Vault-Token", &self.token)
            .json(body)
            .send()
            .await?;
        debug!("POST {path} -> {}", resp.status());
        self.handle_response(resp, path).await
    }

    pub async fn patch(&self, path: &str, body: &impl Serialize) -> Result<Value, KiviError> {
        let url = self.url(path);
        debug!("PATCH {url}");
        let json_bytes = serde_json::to_vec(body)?;
        let resp = self
            .client
            .patch(&url)
            .header("X-Vault-Token", &self.token)
            .header("Content-Type", "application/merge-patch+json")
            .body(json_bytes)
            .send()
            .await?;
        debug!("PATCH {path} -> {}", resp.status());
        self.handle_response(resp, path).await
    }

    pub async fn delete(&self, path: &str) -> Result<(), KiviError> {
        let url = self.url(path);
        debug!("DELETE {url}");
        let resp = self
            .client
            .delete(&url)
            .header("X-Vault-Token", &self.token)
            .send()
            .await?;
        debug!("DELETE {path} -> {}", resp.status());
        match resp.status() {
            StatusCode::NO_CONTENT | StatusCode::OK => Ok(()),
            StatusCode::FORBIDDEN => Err(KiviError::Forbidden {
                path: path.to_string(),
            }),
            StatusCode::NOT_FOUND => Err(KiviError::NotFound {
                path: path.to_string(),
            }),
            status => {
                let message = resp.text().await.unwrap_or_default();
                Err(KiviError::Api {
                    status: status.as_u16(),
                    message,
                })
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
            StatusCode::FORBIDDEN => Err(KiviError::Forbidden {
                path: path.to_string(),
            }),
            StatusCode::NOT_FOUND => Err(KiviError::NotFound {
                path: path.to_string(),
            }),
            status => {
                let body: Value = resp.json().await.unwrap_or(Value::Null);
                let message = body
                    .get("errors")
                    .and_then(|e| e.as_array())
                    .and_then(|a| a.first())
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown error")
                    .to_string();
                Err(KiviError::Api {
                    status: status.as_u16(),
                    message,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_basic_construction() {
        let client = VaultClient::new("https://vault.example.com", "tok", None);
        assert_eq!(
            client.url("secret/data/app"),
            "https://vault.example.com/v1/secret/data/app"
        );
    }

    #[test]
    fn url_strips_leading_slash_from_path() {
        let client = VaultClient::new("https://vault.example.com", "tok", None);
        assert_eq!(
            client.url("/secret/data/app"),
            "https://vault.example.com/v1/secret/data/app"
        );
    }

    #[test]
    fn new_strips_trailing_slash_from_address() {
        let client = VaultClient::new("https://vault.example.com/", "tok", None);
        assert_eq!(
            client.url("secret/data/app"),
            "https://vault.example.com/v1/secret/data/app"
        );
    }

    #[test]
    fn new_strips_multiple_trailing_slashes() {
        let client = VaultClient::new("https://vault.example.com///", "tok", None);
        assert_eq!(
            client.url("secret/data/app"),
            "https://vault.example.com/v1/secret/data/app"
        );
    }

    #[test]
    fn url_with_query_string() {
        let client = VaultClient::new("https://vault.example.com", "tok", None);
        assert_eq!(
            client.url("secret/data/app?version=3"),
            "https://vault.example.com/v1/secret/data/app?version=3"
        );
    }
}
