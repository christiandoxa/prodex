# Migration decisions

## 2026-08-20: Rust remains the host

CLI parsing, orchestration, async runtime, HTTP, TLS, persistence, subprocesses,
credentials, terminal lifecycle, and provider adapters remain Rust. This preserves the
existing ecosystem and Prodex runtime invariants.

## 2026-08-20: First seam is quota arithmetic

`remaining_percent` is deterministic, scalar-only, already tested by its callers, and
has no provider, filesystem, time, or security dependency. It proves the build and ABI
without inventing a domain model or exposing a Rust object graph.

## 2026-08-20: Feature is opt-in and component-scoped

`prodex-quota/mojo` is disabled by default. A normal `cargo build` therefore stays
independent of Mojo. An opt-in build requires `mojo` and `ar` on `PATH`, or an explicitly
configured prebuilt archive; missing tools and Mojo compile errors fail the build instead
of selecting a Rust implementation. Runtime diagnostics intentionally report
`compiler_required=false`; `build_strict` reflects `PRODEX_MOJO_REQUIRED` and may be true
only for the build gate. A shipped binary can therefore report `compiler_required=false`
and `build_strict=true`.

## 2026-08-20: Use object output for this scalar

The local compiler successfully emits an object and Rust links it directly. This avoids
shared-library loader paths and observed runtime dependencies for the scalar function.
Object emission is documented as experimental, so broader migrations must revalidate
this choice or move to a supported artifact/distribution path.

## 2026-08-20: No new Rust dependencies

The integration uses Cargo build-script stdlib APIs and the existing compiler. No FFI,
serialization, networking, or build dependency was added.

## 2026-08-20: Quota policy stays scalar and stateless

Window status, pressure-band aggregation, and pair readiness cross the same opt-in C ABI
as the original remaining-percent calculation. Rust still owns model lookup, labels,
missing-window discovery, and all formatting. Mojo receives only bounded integer values
and explicit presence/status tags.

The runtime-proxy candidate planner was not ported in this wave: its hard-affinity,
health, backoff, inflight, and quota-source ordering are coupled policy invariants, and
per-candidate scalar FFI would be finer-grained than the current bridge justifies. This
does not block the separate governed provider-routing batch recorded below.

Route-specific quota pressure is a narrower exception: two already-normalized window
observations and one route tag form a bounded batch decision. The Mojo entry point returns
only a pressure-band tag; it does not select profiles or override hard affinity.

## 2026-08-20: Consolidate Mojo build ownership

The repeated per-crate build scripts were replaced with `prodex-mojo-core`. It owns the unsafe
FFI declarations, source selection, archive creation, strict-mode behavior, target flags, and
activation cfgs. Consumer crates expose safe typed wrappers. This keeps the ABI small and makes
the release feature set (`mojo-core`) explicit without making Mojo a default dependency.

## 2026-08-20: Governed provider routing crosses one complete batch

The governed provider plan crosses one flat-buffer batch of at most 64 records. Rust validates
descriptors, evaluates static and policy hard filters, normalizes bounded signals, maps provider
identity to a stable order value, and reconstructs routes and candidate evaluations. Mojo applies
the required capability mask, performs score arithmetic, and orders eligible indices by affinity,
score, provider order, and original index. Rust retains credentials, route construction, affinity
error handling, and user-facing errors.

Provider route capability negotiation uses a separate flat-buffer batch. Rust maps the seven
`ModelCapability` values to a mask and keeps model/route objects and redacted error plans; Mojo
returns compatible flags, reason tags, and first compatible/incompatible indices. Malformed route
tokens remain excluded from first-index selection.

Runtime profile scheduling and Smart Context byte estimation are bounded Mojo kernels.
Tokenizer integration and dormant Smart Context candidate selection remain Rust.

## 2026-08-20: Smart Context pressure is a production integer kernel

Rust continues to estimate tokens, resolve model context limits, and collect accounting-risk
signals. After that normalization, `smart_context_pressure_snapshot` sends only fixed-width
values and tags to Mojo. Mojo owns effective-capacity subtraction, saturating pressure arithmetic,
pressure-band classification, safety-floor clamping, and estimator-confidence classification.
The result is authoritative in `smart_context_observed_token_accounting_with_calibration`; the
captured Rust calculation remains a test oracle. Float-heavy relevance scoring is not part of
this boundary.

## 2026-08-20: Runtime candidate ordering crosses one bounded batch

`build_runtime_response_candidate_execution_plan` keeps excluded-profile filtering, quota guard
construction, health/backoff acquisition, prompt-cache ownership, and all route/affinity state in
Rust. It sends one 22-field row per remaining candidate to Mojo, which returns stable ready and
fallback index orders. Mojo's result is authoritative on the feature-enabled path; malformed
results fail closed. The fallback list intentionally contains every
candidate and may overlap ready, matching the existing retry plan contract. A bounded selection
sort is used at 256 candidates to keep the ABI allocation-free; the stream-commit and affinity
invariants are unchanged.

## 2026-08-20: Dormant Smart Context candidate planning stays Rust

The candidate score/selection helpers under `prodex-runtime-proxy::smart_context::candidates`
have no non-test production caller in the current tree. They remain audit-only Rust code rather
than gaining an unused Mojo wrapper. Revisit only when a real production call graph exists.

## 2026-08-20: Keep accounting and distributed rate limiting in Rust

The generic domain `commit_reservation` and `evaluate_rate_limit` helpers were audited but not
promoted: the durable accounting flow uses `reconcile_reserved_usage`, while distributed rate
limiting is owned by Redis/runtime adapters. Adding an unused scalar wrapper would violate the
zero-unused-Mojo rule. Accounting-budget enforcement, SLO classification, and float-heavy
context ranking remain Rust until a real production seam and exact parity contract exist.

## 2026-08-20: Linux Mojo release is fail-closed (superseded for 0.418.0)

The release matrix enables compiled-in Mojo for `x86_64-unknown-linux-gnu` and, after the
2026-08-23 promotion, `aarch64-unknown-linux-gnu`. Mojo is compiled into a target archive in an
isolated Cargo target directory, then the final Rust binary is linked through the existing cross
container with the archive path and strict Mojo variables explicitly forwarded. This prevents
host build-script binaries or host GLIBC from defining the deployment baseline. The ARM64 row
also runs the final artifact and self-test through QEMU. At that time, other platforms remained
Rust-only until final-link, runtime, signing, and clean-machine evidence existed. A compiled-in binary must report
`compiler_required=false` at runtime; the release build must report `build_strict=true` under
`PRODEX_MOJO_REQUIRED=1`.

The old platform split is superseded by the `0.418.0` target matrix: all published rows now
receive a cross-compiled Mojo archive and must pass native link, doctor, and self-test checks.

## 2026-08-20: Release metadata drives installation (platform wording superseded for 0.418.0)

Release CI renders `release-manifest.tsv` and JSON from one target matrix and covers the metadata
with `SHA256SUMS`. `install.sh` and `install.ps1` select by target and verify the staged binary's
own `doctor --runtime --json` implementation and Mojo self-test. They never inspect or install a
user Mojo compiler. WSL naturally uses the Linux shell installer; native Windows was a
Rust-compatible artifact while its target was not release-approved for Mojo. The `0.418.0`
release matrix now requires Mojo-backed artifacts for every published target.

## 2026-08-21: Promote ordered optimistic candidate selection

`runtime_optimistic_current_candidate_decision` is an active runtime selection caller, so its
ordered predicate engine now crosses one normalized scalar ABI. Rust trims and compares profile
and prompt-cache strings, maps route/source/quota tags, retains the Rust predicate oracle, and
reconstructs the public skip reason. Mojo returns the first matching reason; 5,000 generated
cases cover simultaneous rejection conditions and precedence. Affinity state, health acquisition,
continuation ownership, and transport remain Rust.

## 2026-08-21: Provider constraints use one normalized decision kernel

Serde parsing, provider adapter capability lookup, catalog resolution, reasoning-map lookup, and
human-readable errors remain Rust. The active public constraint evaluator normalizes those values
into one fixed-width input. Mojo owns endpoint/catalog branches after normalization, missing
feature classification, reasoning support, output-limit policy, saturating requirement totals,
context policy, adjustments, and warning tags. Rust reconstructs the existing public evaluation;
the Rust algorithm remains only as a test oracle and Rust-only build path. The strict provider-core
suite includes 2,000 generated normalized cases.

## 2026-08-21: Move active Smart Context rehydration admission

The active body-transform path now sorts artifact identities and performs store lookup/token
estimation in Rust, then sends only ranked numeric rows and availability flags to one Mojo batch.
Mojo owns required/minimal-tier admission, missing-artifact classification, saturating budget
admission, and used-token accumulation. Rust maps action tags back to artifact IDs and executes
rehydration. The dormant relevance scorer remains Rust because it has no production caller and
uses unprobed float ordering.

## 2026-08-21: Normalize quota aggregation before Mojo

The active quota pool renderer now collects normalized Gemini/Copilot main-quota rows in Rust and
uses one bounded Mojo batch for profile count, saturating remaining sum, and earliest reset.
Provider JSON, decimal/floating conversion, clock/reset acquisition, report sorting, labels, and
terminal rendering remain Rust. The batch has a 1,024-row ceiling; invalid Mojo output fails
closed instead of recomputing the aggregate in Rust.

## 2026-08-21: Move runtime tuning default tuple

Runtime configuration, probe refresh, WebSocket worker setup, async workers, and log queue setup
share one normalized parallelism batch. Mojo computes only bounded integer defaults; environment
parsing, overrides, clamping of user values, queue construction, and diagnostics remain Rust.
Rust-only builds retain their separate default helpers; the public tuning crate has a 2,000-case
differential suite.

## 2026-08-21: Expand compiled-in ownership diagnostics

`prodex doctor --runtime --json` now reports individual real-Mojo module self-tests for quota
aggregation, provider constraints, Smart Context rehydration, runtime tuning, profile scheduling,
runtime-policy numeric validation, critical-signal arithmetic, and the existing runtime/routing
modules. A module is reported active only when the shared archive is active and that module
self-test passes. The shipped compiler requirement remains false.

## 2026-08-23: Version provider constraints independently

The unversioned provider constraint export exposed 39 positional ABI parameters. Mojo 1.0.0
compiled and linked a C-struct layout probe on x86_64 Linux, but the local environment could not
independently link and execute the same probe for the supported aarch64 Linux release target.
The authoritative provider kernel therefore uses ABI v2 with exact, separate `Int64` and `UInt64`
flat-buffer schemas. `prodex-mojo-core` owns all unsafe declarations and converts checked tags to
typed Rust enums; count, version, tag, presence, and output-coherence failures fail closed. The
shared routing ABI remains version 1.

## 2026-08-23: Keep npm as a small developer interface

The root package exposes 15 maintained commands instead of one alias per focused script. CI
workflows and focused documentation invoke the existing `scripts/ci`, `scripts/docs`,
`scripts/compat`, and load runners directly. The enterprise workflow guard now validates those
real commands and their negative self-tests rather than duplicating the same command map in
`package.json` and the test-impact manifest. Validation implementations and workflow coverage are
unchanged; only redundant aliases and alias-consistency machinery were removed.

## 2026-08-24: Promote borrowed UTF-8 context grouping

The first production rich-data seam is critical-signal duplicate grouping in `prodex-context`.
Rust still strips ANSI escapes, normalizes command output, preserves Rust's Unicode trim
semantics, and classifies diagnostic lines. It passes non-secret trimmed line views plus seven
typed counters in one bounded call. Mojo validates UTF-8, constructs zero-copy `StringSlice`
values, groups exact duplicate lines, and writes the row plan and structured capacity/result
record into Rust-owned buffers. The Rust implementation remains a test oracle; Mojo-enabled
builds do not fall back when the rich call fails.

The text ABI is independently versioned at `1` and uses `#[repr(C)]` pointer-plus-native-length
records. Rust compile-time assertions and a Mojo reflection export verify size, alignment, and
field offsets. Empty and embedded-nul text is supported, no sentinel is read, pointers live for
one synchronous call, and Mojo retains nothing. Secrets, prompts, credentials, paths, and auth
metadata remain outside this seam.

Mojo 1.0.0 locally compiled and ran owning `String`, `List`, `Dict`, `Set`, `Optional`, and
`Variant` probes. Object inspection also proved that the heap-owning types require
`libKGENCompilerRTShared.so` and support libraries whose observed GLIBC requirements exceed the
current release ceilings. Production therefore uses `StringSlice`, `Optional[Pointer]`, and
`InlineArray`; release CI rejects `KGEN_CompilerRT_*` references. A bundled Mojo runtime or
standalone Mojo utility is not promoted until licensing, GLIBC, target packaging, signing, and
clean-machine evidence all pass.

## 2026-08-26: Rich Mojo domain ownership supersedes normalization-only FFI

The first-generation integer and `StringSlice` seams remain compatible contracts, but they no
longer define the semantic boundary. Rich ABI v6 sends bounded non-secret text and record DTOs to
Mojo. Mojo constructs `DiagnosticRecord`, `RouteCandidate`, `PolicyRule`, and `ContextItem`
values, performs parsing/normalization, uses open-addressing arena tables for grouping and set
membership, and returns caller-owned structured records. Rust maps those records into existing
public types and retains IO, authorization, credentials, persistence, transport, and presentation.

The active operations are context diagnostic analysis, provider/model fallback parsing, gateway
route-alias validation, governed provider routing, and Smart Context rehydration planning. A Mojo
error is a hard internal failure on a Mojo-enabled build; no Rust semantic recomputation is
selected. The feature-off implementation remains a Rust-only target and differential oracle.

## 2026-08-26: Arena-backed release strategy per subsystem

Native `String`, `List`, `Dict`, and `Set` compile and run in a standalone Mojo capability probe,
but the executable requires `libKGENCompilerRTShared.so`, AsyncRT/MSupport globals, a developer
RUNPATH, and GLIBC 2.34 on the audit host. That fails the existing static archive and GLIBC
deployment contract. Rich production code therefore uses borrowed `StringSlice`-compatible
views, typed Mojo structs, bounded arrays, caller-owned byte arenas, and open-addressing tables.
This is an intentional release decision, not a permanent rejection of future native runtime
packaging.

## 2026-08-27: Mojo ecosystem package gate

The live catalog and pinned source checkouts were audited before local infrastructure was added.
The exact decisions and source revisions are recorded in `migration/mojo-ecosystem-audit.json`.
EmberJson is `REJECT_COMPILER` because its current 0.3.4 checkout fails under pinned Mojo 1.0.0.
ExtraMojo is `REJECT_RUNTIME` because its selected source links the owning Mojo runtime, and
mojo-regex is `REJECT_PLATFORM` because it has no Windows release evidence. UUID is
`REJECT_COMPILER`; decimo and argmojo are `NOT_RELEVANT` to the current Prodex production seams.
No community package is imported by the release core, so no floating package lock or build-time
network dependency was added.

## 2026-08-26: JSON boundaries remain Rust-owned

The package review did not justify replacing Serde for provider protocols, Codex JSON-RPC,
runtime-policy TOML, or persisted state. Rich v3 receives already bounded non-secret text/records
after those external compatibility and security boundaries. EmberJson immutable-document and
reflection experiments are deferred until its compiler compatibility, complete-document
validation, package provenance, and cross-target artifact gates pass.

## 2026-08-28: 0.419.0 keeps new decisions bounded and fail-closed

Provider catalog identity/choice/deduplication, route-aware quota pressure scoring, and observed
Smart Context token totals now use the existing static Mojo archive through bounded caller-owned
buffers. Serde, provider adapters, clocks, plan scaling, calibration, policy, state, and errors
remain Rust-owned. Mojo-enabled errors are hard failures; the Rust implementations are separate
feature-off builds or test oracles, never production fallback.

`prodex s` keeps its existing interactive main-agent/provider selection but resolves the
provider-scoped remembered model and model-valid effort without ordinary model/effort pickers.
`prodex s expose` retains full per-instance configuration and freezes it before creating the
process-local capability. Quick Tunnel owns an isolated `cloudflared --protocol auto` child and
uses one bounded explicit HTTP/2 compatibility retry only after auto transport registration stalls;
existing Cloudflare hostname mode validates but never owns the user's tunnel.

`prodex ping openai` is a per-profile diagnostic, not a rotation request: it uses a read-only,
ephemeral, isolated Codex JSONL turn and requires a completed turn plus exact `PONG`. Global
provider-secret environment variables are removed from that child so its result remains bound to
the selected profile. Normal Codex retry remains upstream-owned, while Prodex classification
distinguishes explicit quota, rate limit, overload, transport, and permanent errors at the
pre-commit boundary.
