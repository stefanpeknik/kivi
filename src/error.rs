use thiserror::Error;

#[derive(Error, Debug)]
pub enum KiviError {
    #[error(
        "No Vault token found.\n  Run: kivi login\n  or:  export VAULT_TOKEN=<token>\n  or:  pass --token"
    )]
    NoToken,

    #[error(
        "No Vault address found. Provide one via:\n  export VAULT_ADDR=https://your-vault:8200\n  or pass --address\n  or run: kivi config set --address <url>"
    )]
    NoAddress,

    #[error(
        "No KV mount configured. Provide one via:\n  kivi config set --mount <mount>\n  or pass --mount"
    )]
    NoMount,

    #[error("Permission denied at path: {path}")]
    Forbidden { path: String },

    #[error("Not found: {path}")]
    NotFound { path: String },

    #[allow(dead_code)]
    #[error("Vault mount '{mount}' is a KVv1 engine. kivi requires KVv2.")]
    KvV1Engine { mount: String },

    #[error(
        "CAS conflict: secret at '{path}' was modified since last read (expected version {expected})"
    )]
    CasConflict { path: String, expected: u64 },

    #[error("Vault API error ({status}): {message}")]
    Api { status: u16, message: String },

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Config error: {0}")]
    Config(String),

    #[error("{0}")]
    Usage(String),

    #[error("OIDC login failed: {0}")]
    OidcLogin(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Task panicked: {0}")]
    TaskPanic(String),
}
