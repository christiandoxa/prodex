# Rust-to-Mojo migration inventory

Audit basis: workspace source as of 2026-08-20, `Cargo.toml`, maintained architecture
and testing contracts, the code knowledge graph, and direct source inspection. The
workspace contains 58 Cargo packages. This inventory classifies ownership boundaries,
not every generated fixture or test helper.

`MOVE_NOW` means a narrow pure slice has verified Mojo language/ABI support and a
useful boundary. `REFACTOR_THEN_MOVE` means the logic is a candidate only after IO,
security, and persistence are separated. `EXPERIMENT` means a bounded parity spike is
needed. `KEEP_RUST` means Rust currently owns the ecosystem or trust boundary.

| Package / module area | Purpose | Pure vs IO | External / async | Risk | Class | Action | Evidence / reason |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `prodex-quota::render::remaining_percent` | Convert used quota percent to bounded remaining percent | Pure | None | Low | A | `MOVE_NOW` | Scalar integer function; parity-covered Mojo C ABI slice |
| `prodex-quota::render::{windows,gemini,pool,reports}` | Quota summaries, rendering, reset calculations | Mostly pure | `chrono`, terminal formatting | Medium | A/B | `EXPERIMENT` | Batch calculations may benefit; formatting and time stay Rust |
| `prodex-domain::{accounting,accounting_budget,rate_limit,slo}` | Usage, budget, rate-limit, SLO decisions | Pure plans plus identifiers | `serde`, UUID types | High | A/B | `REFACTOR_THEN_MOVE` | Good algorithms, but accounting and policy are security-sensitive |
| `prodex-domain::{policy,security,secrets,identity,ids}` | Signed policy, tenant identity, secret-safe IDs | Pure-looking models | hashes, UUID, zeroization | Critical | C | `KEEP_RUST` | Trust boundary and established Rust security behavior |
| `prodex-domain::{audit,governance,observability}` | Audit, governance, telemetry contracts | Pure plans plus sensitive data | hashing, serialization | Critical | B/C | `KEEP_RUST` | Preserve redaction, authorization, and audit invariants |
| `prodex-provider-core::{catalog,constraints,models}` | Provider capabilities and model constraints | Mostly pure | `serde`, provider data | High | A/B | `REFACTOR_THEN_MOVE` | Candidate after provider-independent input normalization |
| `prodex-provider-core::translators` | Provider request/response translation | Mixed | JSON/provider schemas | High | C | `KEEP_RUST` | Ecosystem and compatibility-heavy; no rewrite justified |
| `prodex-provider-spi::governed_routing` | Governed provider eligibility and routing | Pure decision over policy snapshots | Domain/provider contracts | Critical | B | `REFACTOR_THEN_MOVE` | Strong algorithmic seam, but hard security gates stay Rust initially |
| `prodex-runtime-proxy::{selection_plan,smart_context}` | Candidate selection, scoring, context algorithms | Mixed | tokenizers, runtime state | Critical | B | `EXPERIMENT` | Possible batch kernels; hard affinity and transport remain Rust |
| `prodex-runtime-proxy::{failure_response,gateway_policy}` | Proxy policy and failure semantics | Pure helpers | HTTP-neutral but hot path | Critical | C | `KEEP_RUST` | Runtime invariants require Rust oracle and extensive replay coverage |
| `prodex-application::{data_plane,provider,governance,request_context}` | Side-effect-free application plans and ports | Pure plans | security/policy contracts | Critical | B | `REFACTOR_THEN_MOVE` | Move only isolated calculations after boundary tests |
| `prodex-runtime-policy::validate*` | Semantic runtime policy validation | Pure validation over config | TOML/config models | High | B | `EXPERIMENT` | Parse/config IO stays Rust; validation needs exhaustive parity |
| `prodex-runtime-quota` | Runtime quota snapshots and adapter helpers | Mixed | runtime state, time | High | B | `EXPERIMENT` | Candidate summaries after quota core proves stable |
| `prodex-context` | Context audit, noise filtering, aggregation | Mixed | filesystem/process output | Medium | B | `EXPERIMENT` | Ranking/dedup may move; collection of inputs stays Rust |
| `prodex-state`, `prodex-runtime-state` | Profile/runtime state models and snapshots | Models plus persistence-facing state | `serde`, state contracts | High | B | `KEEP_RUST` | Cross-process merge semantics and serialization remain Rust |
| `prodex-runtime-store`, `prodex-session-store` | State/session persistence and merge | IO/stateful | filesystem, JSON | Critical | C | `KEEP_RUST` | Durable writes and affinity persistence are Rust-owned |
| `prodex-gateway-core` | HTTP-neutral admission/routing contracts | Mostly pure | no transport | Critical | B | `REFACTOR_THEN_MOVE` | Only after gateway parity and security review |
| `prodex-gateway-http` | Framework-neutral HTTP policy | Mixed | HTTP types | High | C | `KEEP_RUST` | Mature Rust HTTP boundary already exists |
| `prodex-gateway-server` | Hyper/TLS serving and connection handling | IO/async | Hyper, Tokio, rustls | Critical | C | `KEEP_RUST` | Explicit ecosystem-heavy keep decision |
| `prodex-storage*` | Adapter-neutral plans and DB/Redis drivers | IO/stateful | SQLite, PostgreSQL, Redis, async | Critical | C | `KEEP_RUST` | Do not recreate mature drivers or wire protocols |
| `prodex-cli`, `prodex-app`, root binaries | CLI parsing, command routing, orchestration | IO/orchestration | Clap, Tokio, processes | Critical | C | `KEEP_RUST` | Rust remains host application |
| `prodex-authn`, `prodex-secret-store`, `prodex-presidio` | OAuth, credentials, redaction | Security/IO | crypto, browser, secret stores | Critical | C | `KEEP_RUST` | Secrets never cross the initial Mojo boundary |
| `prodex-profile-*`, `prodex-shared-codex-fs`, `prodex-core` | Profile/path/filesystem operations | IO | filesystem, platform | High | C | `KEEP_RUST` | OS semantics and secret paths remain Rust-owned |
| `prodex-runtime-{launch,broker,claude,anthropic,gemini-cli-compat}` | Child processes, runtime providers, broker | IO/async | subprocesses, provider CLIs | Critical | C | `KEEP_RUST` | Process lifecycle and provider compatibility stay Rust |
| `prodex-runtime-{doctor,log,metrics,capabilities,tuning}` | Diagnostics, logs, metrics, policy tuning | Mixed | filesystem, time, terminal | High | B/C | `KEEP_RUST` | Pure formatting may remain Rust until useful batch seam appears |
| `prodex-terminal-ui`, `prodex-app-reports` | Generic terminal/report rendering | Pure rendering plus terminal | Crossterm/Ratatui | Medium | C | `KEEP_RUST` | Keep presentation and width behavior in Rust |
| `prodex-audit-log`, `prodex-observability`, `prodex-redaction` | Audit, telemetry, redaction | Mixed/security | hashing, serialization | Critical | C | `KEEP_RUST` | Preserve data-leak and audit guarantees |
| `prodex-optional-tools`, `prodex-housekeeping`, `prodex-update-notice` | Local tooling, cleanup, update checks | IO | processes/filesystem/network | High | C | `KEEP_RUST` | No Mojo benefit over Rust stdlib/dependencies |
| `prodex-bench-support`, benches, tests, scripts | Validation and build/test orchestration | Test/IO | Cargo/Node/processes | High | C | `KEEP_RUST` | Test harness remains Rust/Node; parity tests call both cores |

## Initial conclusion

The first production-worthy seam is quota arithmetic, not routing. `remaining_percent`
is small enough to validate ABI and parity without exposing a Rust object graph. The
next candidate is a batch form of quota or Smart Context scoring only after measuring
that one-call-per-value FFI overhead and extracting a complete input/output contract.
