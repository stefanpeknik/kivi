# kivi Design Document

> Living design document for **kivi** — a custom HashiCorp Vault KV CLI tool.
> Updated: 2026-07-12

---

## 1. Motivation

The official `vault` CLI is fully featured but purely imperative — no interactive navigation, no tree views, poor DX for everyday secret exploration. Third-party tools like [vkv](https://github.com/FalcoSuessgott/vkv) improve read/export workflows but lack write operations, interactive editing, and true recursive key-only listing. A custom bash script (`vaultbrowser`) using fzf proved the value of interactive navigation but hit the limits of bash for anything beyond basic browsing.

This project aims to build a focused, fast CLI for KV secret management that covers the full read-write lifecycle with excellent tab-completion and sensible defaults.

---

## 2. Goals & Non-Goals

### Goals

- **Fast, dynamic tab-completion** for Vault paths — the primary UX differentiator
- **Recursive tree listing** with key-only mode (hide values by default, show on explicit flag)
- **Live editing** of secrets via `$EDITOR` with conflict detection (CAS — Check-And-Set)
- **Recursive delete** with safety prompts and `--force` flag
- **Full KV read-write parity** with the official `vault kv` commands (get, put, patch, delete, undelete, metadata, versions)
- **Persistent user configuration** (default paths, output preferences, auth settings)
- **Extensible architecture** — KV-focused now, but structured so other Vault backends (PKI, transit, auth) can be added later without rewrites

### Non-Goals (for now)

- Auth method management, policy editing, identity management
- Full TUI application (interactive navigation may be revisited later)
- Vault Enterprise namespace management
- Snapshot/backup functionality (vkv already does this well)
- MCP/server mode

---

## 3. Language & Technology Choices

### Decision: Rust

| Criterion | Rust | Python |
|---|---|---|
| Startup latency | ~1-5ms (critical for tab-completion) | ~100-300ms (noticeable on every tab press) |
| Distribution | Single static binary, no runtime | Requires Python runtime or bundling (PyInstaller etc.) |
| CLI framework | `clap` — industry-standard, built-in dynamic completions | `click`/`typer` — excellent, but no native dynamic completions |
| Vault API | `reqwest` + hand-rolled or thin client | `hvac` — mature, full-featured |
| Ecosystem | Strong for CLI/systems tools | Stronger for general-purpose, weaker for perf-sensitive CLI |
| Dev velocity | Slower initial development, safer long-term | Faster prototyping, more runtime surprises |

**Rationale:** Tab-completion that fetches live data from Vault on every keystroke is the core feature. Sub-10ms startup is essential for this to feel responsive. Rust delivers that while also providing a zero-dependency binary.

### Key Dependencies (planned)

| Crate | Purpose |
|---|---|
| `clap` + `clap_complete` | CLI parsing, static & dynamic shell completions |
| `reqwest` | HTTP client for Vault API |
| `serde` + `serde_json` | JSON serialization/deserialization |
| `tokio` | Async runtime (for concurrent Vault API calls during tree listing) |
| `tabled` or `comfy-table` | Table output formatting |
| `console` + `dialoguer` | Terminal colors, interactive prompts (delete confirmations) |
| `toml` | Config file parsing |
| `tempfile` | Temp files for `$EDITOR` workflow |
| `thiserror` / `anyhow` | Error handling |

---

## 4. Command Structure

```
kivi [OPTIONS] <COMMAND>

Commands:
  get       Read a secret at a given path
  list      List secrets/directories at a path (alias: ls)
  tree      Recursively list the full secret tree from a path
  put       Create or overwrite a secret (alias: set, write)
  patch     Update specific keys in a secret without overwriting others
  edit      Open a secret in $EDITOR for interactive editing
  delete    Delete a secret or recursively delete a subtree (alias: rm)
  undelete  Recover a soft-deleted secret version
  metadata  View or manage secret metadata and versions
  config    Manage persistent CLI configuration
  completion  Generate shell completion scripts

Global Options:
  -a, --address <URL>     Vault server address (env: VAULT_ADDR)
  -t, --token <TOKEN>     Vault token (env: VAULT_TOKEN)
  -n, --namespace <NS>    Vault namespace (env: VAULT_NAMESPACE)
  -m, --mount <MOUNT>     KV engine mount point (default: from config or "secret")
  -o, --output <FORMAT>   Output format: human (default), json
  -v, --verbose           Increase output verbosity
      --dry-run           Show what would happen without making any changes
      --no-color          Disable colored output
```

### 4.1 Command Details

#### `kivi get <path>`

Read and display a single secret.

```
Options:
  --version <N>    Read a specific version (KVv2)
  --show-values    Reveal secret values (default: masked as "****")
  --keys-only      Omit values entirely, show only key names
```

Default behavior: show keys with masked values. Explicit `--show-values` to reveal.

#### `kivi list <path>` (alias: `ls`)

List entries (subpaths and leaf secrets) at a single level.

```
Options:
  -l, --long          Show additional metadata (type, version, last modified)
```

#### `kivi tree <path>`

Recursively list the full subtree from a given path.

```
Options:
  --keys-only      Omit values entirely, show only key names (recursively)
  --show-values    Reveal actual values (default: masked as "****")
  --depth <N>      Limit recursion depth
```

Default: show the tree structure with leaf secret key names visible but values masked.

#### `kivi put <path> [key=value...]` (aliases: `set`, `write`)

Create or fully overwrite a secret.

```
Options:
  --stdin             Read key-value pairs from stdin (JSON)
  --file <FILE>       Read key-value pairs from a file (JSON/YAML)
  --cas <VERSION>     Check-And-Set: only write if current version matches
```

#### `kivi patch <path> [key=value...]`

Merge key-value pairs into an existing secret without overwriting unspecified keys.

```
Options:
  --remove <KEY>      Remove a specific key from the secret
  --stdin             Read key-value pairs from stdin (JSON)
```

#### `kivi edit <path>`

Open a secret in `$EDITOR` for interactive full overwrite (same semantics as `put`).

**Workflow:**
1. Fetch the current secret and its version number
2. Write the complete secret data to a temporary JSON file
3. Open `$EDITOR` with the temp file
4. On save+quit, diff against the original
5. If changed, attempt a CAS (Check-And-Set) `put` using the original version number — whatever is in the file becomes the new complete version (keys removed from the file are deleted from the secret)
6. If CAS fails (someone else modified the secret in the meantime), show a conflict warning with both versions and ask the user how to proceed:
   - **Retry**: Re-fetch latest, re-open editor — user reconciles manually
   - **Force**: Overwrite regardless of version conflict
   - **Abort**: Cancel, leave secret unchanged
7. Clean up temp file

```
Options:
  --no-cas     Skip conflict detection (dangerous)
  --diff       Show diff before confirming write
```

#### `kivi delete <path>` (alias: `rm`)

Delete a secret or recursively delete an entire subtree.

```
Options:
  -r, --recursive     Recursively delete all secrets under the path
  -f, --force         Skip confirmation prompt
  --versions <N,N>    Delete specific versions only (KVv2 soft-delete)
  --destroy           Permanently destroy versions (cannot be undeleted)
```

**Safety behavior:**
- Single secret delete: prompt for confirmation (unless `--force`)
- Recursive delete: always list what will be deleted first, then prompt (unless `--force`)
- `--destroy` (permanent): double-prompt ("Are you sure? This cannot be undone.")

#### `kivi undelete <path> --versions <N,N>`

Recover soft-deleted secret versions (KVv2 only).

#### `kivi metadata <path>`

View secret metadata, version history, and configuration.

```
Subcommands:
  metadata show <path>          Show metadata and version history
  metadata set <path> [opts]    Update metadata settings (max-versions, cas-required, etc.)
  metadata delete <path>        Permanently delete all versions and metadata
```

#### `kivi config`

Manage persistent CLI configuration.

Config file is created automatically with defaults on first run if it doesn't exist.

```
Subcommands:
  config show                   Display current configuration
  config set <key> <value>      Set a config value
  config path                   Print the config file path
```

#### `kivi completion <shell>`

Generate shell completion scripts (bash, zsh, fish).

---

## 5. `--dry-run` Behavior

`--dry-run` is a global flag available on all commands. It performs all read operations and validation but skips any writes, deletes, or mutations. Output describes what *would* have happened.

| Command | Dry-run behavior |
|---|---|
| `get` | No-op (already read-only); silently succeeds |
| `list` / `tree` | No-op (already read-only); silently succeeds |
| `put` | Shows the secret that would be written (key/value pairs), no write |
| `patch` | Shows the merged result that would be written, no write |
| `edit` | Opens `$EDITOR` as normal, shows the diff on save, skips the `put` write |
| `delete` | Lists paths that would be deleted, no delete |
| `undelete` | Shows versions that would be restored, no restore |
| `metadata set` | Shows metadata fields that would be updated, no update |

For mutating commands (`put`, `patch`, `edit`, `delete`), `--dry-run` output is prefixed with a `[dry-run]` label and rendered in a distinct color so it's unambiguous that no action was taken.

---

## 6. Tab-Completion (Dynamic)

The key UX feature. When the user types a path argument and presses Tab, the CLI fetches live data from Vault to suggest completions.

Tab completion is the core feature of kivi. Every design decision here is made to make it feel instant and reliable.

### How it works

1. Shell sources a completion script (generated by `kivi completion zsh`)
2. On Tab press, the shell invokes `kivi --complete <partial-path>`
3. The binary checks the completion cache; on miss, calls the Vault KV list API on the parent segment of the partial path
4. Filters results to those matching the typed prefix and returns them to the shell

### Path completion behaviour

Completion is smart about the structure of Vault paths:

- **Any depth**: `kivi get ap<TAB>` → `apps/`; `kivi get apps/te<TAB>` → `apps/team-a/`, `apps/team-b/`; `kivi get apps/team-a/ser<TAB>` → `apps/team-a/service-1`, `apps/team-a/service-2`
- **Directory vs leaf distinction**: entries ending in `/` are directories (Tab into them); entries without `/` are leaf secrets (complete and stop)
- **Prefix filtering**: only entries that start with the typed segment after the last `/` are returned — the shell doesn't need to filter
- **Mount awareness**: if `mount` is set in config, completion operates relative to it; the user never needs to type the mount prefix
- **All path arguments**: completion is wired to every command that takes a `<path>` argument (`get`, `put`, `patch`, `edit`, `delete`, `undelete`, `metadata`, `list`, `tree`)

### Caching

Not implemented in v1 — every Tab press calls the Vault API directly. Caching is a planned future optimisation once the core completion behaviour is stable and real-world latency can be measured.

### Timeout

**10 seconds, hard.** If the Vault API call does not return within 10 seconds, completion returns an empty list. The shell continues normally — the user sees no completions rather than a hung prompt. The timeout is not configurable; a user who wants longer should fix their Vault connectivity.

### Shell support

- **zsh**: Primary target (user's shell). Custom `_kivi` completion function using `_arguments` and `compadd`.
- **bash**: `complete -C` integration.
- **fish**: `complete` with dynamic function.

---

## 7. Configuration

### Config file location

Follow XDG: `$XDG_CONFIG_HOME/kivi/config.toml` (default: `~/.config/kivi/config.toml`)

### Config schema

```toml
# Default Vault server address (overridden by VAULT_ADDR env var)
address = "https://vault.example.com:8200"

# KV engine mount point (equivalent to `vault kv -mount=...`)
mount = "apps"

# Default starting path for commands (relative to mount)
# Useful if you always work within a subpath
default_path = ""

# Default format for $EDITOR workflows (json only currently)
editor_format = "json"

# Show values by default (false = masked)
show_values = false
```

### Precedence (highest to lowest)

1. CLI flags (`--address`, `--token`, `--mount`, etc.)
2. Environment variables (`VAULT_ADDR`, `VAULT_TOKEN`)
3. Config file values (`~/.config/kivi/config.toml`)
4. Built-in defaults

---

## 8. Authentication

`VAULT_TOKEN` env var must be set. If absent, kivi exits with a clear error message and instructions. No fallback chain, no OIDC, no token file.

Auth complexity will be revisited in a future iteration once the core feature set is built.

---

## 9. Output & Display Defaults

### Philosophy: human-friendly by default, JSON on demand

Two output formats controlled by `-o`/`--output`:

| Format | Flag | Use case |
|---|---|---|
| `human` | default | Interactive daily use — colored, readable |
| `json` | `--output json` | Scripting, piping to `jq`, machine consumption |

**Values are masked as `"****"` by default in both formats.** `--show-values` and `--keys-only` apply equally to both.

### Human format examples

#### `kivi get apps/team-a/service-1`

```
Path:    apps/team-a/service-1
Version: 4

DB_HOST    ****
DB_PASS    ****
API_KEY    ****
```

#### `kivi tree apps`

```
apps/
├── team-a/
│   ├── service-1
│   │   ├── DB_HOST: ****
│   │   ├── DB_PASS: ****
│   │   └── API_KEY: ****
│   └── service-2
│       └── TOKEN: ****
└── team-b/
    └── service-3
        ├── SECRET: ****
        └── URL: ****
```

With `--keys-only`:
```
apps/
├── team-a/
│   ├── service-1  [DB_HOST, DB_PASS, API_KEY]
│   └── service-2  [TOKEN]
└── team-b/
    └── service-3  [SECRET, URL]
```

### JSON format examples

#### `kivi get apps/team-a/service-1 --output json`

```json
{
  "path": "apps/team-a/service-1",
  "version": 4,
  "data": {
    "DB_HOST": "****",
    "DB_PASS": "****",
    "API_KEY": "****"
  }
}
```

#### `kivi tree apps --output json`

```json
{
  "team-a": {
    "service-1": { "DB_HOST": "****", "DB_PASS": "****", "API_KEY": "****" },
    "service-2": { "TOKEN": "****" }
  },
  "team-b": {
    "service-3": { "SECRET": "****", "URL": "****" }
  }
}
```

---

## 10. Concurrency

Vault is an HTTP service — sequential requests waste wall-clock time proportional to N × latency. Any operation making N ≥ 2 independent API calls must fan them out concurrently.

### Bounded concurrency

All concurrent operations use `tokio::task::JoinSet` with a cap of **16 in-flight requests** to avoid overwhelming Vault or hitting rate limits. This is an internal constant, not a user-facing config knob.

### Where concurrency applies

| Command | Concurrent operations |
|---|---|
| `tree` | BFS directory listing: all `LIST` calls at each depth level fire in parallel; once dirs are known, all leaf secret reads fire in parallel |
| `list --long` | One metadata fetch per entry, all in parallel |
| `delete --recursive` | After listing (which is itself concurrent), all delete calls fire in parallel |
| `completion` | Single `LIST` call per Tab press — no parallelism needed |

### Tree concurrency model

```
LIST apps/             → [team-a/, team-b/]
  ├─ LIST apps/team-a/ → [service-1, service-2]   ┐ concurrent
  └─ LIST apps/team-b/ → [service-3]               ┘
       ├─ GET apps/team-a/service-1  ┐
       ├─ GET apps/team-a/service-2  │ all concurrent
       └─ GET apps/team-b/service-3  ┘
```

Results are collected and sorted before rendering so output order is deterministic regardless of which request finishes first.

---

## 11. Error Handling

- **Permission denied (403)**: Clear message indicating which path failed and suggesting policy review
- **Not found (404)**: Distinguish between "path doesn't exist" and "no secret at this path" (it might be a directory)
- **Connection errors**: Timeout after configurable duration, suggest checking VAULT_ADDR
- **CAS conflicts (edit command)**: Interactive resolution flow (see section 4.1 edit command)
- **Auth expired**: Auto-detect and re-prompt or suggest re-authentication

---

## 12. Project Structure (planned)

```
kivi/
├── Cargo.toml
├── src/
│   ├── main.rs              # Entry point, clap setup
│   ├── cli/                  # Command definitions and argument parsing
│   │   ├── mod.rs
│   │   ├── get.rs
│   │   ├── list.rs
│   │   ├── tree.rs
│   │   ├── put.rs
│   │   ├── patch.rs
│   │   ├── edit.rs
│   │   ├── delete.rs
│   │   ├── undelete.rs
│   │   ├── metadata.rs
│   │   ├── config.rs
│   │   └── completion.rs
│   ├── vault/                # Vault API client
│   │   ├── mod.rs
│   │   ├── client.rs         # HTTP client, auth, token management
│   │   ├── kv.rs             # KV-specific operations
│   │   └── types.rs          # Request/response types
│   ├── output/               # Output rendering (human + JSON)
│   │   ├── mod.rs            # OutputFormat enum, shared masking logic
│   │   ├── human.rs          # Colored ASCII tree, aligned key/value
│   │   └── json.rs           # JSON serialization
│   ├── config.rs             # Config file management
│   ├── editor.rs             # $EDITOR workflow and CAS logic
│   └── completion.rs         # Dynamic completion logic
├── tests/
│   └── integration/          # Integration tests (against test Vault instance)
├── docs/
│   ├── DESIGN.md             # This document
│   └── DECISIONS.md          # Decision log
└── README.md
```

---

## 13. Open Questions

- [x] ~~**CLI name**~~: **kivi** — short, fun, memorable. (decided 2026-07-12)
- [x] ~~**KVv1 support**~~: KVv2 only. Detect v1 engines and error clearly. (decided 2026-07-12, see D010)
- [x] ~~**Editor temp file format**~~: JSON — consistent with Vault's own default format. (decided 2026-07-12, see D011)
- [ ] **Completion caching**: Future optimisation — v1 calls Vault directly on each Tab press
- [ ] **Extended auth methods**: OIDC, token file fallback — deferred, v1 uses VAULT_TOKEN only
