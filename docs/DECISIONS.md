# Decision Log

Tracks architectural and design decisions with rationale. Each entry is immutable once written — new information leads to a new entry, not editing old ones.

---

## D001 — Language: Rust

**Date:** 2026-07-12
**Status:** Accepted

**Context:** Choosing between Rust and Python for a CLI tool where dynamic tab-completion (spawning the binary on every Tab keystroke) is the core UX feature.

**Decision:** Rust.

**Rationale:**
- Startup latency is the deciding factor. Tab-completion invokes the binary on every keypress. Python startup (~100-300ms) creates noticeable lag; Rust (~1-5ms) is imperceptible.
- Single static binary — no runtime dependencies to manage on target systems.
- `clap` provides first-class support for dynamic shell completions.
- Memory safety without GC pauses.
- Strong ecosystem for CLI tools (`clap`, `reqwest`, `serde`, `dialoguer`).

**Trade-off:** Slower initial development velocity compared to Python. Accepted because the performance characteristics directly affect the primary feature.

---

## D002 — Scope: KV secrets only (extensible)

**Date:** 2026-07-12
**Status:** Accepted

**Context:** Vault supports many backends (KV, PKI, transit, auth methods, policies). The user currently only works with KV secrets.

**Decision:** Focus exclusively on KV secret management. Structure the codebase so other backends can be added as separate modules later without architectural changes.

**Rationale:**
- Doing one thing well beats doing many things poorly.
- The Vault API client layer (`vault/client.rs`) handles auth and HTTP generically; KV logic lives in `vault/kv.rs`. Adding `vault/pki.rs` later is additive, not invasive.
- Avoids premature abstraction — we'll know what the right abstractions are after building the KV module.

---

## D003 — Interaction model: CLI with dynamic completion (no TUI)

**Date:** 2026-07-12
**Status:** Accepted

**Context:** The user's `vaultbrowser` script uses fzf for interactive navigation. Considered: full TUI (like k9s/lazygit), hybrid, or pure CLI with good completions.

**Decision:** Pure CLI with rich dynamic tab-completion. Interactive navigation deferred as a potential future feature.

**Rationale:**
- Tab-completion gives the exploration UX without the overhead of a persistent TUI.
- CLI commands are composable with pipes, scripts, and other tools.
- A TUI can be added later as a separate command (`kivi browse`) without changing the core CLI.
- User explicitly prefers CLI + autocomplete over TUI.

---

## D004 — Dual output: human-readable default, JSON via `--output json`; values hidden by default

**Date:** 2026-07-12
**Status:** Accepted (revised)

**Context:** vkv couples format to value visibility. JSON-only output is clean for scripting but unfriendly for daily interactive use — ASCII trees and aligned key/value display are much easier to read at a glance.

**Decision:** Two output modes controlled by `-o`/`--output <FORMAT>`:
- `human` (default): colored, human-friendly — ASCII tree for `tree`, aligned key/value for `get`, plain list for `list`
- `json`: machine-readable JSON for scripting/piping — pipe to `jq` as needed

Values are masked as `"****"` by default in **both** formats. `--show-values` reveals them in both. `--keys-only` omits values entirely in both. No TTY detection heuristic — format is explicit.

**Rationale:**
- Daily interactive use benefits from rich terminal output; scripting benefits from JSON — both are first-class.
- Value visibility (`--show-values`) and output format (`--output`) remain orthogonal — no coupling.
- Field-present-but-redacted (`"****"`) in JSON output lets tooling enumerate keys without exposing values.
- `--output json` is the explicit, unambiguous way to get machine-readable output.

---

## D005 — Auth: VAULT_TOKEN only (v1)

**Date:** 2026-07-12
**Status:** Accepted

**Context:** Auth in Vault can be complex (OIDC, token files, fallback chains). The user already handles authentication externally and exports VAULT_TOKEN before running CLI commands.

**Decision:** Only `VAULT_TOKEN` env var is supported. If not set, kivi exits immediately with a clear error and instructions. No token file fallback, no OIDC, no fallback chain.

**Rationale:**
- The user's existing workflow already produces a VAULT_TOKEN via OIDC or direct export — kivi doesn't need to replicate that.
- A fallback chain adds code complexity and surprising behavior with no current benefit.
- Auth will be revisited in a future iteration once the core feature set is stable.

---

## D006 — Config: XDG-compliant TOML file

**Date:** 2026-07-12
**Status:** Accepted

**Context:** Need persistent configuration for default paths, auth method, display preferences.

**Decision:** TOML config at `$XDG_CONFIG_HOME/kivi/config.toml`. CLI flags override env vars override config file override built-in defaults.

**Rationale:**
- TOML is human-readable and standard for Rust CLI tools (Cargo uses it).
- XDG compliance is expected on Linux systems.
- Clear precedence chain avoids surprise behavior.

---

## D007 — Edit workflow: CAS-based conflict detection

**Date:** 2026-07-12
**Status:** Accepted

**Context:** The `edit` command opens secrets in $EDITOR. Between read and write-back, another user or system could modify the secret.

**Decision:** Use Vault's Check-And-Set (CAS) mechanism. Record the version at read time, pass it as the `cas` parameter on write. If the version has changed, present a conflict resolution prompt (retry/force/abort).

**Rationale:**
- CAS is the standard optimistic concurrency control for Vault KVv2.
- Interactive conflict resolution is better than silent overwrites or silent failures.
- `--no-cas` flag available for users who knowingly want to skip this.

---

## D008 — Recursive delete: safe by default

**Date:** 2026-07-12
**Status:** Accepted

**Context:** Recursive deletion is a destructive, hard-to-reverse operation.

**Decision:** Recursive delete (`-r`) always lists what will be deleted and prompts for confirmation. `--force` skips the prompt. `--dry-run` shows what would happen without acting. `--destroy` (permanent deletion) requires double confirmation.

**Rationale:**
- Follows the principle of least surprise (similar to `rm -ri` behavior).
- `--dry-run` enables safe scripting and verification before destructive ops.
- Double confirmation for `--destroy` because Vault's permanent destroy cannot be undone.

---

## D009 — `--dry-run` is a global flag

**Date:** 2026-07-12
**Status:** Accepted

**Context:** Dry-run behavior is useful for any mutating command, not just `delete`. Users should be able to safely preview any operation before committing.

**Decision:** `--dry-run` is a global flag available on all commands. Read-only commands (`get`, `list`, `tree`) treat it as a no-op. Mutating commands (`put`, `patch`, `edit`, `delete`, `undelete`, `metadata set`) perform all reads and validation but skip the write/delete. Output is prefixed with `[dry-run]` in a distinct color.

**Rationale:**
- Consistent UX — users don't need to remember which commands support `--dry-run`.
- Low cost to implement on read-only commands (no-op) with high benefit on write commands.
- Especially valuable for `edit` (preview the diff before writing) and `delete --recursive` (see exactly what would be removed).

---

## D010 — KVv2 only (no KVv1 support)


**Date:** 2026-07-12
**Status:** Accepted

**Context:** Vault supports two KV engine versions. KVv1 is a simple key-value store with no versioning, no soft-delete, and no Check-And-Set. KVv2 adds versioning, soft-delete/undelete, CAS for conflict detection, and per-secret metadata.

**Decision:** Target KVv2 exclusively. Detect KVv1 engines and return a clear error message rather than silently misbehaving.

**Rationale:**
- Core features (`edit` with CAS, `undelete`, version history via `metadata`) are impossible on KVv1.
- KVv1 is the legacy engine; most modern deployments use KVv2.
- Supporting both doubles the API surface and testing matrix for minimal user benefit.
- Clean detection + error message is better than degraded functionality.

---

## D011 — Editor format: JSON

**Date:** 2026-07-12
**Status:** Accepted

**Context:** The `edit` command writes secrets to a temp file for `$EDITOR`. Need a default serialization format.

**Decision:** Default to JSON for editor temp files. Configurable via `editor_format` in config.

**Rationale:**
- Consistent with Vault's own default output format (`-format=json`).
- Round-trip safe — no YAML gotchas (e.g., `yes`/`no` becoming booleans, numbers losing quotes).
- User preference.

---

## D012 — CLI name: kivi

**Date:** 2026-07-12
**Status:** Accepted

**Context:** Need a short, memorable, unique CLI name for frequent daily use.

**Decision:** `kivi` — evokes "KV" (key-value), short, fun, easy to type, no conflicts with existing tools.

**Rationale:**
- 4 characters, pronounceable, googleable.
- No conflicts with existing CLI tools or HashiCorp products.
- `kivi get apps/service` reads naturally.

---

## D013 — Config: `mount` field for KV engine mount point

**Date:** 2026-07-12
**Status:** Accepted

**Context:** Vault KV commands require specifying the engine mount point (e.g., `vault kv -mount=apps get path/to/secret`). Users who always work within one mount shouldn't have to type it every time.

**Decision:** Config file includes a `mount` field (equivalent to `vault kv -mount=...`). Overridable via `--mount` / `-m` CLI flag.

**Rationale:**
- Most users work within a single mount (e.g., `apps`, `secret`).
- Eliminates repetitive typing.
- Follows the same precedence as other config: CLI flag > env var > config > default (`"secret"`).

---

## D014 — Config file auto-created on first run; no `init` or `edit` subcommands

**Date:** 2026-07-12
**Status:** Accepted

**Context:** Config needs to exist before commands can use it. Options: require explicit `config init`, auto-create, or just document the file format and let users create it manually.

**Decision:** Any command that needs the config file auto-creates it with built-in defaults if it doesn't exist. `config init` and `config edit` subcommands are dropped — `config show`, `config set`, and `config path` are sufficient.

**Rationale:**
- Zero-friction first run — `kivi get apps/svc` just works, no setup ceremony.
- `config edit` is redundant: the user can open `~/.config/kivi/config.toml` directly; `config path` tells them where it is.
- `config init` is redundant: auto-creation does the same thing implicitly.
- Fewer subcommands, smaller surface to document and maintain.

---

## D015 — Tab completion: 10s hard timeout, smart path parsing, no cache in v1

**Date:** 2026-07-12
**Status:** Accepted

**Context:** Tab completion is the core feature of kivi. Every Tab press invokes the binary and calls Vault directly (no cache in v1).

**Decision:**
- No completion cache in v1 — calls Vault API on every Tab press. Caching deferred as a future optimisation once real-world latency is measured.
- Vault API timeout during completion is **10 seconds, hard**. On timeout, return empty — never hang the shell. Not user-configurable.
- Completion is wired to every `<path>` argument across all commands.
- Directories (entries ending in `/`) and leaf secrets are distinguished so the shell knows whether to add a trailing space or continue completing.
- With `mount` set in config, users never type the mount prefix during completion.

**Rationale:**
- No cache keeps v1 simple — one less moving part to get right before the core behaviour is proven.
- Hard timeout and fail-open (empty results) ensures the shell never hangs regardless of Vault connectivity.
- Completing paths on all commands means muscle memory transfers across the whole tool.

---

## D016 — Concurrency-first implementation: maximise parallel Vault API calls

**Date:** 2026-07-12
**Status:** Accepted

**Context:** Many kivi operations require N Vault API calls (tree listing, recursive delete, `list --long` metadata fetches). Sequential calls are slow and Vault is designed to handle concurrent requests.

**Decision:** Any operation that makes N ≥ 2 independent Vault API calls MUST do so concurrently using `tokio::task::JoinSet` with a bounded concurrency limit (default: 16 in-flight requests). This applies to:
- `tree`: BFS directory listing (concurrent `LIST` per dir level) + concurrent secret reads at leaf nodes
- `list --long`: concurrent metadata fetch per entry
- `delete --recursive`: concurrent deletes after listing
- `completion`: single request per Tab press (no parallelism needed)

**Rationale:**
- Vault is an HTTP service behind a network — sequential requests waste wall-clock time proportional to N × latency.
- `tree` on a deep hierarchy with many secrets is the primary case: concurrent fan-out turns O(depth × breadth × latency) into O(depth × latency).
- Bounded concurrency (16) prevents overwhelming Vault or triggering rate limits.
- `tokio::task::JoinSet` gives clean cancellation and error propagation across concurrent tasks.
