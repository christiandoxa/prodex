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
| `prodex-quota::render::{quota_window_status,quota_pressure_band_from_windows}` | Classify quota windows and aggregate pressure | Pure | None | Low | A | `MOVE_NOW` | Explicit status/band tags; parity-covered in the existing quota bridge |
| `prodex-quota::render::window_pair_has_ready_limit` | Decide whether a quota window pair is usable | Pure | None | Low | A | `MOVE_NOW` | Four scalar inputs in one batched ABI call; model lookup stays Rust |
| `prodex-runtime-proxy::runtime_proxy_quota_pressure_band_for_route` | Apply route-specific quota thresholds to two observations | Pure | None | Medium | A/B | `MOVE_NOW` | Batch scalar ABI preserves route-specific thresholds; affinity and transport remain Rust |
| `prodex-runtime-proxy::runtime_proxy_quota_profile_scores_batch` | Score bounded profile quota pressure inputs for selection | Pure | None after normalization | Medium | A | `MOVE_NOW` | One flat-buffer batch call; Rust saturation oracle retained; selection ordering remains Rust |
| `prodex-provider-spi::score_providers` | Score already-eligible provider descriptors | Pure after hard filtering | normalized provider signals and weights | High | B | `MOVE_NOW` | Mojo score arithmetic is now part of the complete bounded routing-plan batch; Rust still owns normalization and policy inputs |
| `prodex-provider-spi::plan_governed_provider_route` | Filter, score, and order governed provider candidates | Pure after Rust validation and policy normalization | capability masks, provider order, normalized signals, and weights | High | B | `MOVE_NOW` | One batch covers up to 64 candidates; Mojo applies capability eligibility and stable ordering, while Rust retains hard policy gates, credentials, affinity, route construction, and error semantics |
| `prodex-provider-spi::negotiate_provider_route_capability` | Match provider routes to required model capabilities | Pure after Rust enum-to-mask normalization | well-formed flags and seven-bit capability masks | High | B | `MOVE_NOW` | Separate flat-buffer capability batch returns first compatible/incompatible indices; Rust retains route/model selection, missing-capability details, redacted errors, and fallback |
| `prodex-runtime-proxy::smart_context_estimate_tokens_from_body_bytes` | Estimate token budget from body size | Pure | None | Low | A | `MOVE_NOW` | Deterministic `u64` arithmetic; real Mojo parity and release diagnostic coverage |
| `prodex-runtime-proxy::smart_context_pressure_snapshot` | Calculate effective context capacity, pressure, safety floor, and estimator confidence | Pure after Rust token/risk normalization | Fixed-width token values and tags | Medium | A/B | `MOVE_NOW` | `smart_context_observed_token_accounting_with_calibration` now consumes one Mojo batch-style scalar decision; Rust retains token estimation, model lookup, and risk collection; 300 generated parity cases |
| `prodex-runtime-proxy::build_runtime_response_candidate_execution_plan` | Order ready and fallback runtime candidates | Pure after Rust exclusion, quota, and affinity normalization | Bounded normalized candidate fields | High | B | `MOVE_NOW` | One 22-field batch for up to 256 candidates; Mojo returns authoritative ready/fallback indices; Rust retains excluded-profile filtering, quota guard construction, affinity, health acquisition, and transport |
| `prodex-quota::render::{windows,gemini,pool,reports}` | Quota summaries, rendering, reset calculations | Mostly pure | `chrono`, terminal formatting | Medium | A/B | `EXPERIMENT` | Batch calculations may benefit; formatting and time stay Rust |
| `prodex-domain::accounting::commit_reservation` | Commit normalized reservation arithmetic | Pure helper, not the durable production path | Fixed-width usage values | High | B | `KEEP_RUST` | The production accounting flow uses durable `reconcile_reserved_usage`; the generic helper has no safe Mojo production seam and the domain boundary remains Rust-only |
| `prodex-domain::rate_limit::evaluate_rate_limit` | Evaluate bounded request-window admission arithmetic | Pure helper, not distributed admission | Fixed-width counters and timestamps | High | B | `KEEP_RUST` | Production rate limiting is owned by Redis/runtime adapters; tenant ownership, clocks, and persistence stay Rust |
| `prodex-domain::{accounting_budget,slo}` | Budget and SLO decisions | Pure plans plus identifiers | `serde`, UUID types, float semantics | High | A/B | `REFACTOR_THEN_MOVE` | Candidate arithmetic remains separate; SLO float parity and budget enforcement contract need explicit probes |
| `prodex-domain::{policy,security,secrets,identity,ids}` | Signed policy, tenant identity, secret-safe IDs | Pure-looking models | hashes, UUID, zeroization | Critical | C | `KEEP_RUST` | Trust boundary and established Rust security behavior |
| `prodex-domain::{audit,governance,observability}` | Audit, governance, telemetry contracts | Pure plans plus sensitive data | hashing, serialization | Critical | B/C | `KEEP_RUST` | Preserve redaction, authorization, and audit invariants |
| `prodex-provider-core::{catalog,constraints,models}` | Provider capabilities and model constraints | Mostly pure | `serde`, provider data | High | A/B | `REFACTOR_THEN_MOVE` | Candidate after provider-independent input normalization |
| `prodex-provider-core::translators` | Provider request/response translation | Mixed | JSON/provider schemas | High | C | `KEEP_RUST` | Ecosystem and compatibility-heavy; no rewrite justified |
| `prodex-provider-spi::governed_routing` | Governed provider eligibility and routing | Pure decision over normalized policy snapshots | Domain/provider contracts | Critical | B | `MOVE_NOW` | Mojo owns the bounded filter/score/order batch; Rust retains hard security gates, credentials, affinity, route construction, and errors |
| `prodex-runtime-proxy::{selection_plan,smart_context}` | Candidate selection, scoring, context algorithms | Mixed | tokenizers, runtime state | Critical | B | `EXPERIMENT` | Runtime candidate ordering and pressure snapshot are now production Mojo seams; dormant candidate scoring/selection has no non-test caller, while tokenizer integration, hard affinity, and transport remain Rust |
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

Quota arithmetic remains the first validated seam. Provider routing now has a complete
candidate-plan batch after Rust validation and normalization, plus a separate capability-mask
batch. Runtime candidate ordering and Smart Context pressure accounting are now additional
production Mojo kernels. The generic accounting and rate-limit helpers remain Rust-only because
their actual production owners are durable storage and Redis/runtime admission. Mojo does not
own policy, credentials, affinity, tenant ownership, transport, route construction, durable
mutation, or user-facing errors. The dormant Smart Context candidate scorer/selector remains an
audit-only candidate until a non-test production caller exists.
