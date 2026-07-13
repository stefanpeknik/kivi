# kivi

[![CI](https://github.com/stefanpeknik/kivi/actions/workflows/ci.yml/badge.svg)](https://github.com/stefanpeknik/kivi/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

A fast, ergonomic CLI for HashiCorp Vault KV v2 secrets.

kivi replaces the stock `vault kv` commands with live tab-completion, recursive tree views, interactive `$EDITOR` editing with conflict detection, and sane defaults (values masked by default, confirmations before deletes).

## Features

- **Live tab-completion** -- every `<path>` argument completes against Vault in real time
- **Tree view** -- recursive listing with masked values, key-only mode, and depth limits
- **Interactive editing** -- open secrets in `$EDITOR`, automatic CAS conflict detection with retry/force/abort
- **Recursive delete** -- list, confirm, then delete an entire subtree in one command
- **Masked by default** -- values shown as `****` unless `--show-values` is passed
- **Dual output** -- `--output human` (default, colored) or `--output json` for scripting
- **Dry-run** -- `--dry-run` on any mutating command shows what would happen without writing

## Installation

```sh
cargo install --path .
```

## Quick start

```sh
export VAULT_ADDR=https://vault.example.com:8200
export VAULT_TOKEN=$(vault login -method=oidc -field=token)

kivi config set --mount secret --default-path team/prod

kivi ls
kivi get db-creds --show-values
kivi tree --keys-only
kivi put db-creds user=admin pass=hunter2
kivi edit db-creds
kivi delete -r old-stuff
```

## Commands

| Command    | Alias        | Description                                      |
|------------|--------------|--------------------------------------------------|
| `get`      |              | Read a secret                                    |
| `list`     | `ls`         | List entries at a path                           |
| `tree`     |              | Recursive tree view                              |
| `put`      | `set`,`write`| Create or overwrite a secret                     |
| `patch`    |              | Merge keys into a secret without full overwrite  |
| `edit`     |              | Open in `$EDITOR` with CAS conflict detection    |
| `diff`     |              | Compare two versions of a secret                 |
| `delete`   | `rm`         | Delete a secret or subtree (`-r`)                |
| `undelete` |              | Recover a soft-deleted version                   |
| `metadata` |              | View/manage metadata and version history         |
| `config`   |              | Manage CLI configuration                         |

All commands that take a `<path>` support live tab-completion.

## Configuration

Config file: `~/.config/kivi/config.toml` (created on first run)

```toml
address = "https://vault.example.com:8200"
mount = "secret"
default_path = "team/prod"
show_values = false
```

**Precedence** (highest to lowest): CLI flags > environment variables (`VAULT_ADDR`, `VAULT_TOKEN`, `VAULT_NAMESPACE`) > config file > defaults.

## Git hooks

The repo includes a pre-push hook that runs `cargo fmt --check`, `cargo clippy`, and `cargo test`. To enable:

```sh
git config core.hooksPath .githooks
```

## License

[MIT](LICENSE)
