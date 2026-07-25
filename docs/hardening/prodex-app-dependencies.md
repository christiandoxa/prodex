# `prodex-app` Dependency Inventory

Baseline: commit `c635d485cf750637512bfacb4e1cefc854ed1bef`, 90 direct normal
dependencies. `cargo machete --with-metadata --skip-target-dir
crates/prodex-app` reported no unused dependency. This inventory records why
the composition crate currently owns each edge; later slices must update the
table when an implementation moves behind a narrower crate.

| Dependency | Current app-owned reason |
| --- | --- |
| `anyhow` | Adds command, launch, filesystem, and runtime integration error context. |
| `arc-swap` | Atomically publishes validated runtime and gateway snapshots. |
| `aws-lc-rs` | Implements app-owned certificate/key and cryptographic gateway integration. |
| `base64` | Encodes and decodes provider, tunnel, guardrail, and protocol payloads. |
| `bytes` | Carries bounded HTTP, SSE, WebSocket, and provider buffers. |
| `chrono` | Formats and compares persisted/runtime timestamps. |
| `clap` | Integrates typed CLI values and command dispatch. |
| `crossterm` | Owns interactive terminal event handling and restoration. |
| `dirs` | Resolves platform user data/config locations for launch integration. |
| `fs2` | Coordinates app-owned cross-process file locks. |
| `getrandom` | Generates local capabilities, nonces, and secret material. |
| `http-body-util` | Adapts gateway/runtime HTTP bodies. |
| `jsonwebtoken` | Verifies and decodes app-owned OIDC/workload tokens. |
| `os_info` | Supplies bounded doctor and diagnostic platform metadata. |
| `portable-pty` | Runs browser/expose and interactive child processes through PTYs. |
| `postgres` | Wires synchronous PostgreSQL migration and repository paths at composition. |
| `prodex-app-reports` | Renders app-owned quota, status, doctor, and command reports. |
| `prodex-application` | Invokes enterprise use cases and ports. |
| `prodex-audit-log` | Appends and queries Prodex-owned audit events. |
| `prodex-authn` | Wires gateway/control-plane authentication adapters. |
| `prodex-authz` | Wires gateway/control-plane authorization adapters. |
| `prodex-bench-support` | Exposes optional production-shaped benchmark fixtures. |
| `prodex-caveman-assets` | Prepares embedded Caveman/Super overlays; scheduled for deletion. |
| `prodex-cli` | Owns parsed commands, launch arguments, and help contracts. |
| `prodex-codex-config` | Reads Codex provider/model/profile configuration. |
| `prodex-config` | Loads typed enterprise deployment configuration. |
| `prodex-context` | Runs context audit/compact commands and Smart Context validation helpers. |
| `prodex-control-plane` | Composes control-plane domain/application operations. |
| `prodex-core` | Resolves Prodex paths and common filesystem rules. |
| `prodex-domain` | Uses canonical enterprise identifiers, decisions, and error values. |
| `prodex-gateway-core` | Builds canonical gateway request/response and policy contracts. |
| `prodex-gateway-http` | Adapts gateway HTTP requests and responses. |
| `prodex-gateway-server` | Starts and controls the async gateway server. |
| `prodex-housekeeping` | Implements cleanup and duplicate detection commands. |
| `prodex-mcp-stdio` | Adapts Prodex MCP stdio command transport. |
| `prodex-observability` | Wires metrics, traces, logs, and export sinks. |
| `prodex-presidio` | Starts, checks, and applies optional Presidio redaction. |
| `prodex-profile-export` | Implements encrypted profile import/export orchestration. |
| `prodex-profile-identity` | Parses account identity and normalizes profile names. |
| `prodex-provider-core` | Uses provider catalog metadata and pure protocol transforms. |
| `prodex-provider-spi` | Composes validated provider invocation/runtime contracts. |
| `prodex-proxy-config` | Resolves upstream proxy/client policy. |
| `prodex-quota` | Fetches and classifies account quota. |
| `prodex-redaction` | Redacts app diagnostics, logs, arguments, and payload summaries. |
| `prodex-runtime-anthropic` | Wires Anthropic-compatible translation and streaming. |
| `prodex-runtime-broker` | Starts and observes the local runtime broker. |
| `prodex-runtime-broker-log` | Parses broker logs for status and doctor output. |
| `prodex-runtime-capabilities` | Detects route/request runtime compatibility. |
| `prodex-runtime-claude` | Plans Claude Code launch configuration. |
| `prodex-runtime-cookies` | Relays profile-scoped runtime cookies. |
| `prodex-runtime-doctor` | Produces runtime diagnostic summaries. |
| `prodex-runtime-gemini-cli-compat` | Maintains native Gemini CLI compatibility state. |
| `prodex-runtime-launch` | Builds child-process and runtime launch plans. |
| `prodex-runtime-log` | Resolves and writes structured runtime logs and markers. |
| `prodex-runtime-metrics` | Aggregates and renders runtime broker metrics. |
| `prodex-runtime-policy` | Loads, validates, caches, and publishes runtime policy. |
| `prodex-runtime-proxy` | Uses pure proxy classifiers, contracts, and Smart Context logic. |
| `prodex-runtime-quota` | Adapts quota state into runtime selection inputs. |
| `prodex-runtime-state` | Owns live runtime counters, bindings, and snapshots. |
| `prodex-runtime-store` | Merges and compacts persisted runtime state. |
| `prodex-runtime-tuning` | Resolves bounded runtime tuning and fault injection. |
| `prodex-secret-store` | Reads/writes bounded private secrets and refresh leases. |
| `prodex-session-store` | Persists and queries shared Codex session metadata. |
| `prodex-shared-codex-fs` | Prepares shared Codex state and compatibility links. |
| `prodex-shared-types` | Exchanges serializable command/runtime DTOs. |
| `prodex-state` | Loads and mutates the profile/application state model. |
| `prodex-storage` | Uses backend-neutral enterprise repository contracts. |
| `prodex-storage-postgres` | Validates PostgreSQL backend configuration and migrations. |
| `prodex-storage-postgres-runtime` | Constructs PostgreSQL repository adapters. |
| `prodex-storage-redis` | Validates Redis backend configuration and schema. |
| `prodex-storage-redis-runtime` | Constructs Redis repository adapters. |
| `prodex-storage-sqlite` | Validates SQLite backend configuration and migrations. |
| `prodex-storage-sqlite-runtime` | Constructs SQLite repository adapters. |
| `prodex-terminal-ui` | Owns terminal layout/session/printing primitives used by the app. |
| `prodex-update-notice` | Checks and renders bounded release update notices. |
| `ratatui` | Builds app-owned interactive terminal screens. |
| `redis` | Wires live Redis coordination at the outer composition boundary. |
| `reqwest` | Performs bounded HTTP/OIDC/provider/admin integration calls. |
| `rpassword` | Reads secret CLI input without terminal echo. |
| `rusqlite` | Wires app-owned SQLite/session/desktop integration. |
| `serde` | Serializes typed app/runtime state. |
| `serde_json` | Parses and emits JSON protocols, state, and reports. |
| `sha2` | Computes security-sensitive checksums and fingerprints. |
| `tiny_http` | Serves small synchronous local callback/compatibility endpoints. |
| `tokio` | Runs async gateway, proxy, and provider orchestration. |
| `toml` | Parses and writes Codex/Prodex configuration overlays. |
| `tungstenite` | Handles app-owned upstream WebSocket integration. |
| `uuid` | Creates and validates request, resource, and correlation identifiers. |
| `zeroize` | Clears app-owned secret buffers. |
| `zstd` | Compresses bounded persisted/runtime payloads. |

The table describes current ownership, not the desired dependency direction.
An edge should disappear from `prodex-app` when its final implementation caller
moves behind a focused crate; a facade-only re-export is not sufficient.
