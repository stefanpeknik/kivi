use crate::error::KiviError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const CALLBACK_TIMEOUT: Duration = Duration::from_secs(120);

const SUCCESS_HTML: &str = r#"<!DOCTYPE html>
<html><body style="font-family:system-ui;display:flex;justify-content:center;align-items:center;height:100vh;margin:0">
<div style="text-align:center"><h2>Login successful</h2><p>This tab will close automatically. You can also close it manually.</p></div>
<script>setTimeout(function(){window.close()},500)</script>
</body></html>"#;

pub struct OidcLoginResult {
    pub client_token: String,
    pub lease_duration: u64,
    pub renewable: bool,
}

#[derive(Serialize)]
struct AuthUrlRequest {
    redirect_uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
}

#[derive(Deserialize)]
struct AuthUrlResponse {
    data: AuthUrlData,
}

#[derive(Deserialize)]
struct AuthUrlData {
    auth_url: String,
}

#[derive(Deserialize)]
struct CallbackResponse {
    auth: AuthData,
}

#[derive(Deserialize)]
struct AuthData {
    client_token: String,
    lease_duration: u64,
    renewable: bool,
}

pub async fn oidc_login(
    address: &str,
    oidc_mount: &str,
    role: Option<&str>,
    port: u16,
) -> Result<OidcLoginResult, KiviError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    let redirect_uri = format!("http://localhost:{port}/oidc/callback");

    // Step 1: request auth URL from Vault
    let auth_url_endpoint = format!(
        "{}/v1/auth/{}/oidc/auth_url",
        address.trim_end_matches('/'),
        oidc_mount,
    );

    let body = AuthUrlRequest {
        redirect_uri,
        role: role.map(str::to_string),
    };

    tracing::debug!("requesting OIDC auth URL");
    let resp = client.post(&auth_url_endpoint).json(&body).send().await?;
    tracing::debug!(status = resp.status().as_u16(), "auth URL response");

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        return Err(KiviError::OidcLogin(format!(
            "Vault returned {status} from auth_url endpoint: {text}"
        )));
    }

    let auth_url_resp: AuthUrlResponse = resp.json().await?;
    let auth_url = auth_url_resp.data.auth_url;

    // Extract nonce from the auth URL — the IdP callback only returns state + code,
    // but Vault's callback endpoint needs the nonce too.
    let nonce = extract_query_param(&auth_url, "nonce")
        .ok_or_else(|| KiviError::OidcLogin("No 'nonce' found in auth URL from Vault".into()))?;

    // Step 2: start callback listener before opening browser
    tracing::debug!(port, "binding callback listener");
    let listener = TcpListener::bind(format!("127.0.0.1:{port}"))
        .await
        .map_err(|e| {
            KiviError::OidcLogin(format!(
                "Failed to bind localhost:{port}: {e}. Is another login in progress?"
            ))
        })?;

    // Step 3: open browser
    if open::that(&auth_url).is_err() {
        eprintln!("Could not open browser automatically.");
        eprintln!("Open this URL manually:\n  {auth_url}");
    }

    // Step 4: wait for the IdP callback (returns state + code only)
    let (state, code) = wait_for_callback(&listener).await?;

    // Step 5: exchange callback params for a Vault token
    let callback_endpoint = format!(
        "{}/v1/auth/{}/oidc/callback",
        address.trim_end_matches('/'),
        oidc_mount,
    );

    tracing::debug!("exchanging callback for token");
    let resp = client
        .get(&callback_endpoint)
        .query(&[("state", &state), ("code", &code), ("nonce", &nonce)])
        .send()
        .await?;
    tracing::debug!(status = resp.status().as_u16(), "token exchange response");

    if !resp.status().is_success() {
        let status = resp.status().as_u16();
        let text = resp.text().await.unwrap_or_default();
        return Err(KiviError::OidcLogin(format!(
            "Vault returned {status} from callback endpoint: {text}"
        )));
    }

    let cb_resp: CallbackResponse = resp.json().await?;

    Ok(OidcLoginResult {
        client_token: cb_resp.auth.client_token,
        lease_duration: cb_resp.auth.lease_duration,
        renewable: cb_resp.auth.renewable,
    })
}

async fn wait_for_callback(listener: &TcpListener) -> Result<(String, String), KiviError> {
    let (mut stream, _) = tokio::time::timeout(CALLBACK_TIMEOUT, listener.accept())
        .await
        .map_err(|_| {
            KiviError::OidcLogin(format!(
                "Timed out waiting for browser callback ({}s)",
                CALLBACK_TIMEOUT.as_secs()
            ))
        })?
        .map_err(|e| KiviError::OidcLogin(format!("Failed to accept callback connection: {e}")))?;

    let mut buf = vec![0u8; 4096];
    let n = stream
        .read(&mut buf)
        .await
        .map_err(|e| KiviError::OidcLogin(format!("Failed to read callback request: {e}")))?;

    let request = String::from_utf8_lossy(&buf[..n]);

    let request_line = request
        .lines()
        .next()
        .ok_or_else(|| KiviError::OidcLogin("Empty callback request".to_string()))?;

    // Parse "GET /oidc/callback?state=...&code=... HTTP/1.1"
    let path = request_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| KiviError::OidcLogin("Malformed callback request line".to_string()))?;

    let query = path
        .split_once('?')
        .map(|(_, q)| q)
        .ok_or_else(|| KiviError::OidcLogin("Callback URL has no query parameters".to_string()))?;

    let params = parse_query(query);

    // Respond with success page before extracting params (browser gets feedback immediately)
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        SUCCESS_HTML.len(),
        SUCCESS_HTML,
    );
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.flush().await;

    let get = |key: &str| -> Result<String, KiviError> {
        params
            .get(key)
            .cloned()
            .ok_or_else(|| KiviError::OidcLogin(format!("Missing '{key}' in callback query")))
    };

    Ok((get("state")?, get("code")?))
}

fn parse_query(query: &str) -> HashMap<String, String> {
    query
        .split('&')
        .filter_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            Some((k.to_string(), percent_decode(v)))
        })
        .collect()
}

fn extract_query_param(url: &str, key: &str) -> Option<String> {
    let query = url.split_once('?').map(|(_, q)| q)?;
    parse_query(query).remove(key)
}

fn percent_decode(input: &str) -> String {
    let mut out = Vec::with_capacity(input.len());
    let mut bytes = input.bytes();
    while let Some(b) = bytes.next() {
        match b {
            b'%' => {
                let decoded = bytes
                    .next()
                    .zip(bytes.next())
                    .and_then(|(h, l)| hex_pair(h, l));
                match decoded {
                    Some(v) => out.push(v),
                    None => out.push(b'%'),
                }
            }
            b'+' => out.push(b' '),
            _ => out.push(b),
        }
    }
    String::from_utf8(out).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}

fn hex_pair(hi: u8, lo: u8) -> Option<u8> {
    let h = char::from(hi).to_digit(16)? as u8;
    let l = char::from(lo).to_digit(16)? as u8;
    Some(h << 4 | l)
}
