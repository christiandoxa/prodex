# Rust-to-Mojo migration inventory

Audit basis: workspace source as of 2026-08-28, immutable release baseline
`6f0f632a178492647da764e3522ff2092db40fb3`, `Cargo.toml`, maintained architecture and testing
contracts, the code knowledge graph, and direct source inspection. The
workspace contains 59 Cargo packages. This inventory classifies ownership boundaries,
not every generated fixture or test helper. Status `MOJO` means compiled Mojo is
authoritative on the supported Mojo target: a feature-off Rust implementation is a
separate Rust-only build or test oracle, never runtime fallback.

`MOVE_NOW` means a narrow pure slice has verified Mojo language/ABI support and a
useful boundary. `REFACTOR_THEN_MOVE` means the logic is a candidate only after IO,
security, and persistence are separated. `EXPERIMENT` means a bounded parity spike is
needed. `KEEP_RUST` means Rust currently owns the ecosystem or trust boundary.

## 0.419.1 accounting freeze

The frozen source-level accounting is recorded in
`migration/mojo-ownership-baseline-0.419.1.json` and checked by
`node scripts/ci/mojo-ownership.mjs --check`. The historical 0.419.0 record is preserved in
`migration/mojo-ownership-baseline-0.419.0.json`; the new 0.419.1 baseline contains 4,727
eligible Rust deterministic production semantic LOC and 4,959 eligible Mojo LOC. The
migration-volume floor is `ceil(4,727 * 10 / 100) = 473` LOC. The counter excludes blank/comment/import/directive
lines, test or `cfg(not(feature=...))` Rust bodies, and anything outside declared semantic
ranges; the release inventory cannot reduce the Rust denominator without a source-traceable
reduction record. Mojo volume is the eligible production Mojo semantic LOC, and existing Mojo
ownership and authoritative operations must not regress.

The 0.419.1 wave records 488 counted migrated Rust semantic LOC (10.32%), leaving 4,227
eligible Rust semantic LOC in the release inventory. The complete release inventory is 500 LOC
lower than the frozen Rust inventory because it also contains smaller traceable reductions outside
the counted target-operation volume. It adds or expands eight authoritative
units across routing, Smart Context, quota/capacity, runtime capacity, context classification,
and candidate planning. The release report is generated from the manifest; these figures are
not claims about the historical 0.419.0 wave.

| Package / module area | Purpose | Pure vs IO | External / async | Risk | Class | Action | Evidence / reason |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `prodex-quota::render::remaining_percent` | Convert used quota percent to bounded remaining percent | Pure | None | Low | A | `MOVE_NOW` | Scalar integer function; parity-covered Mojo C ABI slice |
| `prodex-quota::render::{quota_window_status,quota_pressure_band_from_windows}` | Classify quota windows and aggregate pressure | Pure | None | Low | A | `MOVE_NOW` | Explicit status/band tags; parity-covered in the existing quota bridge |
| `prodex-quota::render::window_pair_has_ready_limit` | Decide whether a quota window pair is usable | Pure | None | Low | A | `MOVE_NOW` | Four scalar inputs in one batched ABI call; model lookup stays Rust |
| `prodex-runtime-proxy::runtime_proxy_quota_pressure_band_for_route` | Apply route-specific quota thresholds to two observations | Pure | None | Medium | A/B | `MOVE_NOW` | Batch scalar ABI preserves route-specific thresholds; affinity and transport remain Rust |
| `prodex-runtime-quota::schedule_ready_profile_candidates_with_view` | Score and order ready profiles | Pure after Rust state normalization | profile state view and clock | High | B | `MOVE_NOW` | One Mojo batch derives scaled pressure, reserve bias, near-optimal and preferred-profile hysteresis, then returns stable indices; Rust retains state reads, names, clock, and persisted preference state |
| `prodex-provider-spi::score_providers` | Score already-eligible provider descriptors | Pure after hard filtering | normalized provider signals and weights | High | B | `MOVE_NOW` | Mojo score arithmetic is now part of the complete bounded routing-plan batch; Rust still owns normalization and policy inputs |
| `prodex-provider-spi::plan_governed_provider_route` | Filter, score, and order governed provider candidates | Pure after Rust validation and policy normalization | capability masks, provider order, normalized signals, and weights | High | B | `MOVE_NOW` | One batch covers up to 64 candidates; Mojo applies capability eligibility and stable ordering, while Rust retains hard policy gates, credentials, affinity, route construction, and error semantics |
| `prodex-provider-spi::negotiate_provider_route_capability` | Match provider routes to required model capabilities | Pure after Rust enum-to-mask normalization | well-formed flags and seven-bit capability masks | High | B | `MOVE_NOW` | Separate flat-buffer capability batch returns first compatible/incompatible indices; Rust retains route/model selection, missing-capability details, and redacted errors |
| `prodex-runtime-proxy::smart_context_estimate_tokens_from_body_bytes` | Estimate token budget from body size | Pure | None | Low | A | `MOVE_NOW` | Deterministic `u64` arithmetic; real Mojo parity and release diagnostic coverage |
| `prodex-runtime-proxy::smart_context_pressure_snapshot` | Calculate effective context capacity, pressure, safety floor, and estimator confidence | Pure after Rust token/risk normalization | Fixed-width token values and tags | Medium | A/B | `MOVE_NOW` | `smart_context_observed_token_accounting_with_calibration` now consumes one Mojo batch-style scalar decision; Rust retains token estimation, model lookup, and risk collection; 300 generated parity cases |
| `prodex-runtime-proxy::build_runtime_response_candidate_execution_plan` | Order ready and fallback runtime candidates | Pure after Rust state normalization | Bounded normalized candidate fields | High | B | `MOVE_NOW` | One bounded candidate batch plus one affinity batch for up to 256 candidates; Mojo returns authoritative availability, affinity, ready/fallback indices; Rust retains profile exclusion, state acquisition, and transport |
| `prodex-quota::render::{windows,gemini,pool,reports}` | Quota summaries, rendering, reset calculations | Mostly pure | `chrono`, terminal formatting | Medium | A/B | `MOVE_NOW` for normalized main-pool aggregation; `KEEP_RUST` for conversion/formatting | `prodex_quota_main_aggregate_batch` is called by the active pool renderer over normalized Gemini/Copilot rows; chrono, float conversion, labels, and rendering stay Rust |
| `prodex-domain::accounting::commit_reservation` | Commit normalized reservation arithmetic | Pure helper, not the durable production path | Fixed-width usage values | High | B | `KEEP_RUST` | The production accounting flow uses durable `reconcile_reserved_usage`; the generic helper has no safe Mojo production seam and the domain boundary remains Rust-only |
| `prodex-domain::rate_limit::evaluate_rate_limit` | Evaluate bounded request-window admission arithmetic | Pure helper, not distributed admission | Fixed-width counters and timestamps | High | B | `KEEP_RUST` | Production rate limiting is owned by Redis/runtime adapters; tenant ownership, clocks, and persistence stay Rust |
| `prodex-domain::{accounting_budget,slo}` | Budget and SLO decisions | Pure plans plus identifiers | `serde`, UUID types, float semantics | High | A/B | `REFACTOR_THEN_MOVE` | Candidate arithmetic remains separate; SLO float parity and budget enforcement contract need explicit probes |
| `prodex-domain::{policy,security,secrets,identity,ids}` | Signed policy, tenant identity, secret-safe IDs | Pure-looking models | hashes, UUID, zeroization | Critical | C | `KEEP_RUST` | Trust boundary and established Rust security behavior |
| `prodex-domain::{audit,governance,observability}` | Audit, governance, telemetry contracts | Pure plans plus sensitive data | hashing, serialization | Critical | B/C | `KEEP_RUST` | Preserve redaction, authorization, and audit invariants |
| `prodex-provider-core::{catalog,constraints,models}` | Provider capabilities and model constraints | Mostly pure | `serde`, provider data | High | A/B | `MOVE_NOW` for normalized constraints; `KEEP_RUST` for parsing/catalog/provider adapter | `prodex_provider_constraints_evaluate_v2` is called by the active public constraint evaluator through a typed, versioned flat-buffer wrapper after Rust normalization; 2,000 generated differential cases and full provider-core suite pass |
| `prodex-provider-core::translators` | Provider request/response translation | Mixed | JSON/provider schemas | High | C | `KEEP_RUST` | Ecosystem and compatibility-heavy; no rewrite justified |
| `prodex-provider-spi::governed_routing` | Governed provider eligibility and routing | Pure decision over normalized policy snapshots | Domain/provider contracts | Critical | B | `MOVE_NOW` | Mojo owns the bounded filter/score/order batch; Rust retains hard security gates, credentials, affinity, route construction, and errors |
| `prodex-runtime-proxy::{selection_plan,smart_context}` | Candidate selection, scoring, context algorithms | Mixed | tokenizers, runtime state | Critical | B | `MOVE_NOW` for optimistic decision and rehydration admission; `KEEP_RUST` for affinity/state/transport and dormant float scorer | Optimistic decision and active `smart_context_auto_rehydrate_plan` callers now reach Mojo after string/identity normalization; 5,000 and 2,000 exact generated cases pass |
| `prodex-runtime-proxy::{failure_response,gateway_policy}` | Proxy policy and failure semantics | Pure helpers | HTTP-neutral but hot path | Critical | C | `KEEP_RUST` | Runtime invariants require Rust oracle and extensive replay coverage |
| `prodex-application::{data_plane,provider,governance,request_context}` | Side-effect-free application plans and ports | Pure plans | security/policy contracts | Critical | B | `REFACTOR_THEN_MOVE` | Move only isolated calculations after boundary tests |
| `prodex-runtime-policy::validate*` | Semantic runtime policy validation | Pure validation over config | TOML/config models | High | B | `MOVE_NOW` for normalized numeric rules; `KEEP_RUST` for security/string validation | One bounded Mojo numeric batch validates active runtime-proxy bounds and governance session ranges; Rust preserves parsing, paths, exact errors, selectors, secrets, signatures, and policy gates |
| `prodex-runtime-quota` | Runtime quota snapshots and adapter helpers | Mixed | runtime state, time | High | B | `MOVE_NOW` for normalized window summaries; `KEEP_RUST` for clocks/state/adapters | Active proxy summary classification now reuses compiled quota status/band kernels; runtime state and time remain Rust |
| `prodex-context` | Context audit, noise filtering, aggregation | Mixed | filesystem/process output | Medium | B | `MOVE_NOW` for critical-signal text grouping | Rust collects and classifies non-secret diagnostic lines; Mojo validates UTF-8, compares borrowed text, groups duplicates, and constructs bounded row plans |
| `prodex-context::critical_signal_self_check` | Compare critical-signal text and counters | Pure after Rust terminal normalization/classification | None | Medium | A/B | `MOVE_NOW` for text grouping and loss/gain arithmetic; `KEEP_RUST` for classifiers | One text/record call plus the existing counter call are active in compaction and Smart Context validation; exact Rust grouping remains a test oracle |
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
| `prodex-runtime-{doctor,log,metrics,capabilities,tuning}` | Diagnostics, logs, metrics, policy tuning | Mixed | filesystem, time, terminal | High | B/C | `MOVE_NOW` for normalized tuning defaults and bounded log-event classification; `KEEP_RUST` for diagnostics/capabilities/formatting | `prodex_runtime_tuning_defaults` and `prodex_mojo_log_classify_v3` are active callers; environment parsing, file IO, redaction, and rendering stay Rust |
| `prodex-terminal-ui`, `prodex-app::reports` | Generic terminal layout and app-owned report rendering | Pure rendering plus terminal | Crossterm/Ratatui | Medium | C | `KEEP_RUST` | Keep presentation and width behavior in Rust; the single-consumer report crate moved into its app owner |
| `prodex-audit-log`, `prodex-observability`, `prodex-redaction` | Audit, telemetry, redaction | Mixed/security | hashing, serialization | Critical | C | `KEEP_RUST` | Preserve data-leak and audit guarantees |
| `prodex-optional-tools`, `prodex-housekeeping`, `prodex-update-notice` | Local tooling, cleanup, update checks | IO | processes/filesystem/network | High | C | `KEEP_RUST` | No Mojo benefit over Rust stdlib/dependencies |
| `prodex-bench-support`, benches, tests, scripts | Validation and build/test orchestration | Test/IO | Cargo/Node/processes | High | C | `KEEP_RUST` | Test harness remains Rust/Node; parity tests call both cores |

## Initial conclusion

Quota arithmetic remains the first validated seam. Runtime policy numeric validation and profile
scheduling now also have active Mojo batches. Provider routing now has a complete
candidate-plan batch after Rust validation and normalization, plus a separate capability-mask
batch. Runtime candidate ordering and Smart Context pressure accounting are now additional
production Mojo kernels. Optimistic selection, provider constraints, Smart Context rehydration,
quota aggregation, runtime tuning defaults, and critical-signal UTF-8 duplicate grouping are now
production Mojo components as well. The
generic accounting and rate-limit helpers remain Rust-only because
their actual production owners are durable storage and Redis/runtime admission. Mojo does not
own policy, credentials, affinity, tenant ownership, transport, route construction, durable
mutation, secrets, or user-facing errors. The dormant Smart Context candidate scorer/selector remains an
audit-only candidate until a non-test production caller exists.

## Rich domain promotion on 2026-08-26

These rows supersede the earlier normalization-only descriptions. They are active production
callers under a Mojo-enabled build; a Mojo result error is a hard internal error and never causes
Rust to recompute the semantic answer.

| Operation | Mojo semantic ownership | Rust ownership | Consumer |
| --- | --- | --- | --- |
| `prodex-context::count_critical_signals` | UTF-8 validation, CR/LF handling, ANSI skipping, Unicode trimming, diagnostic classification, token counting, duplicate grouping, and context groups | command/process collection, public report mapping, and Rust-only oracle | `prodex-context` |
| `prodex-provider-core::provider_model_fallback_chain` | `combo:` scanner, separators, optional/empty components, case-folded deduplication, provider aliases, and ordered model records | provider ID API and transport | `prodex-provider-core` |
| `prodex-runtime-policy::validate_gateway_route_alias` | alias/model/strategy grammar, identifier validation, metric-to-model relationships, and structured issues | TOML/Serde, paths, numeric rules, secrets, and final errors | `prodex-runtime-policy` |
| `prodex-provider-spi::plan_governed_provider_route` | provider/capability text normalization, capability interpretation, candidate objects, deduplication, score components, ranking, and reasons | hard tenant/policy/security filters, credentials, affinity, routes, and transport | `prodex-provider-spi` |
| `prodex-runtime-proxy::smart_context_auto_rehydrate_plan` | opaque artifact-reference set lookup, context-item objects, budget admission, and action records | artifact acquisition, confidentiality boundary, ordering compatibility, and IO | `prodex-runtime-proxy` |
| `prodex-app::classify_runtime_log_event` | bounded event-key classification and severity mapping for operational log rendering | file IO, redaction, event-field extraction, and terminal/JSON rendering | `prodex-app` |

The original scalar and text ABI rows remain historically accurate as the first migration wave.
Rich ABI v6 is now the semantic boundary. JSON, TOML deserialization, credentials, prompts,
filesystem paths, and provider wire payloads remain Rust-owned after the package/runtime review.

## 0.419.1 migration wave

Measured from immutable baseline `6f0f632a178492647da764e3522ff2092db40fb3` (the 0.419.0
product state), the release transfers 488 baseline Rust semantic LOC into Mojo-authoritative
production paths: gateway route selection, adaptive and calibrated Smart Context planning,
quota capacity-lane planning, runtime capacity defaults, quota route scoring, critical-signal
classification, and candidate availability/prompt-cache planning. The release retains Rust for
profile and quota acquisition, clocks, async orchestration, file/network/process IO, persistence,
credentials, security, and ABI/result validation. Parity evidence and final-state records live
in `migration/mojo-ownership.json`; the superseded Rust code is either removed from the Mojo
production path or retained only as a feature-off/test oracle where the repository's Rust-only
compatibility build requires it.
