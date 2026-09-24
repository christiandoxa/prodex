# Mojo-first production migration

## Objective and accounting

The next development campaign targets **75% Mojo** of first-party production
Rust plus reachable Mojo, using the existing counting rules in
`scripts/ci/mojo-production-share.mjs`. This is a migration objective, not a
claim that the current tree has reached the target or a proved technical ceiling.

The starting commit is `5c13c6db856ca3dafcc8a84f6babbfd16c054c63`.
Its canonical inventory is 44,316 Mojo LOC and 194,849 Rust LOC, or
18.529467104300377% of 239,165 production LOC. The source inventory SHA-256 is
`3c1259acf29f55b93e8ca814e80776920c7866c400c5f6c920c68b682c3a1713`.

Do not change counting exclusions, inflate Mojo, restore retired features, or
remove working functionality to increase the share. Dependency implementation
code is not first-party source; Rust adapters remain in the denominator.
A smaller, historical semantic inventory is not the broad production metric.

The historical 7% release floor and its existing non-regression checks stay in
force. A successful release-floor check does **not** mean this campaign is done:
completion requires the canonical report to show `project_target_met: true` at
75%, together with real production activation, compatibility and target gates.

## Migration boundary

Mojo owns application decisions, transformations, argument and protocol plans,
normalization, classification, collection algorithms and rendering semantics.
Rust should retain the smallest necessary bridge for an unproven Mojo capability,
a mature dependency or an OS, asynchronous execution or security boundary.

A Rust-only dependency does not justify retaining the entire surrounding
application algorithm in Rust. Split acquisition and effects from the semantic
plan, use typed/versioned caller-owned ABI records, validate returned indices,
lengths and tags, and never silently recompute in Rust after a Mojo error.
Feature-off Rust builds and test oracles are separate from the Mojo production path.

Existing process, affinity, streaming, persistence and secret-safety invariants
remain compatibility requirements. Compiler/library maturity must be established
with actual link/runtime evidence, not assumed from old migration documents.

## Checkpoint discipline

Each migration checkpoint must identify its production consumer and Mojo owner,
exercise differential and caller-boundary cases, report exact validation, and
commit only reviewed files on `main`. No release or tag is requested here.
Preserve untracked work and unrelated active processes.

Builds and temporary probes have one explicit owner. Use bounded synchronous
commands, reuse the campaign target while it has active consumers, check disk
space, remove only owned artifacts after verification, and run `trash-empty -f`
regularly and at final cleanup. Do not delete dependency download caches.

## Initial implementation scope

Start with the active Codex launch argument planner in `prodex-runtime-launch`:
move argument classification, command discovery and option transformation into
Mojo while Rust retains `OsString`, socket formatting and external process calls.
Preserve non-UTF-8 arguments byte-for-byte, separator behavior, option-value
pairing, session retargeting, configuration precedence and existing full-access
semantics. Expand only after parity is demonstrated at the public launch API.

## Launch argument wave

`mojo/prodex_core/launch_args.mojo` and `launch_args_common.mojo` now own
command/model discovery, resume retargeting, profile normalization, dry-run
extraction, launch preparation and configuration scope ordering. The active
consumer is `crates/prodex-runtime-launch/src/args_mojo.rs`, enabled through
`prodex-app/mojo-core -> prodex-runtime-launch/mojo`. The original Rust source
was first moved byte-for-byte and then restricted to feature-off/test builds.

Validation before activation: 64 strict Mojo launch unit tests, including 5,000
seeded differential argument vectors; 60 feature-off tests; four raw ABI,
Unicode-whitespace and reentrancy tests; focused Clippy with warnings denied;
crate-boundary, size, authority and no-fallback guards. Non-UTF-8 OS arguments
are passed as opaque records and preserved by original index. Unicode whitespace
matches Rust `str::trim`, including the distinction from Python U+001C..U+001F.

The new kernel compiled to objects for all six release target triples using the
pinned 1.0.0 compiler. Linux x86_64 tests execute the compiled kernel; object
creation is not native macOS/Windows runtime proof. Its object also compiles with
the locally installed 1.1.0 compiler, but a whole-tree 1.1.0 build exposed an
existing `InlineArray` import incompatibility outside this migration. The pinned
compiler is isolated for validation; the global installation and release pin
are unchanged. Real-Mojo CI now explicitly runs the launch package parity suite.

These are development checkpoints, not completion of the 75% objective. Run the
canonical report for the current measured share.

## Configuration override wave

The live launch adapters now delegate option-form classification, Unicode key
trimming, exact first-key matching, duplicate override precedence, separator
handling and replacement selection to `launch_config.mojo`. Only override keys
cross this decision boundary; replacement values stay with the Rust owner.
The returned plan is checked against argument/key counts, valid tags and the
observed replacement bitmap before reconstruction.

Validation: 10,000 seeded differential vectors inside three configuration ABI
integration tests, 65 public launch tests, focused Clippy for both the caller and
ABI test, the boundary/size/authority/no-fallback guards, and new-kernel object
compilation on all six release triples. Native target runtime validation remains
separate from cross-target object compilation.

## Complete provider tool-shape wave

The existing `prodex-provider-core/mojo` feature now selects complete Mojo
ownership of function/custom/namespace/MCP/tool-search expansion, choice
precedence, namespace naming and web-search extraction/removal. Serde acquires
only the relevant request member into a borrowed, parent-checked JSON arena;
replacement output is capacity-measured in Mojo and materialized by Serde.
No external dependency, dynamic Mojo runtime or new MCP surface was added.

The provider suite passes 231 tests with one explicitly manual benchmark ignored
by the normal correctness run. New coverage includes 5,000 seeded JSON trees,
Unicode and control characters, first-key precedence, MCP sort/dedup, 1,024
custom tools and 2,048 MCP names. Three additional core ABI/graph/reentrancy
tests pass, as do focused Clippy and object builds for all six release targets.

The optimized local complete-boundary benchmark was also run explicitly. Median
nanoseconds for Rust versus the new Mojo boundary were 9/204 at zero tools,
16,278/39,372 at eight tools and 155,977/333,660 at 64 tools. This is an ownership
migration, **not a speedup**: the 64-tool fixture adds about 178 microseconds.
The measured cost includes Serde acquisition/materialization, allocation and FFI;
future batching and direct structured output should target that overhead.
See `migration/benchmarks.md` for the reproducible command and scope.

## DeepSeek message wave and shared arena refinement

The same checked JSON ABI and one shared Serde acquisition module now support
complete thinking-message normalization, assistant tool-call content shaping,
tool-call/output adjacency repair and two-level response-metadata merging.
Mojo owns first-output lookup, global single emission, unanswered-call removal,
content-presence rules and the historical no-valid-output special case. The
lookup uses a stable index sort and binary searches over caller-owned scratch;
Rust retains only value acquisition/materialization and feature-off/test oracles.

The new differential corpus contains 10,000 generated histories and metadata
merges, plus explicit first-output/global-emission, Unicode, scalar/object merge
and 2,048-call out-of-order cases. The provider suite passes 235 tests with two
manual benchmark tests excluded from ordinary correctness runs; both benchmarks
were separately run successfully. The new message ABI adds two negative/empty
shape tests. Shared JSON capacity coverage now sweeps every output capacity for
an escaped Unicode fixture with prefix/suffix canaries.

The shared writer was improved to measure/copy complete validated spans and
unescaped runs rather than repeat a capacity check for every byte. Latest
serialized benchmark runs measured 299,685 ns for 64 function tools against
157,943 ns for the Rust oracle. DeepSeek adjacency at 64 call/output pairs with
4,096-byte contents measured 1,801,497 ns against 183,095 ns. Those are real
complete-boundary costs, not speedup claims; see the benchmark record for scope.
The writer and both JSON kernels compile to objects on all six release triples.

## Broader integration findings

The complete Mojo app library was exercised as two disjoint partitions. The
1,540 non-main-internal tests passed. The initial 408-test main-internal run
found two previously unexercised Mojo contract mismatches: raw quota snapshot
observations above 100 were rejected before inflight admission, and long unknown
doctor log tokens were treated as invalid instead of unrecognized markers.

Quota snapshot planning now preserves signed remaining observations exactly as
the Rust snapshot path does; status, route and grace validation remain intact.
Doctor still validates complete UTF-8 input, but limits catalog matching rather
than rejecting a valid long token. New tests cover extreme signed observations,
reset/hold semantics, long Unicode text and malformed UTF-8. Both original app
regressions pass without changing their fixtures or selecting a Rust fallback.
The complete 40-root Mojo archive was freshly compiled from current sources to
exclude stale incremental-archive artifacts from this validation.

Provider replay initially rejected Kiro secret fixtures because the campaign's
own TMPDIR had mode 775. Making that one owned directory private (700) allowed
the unchanged 499-test replay suite to pass. No secret-store policy was relaxed.


## Complete Responses-to-chat request planning

The shared OpenAI Responses-to-Chat request bridge now has one production-authoritative
Mojo transform over the caller-owned parsed JSON arena. Mojo owns rejection precedence,
text/history message planning, function-call and function-output mapping, model precedence,
forwarded request controls, and the final chat request object. Rust retains Serde parsing,
the ProviderTransformResult host contract, and a test/Rust-only oracle.

Validation includes explicit empty/wrong-type precedence fixtures plus 5,000 deterministic
generated requests. The Mojo boundary also has raw ABI, malformed-tree, and reentrancy tests.
The kernel compiles with pinned Mojo 1.0.0 for all six release target triples. No production
Rust recomputation is selected after a Mojo error.

## Complete Anthropic Messages request wave

The Anthropic chat-to-Messages request bridge now has one authoritative Mojo
transform over the shared caller-owned JSON arena. Mojo validates accepted chat
fields, message/tool-call structure, web-search options and tool-choice shape,
merges adjacent roles, converts system/developer messages, normalizes tool names,
constructs tool-use/tool-result blocks, applies request defaults, and records the
existing web-search context-size degradation contract. Rust retains Serde JSON
acquisition/materialization, provider result DTOs, time, transport, and the
feature-off/test oracle only.

The production path no longer composes the former per-fragment Mojo request
builder or Rust-side tool-shape decisions. Focused evidence includes four raw
ABI tests, explicit edge fixtures, 5,000 generated differential chat requests,
18 existing Anthropic Mojo tests, 13 feature-off Anthropic tests, the complete
provider Mojo suite (238 passed, two manual benchmarks ignored), focused
all-target Clippy, and object compilation for all six release triples with the
pinned Mojo 1.0.0 compiler. No speedup is claimed.

## Stable compiler baseline after 0.431.1

The post-0.431.1 Mojo campaign uses Rust 1.98.1 and Mojo 1.1.0, the latest
stable releases verified on 2026-09-23. CI, standalone release targets,
supply-chain policy, install tests, and documentation use those exact pins.

Mojo 1.1.0 removes the old implicit `InlineArray` surface. Prodex migrated
fixed-size semantic buffers to `std.collections.Array` and the two-value trim
bounds helper to a fixed `Tuple`; this is a representation-only compiler
compatibility change and does not change the caller-owned C ABI.

Upgrade evidence before promotion included the complete `prodex-mojo-core`
suite, runtime-launch Mojo tests, provider-core Mojo tests, all-feature Clippy
for the core runtime/provider/app consumers, a compiled-in root binary, and all
Mojo authority/no-fallback/source-size/supply-chain guards.
## Model-aware quota capacity wave

OpenAI model classification and model-aware quota routing now use a production-authoritative
Mojo policy in `quota.mojo`. The kernel owns normalized Luna/Spark identity, Luna-reserve
identifier matching, regular-versus-reserve selection, unknown-Luna-capacity classification,
ready-limit decisions, and the code-review gate. Rust retains provider JSON acquisition,
borrowed `WindowPair` reconstruction, account-identity comparison, and the feature-off oracle.

The boundary is exercised by the existing quota integration suite plus new direct parity
coverage: normalized identifier fixtures, Luna-reserve identity cases, and all 2,048
combinations of four model classes across the nine boolean capacity inputs. Both the real-Mojo
68-test quota suite and the 59-test feature-off suite pass, as does focused all-target Clippy
with warnings denied.

At this checkpoint the canonical broad inventory reports 47,626 reachable Mojo LOC and
196,196 Rust production LOC, or 19.533102% Mojo. The previous pushed checkpoint after CI
cleanup measured 19.480578%; this is incremental progress toward the 75% objective, not a
target-completion claim.

## CI consolidation during the campaign

The main Rust quality lane previously compiled Clippy twice: once as a production JSON report
for Sonar and again as an all-target warning gate. The lane now performs one all-target Clippy
pass with JSON output and `-D warnings`, preserving the Sonar artifact and warning enforcement
while removing the duplicate compilation. Intentional conditional jobs such as scheduled
optional-tool freshness and path-gated benchmark smoke remain conditional rather than being
deleted merely because they appear as skipped on ordinary pushes.
## Quota status classification wave

Quota error and blocked-limit status classification now run in `quota.mojo`. Mojo owns
the historical classification precedence for unavailable/configuration, transport, auth,
rate-limit and response errors, plus the 5h/weekly/generic exhausted ordering used by the
compact quota status. Rust keeps first-line extraction, provider-owned text and final
user-facing string allocation.

The real-Mojo quota suite covers the public rendering behavior and direct classifier fixtures,
including precedence collisions such as server-plus-timeout and TLS-plus-proxy. Feature-off
Rust behavior remains the compatibility oracle. The canonical broad inventory at this
checkpoint is 47,778 Mojo LOC and 196,186 Rust production LOC, or 19.584037% Mojo.
## Runtime health adapter consolidation

Runtime profile coupling and performance scoring now reuse one normalized
`ProfileHealthScoreInput` projection instead of rebuilding route/coupled-route state in four
separate feature-on Rust paths. The actual decay, coupling, performance, and aggregate sort
arithmetic remains Mojo-authoritative; Rust is reduced to one state-observation adapter for
both map-backed and callback-backed callers.

Focused Mojo parity tests and all-target runtime-proxy Clippy pass. This removes 51 net Rust
production lines without adding a duplicate policy implementation. The canonical broad
inventory is now 47,778 Mojo LOC and 196,131 Rust production LOC, or 19.588453% Mojo.

## Operational log event classification wave

Operational runtime-log source and interest classification now has one production-authoritative
Mojo plan in `observability_labels.mojo`. The kernel owns the exact event catalog, dynamic
compact/MCP/sub-agent/local-rewrite classifications, compatibility-surface precedence, and the
`smart_context_prepare_fallback` interest decision. Rust retains parsed field acquisition,
redaction, human rendering, load coalescing, and a feature-off/test oracle.

Focused parity exercises representative exact and dynamic event families plus the compatibility
and Smart Context edge cases in both `mojo-core` and feature-off builds. The production Rust
oracle is isolated behind the feature-off module instead of remaining in the broad production
count. Focused all-target `prodex-app` Clippy with warnings denied, real-Mojo linkage, the
production-share check, ownership check, and authority guard pass.

The canonical broad inventory at this checkpoint is 48,175 reachable Mojo LOC and 196,124 Rust
production LOC, or 19.719688% Mojo. The 75% project target remains a forward migration goal.

## Operational log detail planning wave

The operational transcript detail policy now delegates source-specific field ordering and the
first-local-chunk latency/TTFT distinction to the same reachable observability Mojo kernel. Rust
retains field acquisition, safety redaction, endpoint sanitization, bounded rendering, and the
feature-off/test oracle; the production path consumes only validated detail-plan indices.

Focused feature-on and feature-off differential tests compare every rendered source family against
the Rust oracle with a complete synthetic field set. A forced fresh Mojo-core rebuild verified the
new export is present in the linked archive. Focused all-target Clippy for `prodex-app` and
`prodex-mojo-core`, production-share, ownership, authority, and no-fallback checks pass.

The canonical broad inventory at this checkpoint is 48,471 reachable Mojo LOC and 196,151 Rust
production LOC, or 19.814653% Mojo. Adapter cost remains in the Rust denominator; the 75% target is
not yet met.

## Retry-after numeric policy wave

Runtime retry-after numeric policy now executes in the reachable rich Mojo kernel under the
production `mojo` feature. Rust retains HTTP/header acquisition, UTF-8 validation, phrase/suffix
boundary extraction, and a feature-off/test oracle; Mojo owns bounded integer parsing, fractional
millisecond rounding, overflow rejection, zero rejection, and the 300-second cap.

The differential corpus covers u64/u128 boundaries, overflow, leading zeroes, fractional
millisecond rounding, second rounding, zero values, malformed decimal forms, and cap behavior.
Focused runtime-proxy tests pass with and without Mojo, and all-target Clippy for both
`prodex-runtime-proxy` and `prodex-mojo-core` passes with the local Mojo 1.1.0 compiler.

The canonical broad inventory at this checkpoint is 48,602 reachable Mojo LOC and 196,561 Rust
production LOC, or 19.824362% Mojo. The 75% target remains a forward migration goal.

## Request compatibility surface wave

Request compatibility-surface classification now has a production Mojo plan. Rust keeps HTTP
header/path acquisition, bounded JSON acquisition, owned string rendering, and a feature-off/test
oracle; Mojo owns Codex-vs-compatible client classification, streaming semantics, continuation
flags, tool-capability classification, request origin, approval detection, and compatibility
warning decisions.

Focused feature-on differential tests compare HTTP and WebSocket fixtures against the Rust oracle,
including Codex headers, sub-agent detection, chat completions, unknown routes, tools, streaming,
and previous-response warning behavior. Feature-off compatibility tests and all-target Clippy for
`prodex-runtime-proxy` and the `mojo-runtime` core pass with Mojo 1.1.0.

The canonical broad inventory at this checkpoint is 48,790 reachable Mojo LOC and 196,782 Rust
production LOC, or 19.867900% Mojo. The 75% target remains a forward migration goal.

## Previous-response error classification wave

Previous-response failure classification now runs through the existing rich Mojo error-policy
kernel. Rust retains JSON traversal, UTF-8 acquisition, message ownership, and feature-off/test
oracles; Mojo owns exact `previous_response_not_found`, invalid `previous_response_id`, and missing
tool/function-call classification for both structured error fields and bounded text payloads.

Focused feature-on differential tests cover exact and near-match structured/text cases, while the
existing payload-detection and failure-response suites pass with and without Mojo. All-target
Clippy for `prodex-runtime-proxy` and `prodex-mojo-core` passes with Mojo 1.1.0. The same checkpoint
also consolidates repeated previous-response planning inputs through one semantic default adapter.

The canonical broad inventory at this checkpoint is 48,959 reachable Mojo LOC and 196,854 Rust
production LOC, or 19.917173% Mojo. The 75% target remains a forward migration goal.

## Runtime health duplicate Rust deletion wave

The runtime profile health map-backed scoring paths now delegate directly to the already
Mojo-authoritative by-key scoring path. The duplicate Rust implementations for map-backed
coupling, performance, aggregate sort-key scoring, and their map-only effective-score helpers
were deleted rather than retained as a second production implementation.

Feature-on tests now verify the map adapter produces the same result as the canonical by-key Mojo
path, while the remaining feature-off compatibility implementation is limited to the explicit
non-Mojo build path. Focused Mojo/default tests, all-target runtime-proxy Clippy, size guard,
production-share, authority, and no-fallback guards pass with Mojo 1.1.0.

The canonical broad inventory at this checkpoint is 48,959 reachable Mojo LOC and 196,734 Rust
production LOC, or 19.926901% Mojo. The 75% project target remains a forward migration goal.

## Smart Context duplicate Rust cleanup wave

Smart Context token accounting now keeps the feature-on production path free of helper policy that
Mojo already owns. Rust-only pressure-band, estimator-confidence, and safety-floor helpers are gated
to the explicit feature-off/test compatibility path; duplicate feature-gated memory-capsule wrappers
were collapsed; and the shared result-assembly adapter was separated from the Rust oracle module.

Focused feature-on and feature-off token-accounting tests pass, all-target runtime-proxy Clippy is
clean, and production-share, authority, no-fallback, and size guards pass with Mojo 1.1.0. No
second Rust production implementation was retained for the migrated policy decisions.

The canonical broad inventory at this checkpoint is 48,959 reachable Mojo LOC and 196,711 Rust
production LOC, or 19.928766% Mojo. The 75% project target remains a forward migration goal.

## Runtime log parser Mojo wave

Structured runtime-log tokenization now runs in a dedicated Mojo parser. Mojo owns event-token
selection, key/value boundary detection, quoted-value scanning, escape handling, whitespace
skipping, and field-span planning. Rust retains owned-string construction, JSON unescaping,
redaction, and rendering. The old Rust tokenizer is isolated to the explicit non-Mojo compatibility
build and is not used by the Mojo production path.

A forced fresh Mojo archive build verified the new parser export. Feature-on expectation tests cover
quoted/escaped values, Unicode, empty values, malformed field values, and event-only parsing; the
feature-off structured-log tests remain green. All-target Clippy, production-share, ownership,
authority, no-fallback, and size guards pass with Mojo 1.1.0.

The canonical broad inventory at this checkpoint is 49,076 reachable Mojo LOC and 196,723 Rust
production LOC, or 19.965907% Mojo. The 75% project target remains a forward migration goal.

## Compatibility adapter minimization wave

The request compatibility-surface adapter was tightened after the Mojo ownership migration. The
Rust production path no longer duplicates per-tag constants and one-off label mappers; it now uses
compact table-driven tag/flag adapters around the Mojo plan. Client-family, stream, continuation,
tool-capability, approval, origin, and warning decisions remain Mojo-owned.

Feature-on differential tests against the feature-off compatibility implementation and default
compatibility tests pass. All-target runtime-proxy Clippy plus production-share, ownership,
authority, no-fallback, and size guards are green with Mojo 1.1.0.

The canonical broad inventory at this checkpoint is 49,076 reachable Mojo LOC and 196,698 Rust
production LOC, or 19.967938% Mojo. The 75% project target remains a forward migration goal.

## Runtime Mojo adapter deduplication wave

The runtime-proxy Mojo adapter now reuses one canonical route/status/band/source conversion layer
across quota snapshots, pressure scoring, candidate planning, and window classification. Repeated
Rust enum/tag matches that merely re-encoded already-Mojo-owned decisions were deleted instead of
being kept beside the migrated kernels.

Focused Mojo quota/candidate tests pass, all-target runtime-proxy Clippy is clean, and
production-share, ownership, authority, no-fallback, and size guards pass with Mojo 1.1.0.

The canonical broad inventory at this checkpoint is 49,076 reachable Mojo LOC and 196,659 Rust
production LOC, or 19.971107% Mojo. The 75% project target remains a forward migration goal.

## Response-forwarding classification wave

Response-forwarding classification now runs in a dedicated Mojo kernel. Mojo owns hop-by-hop
response-header classification, SSE content-type recognition, terminal/completed token-usage event
classification, and model-generation-start event classification. The former Rust decision logic is
removed from the Mojo production path; Rust retains transport/header acquisition, typed result
construction, stateful timing, and the explicit feature-off compatibility implementation.

A fresh Mojo 1.1.0 archive build plus feature-on/feature-off response-forwarding tests pass, including
header casing/whitespace, SSE content types, terminal event families, generation deltas, live usage,
and SSE tap-state behavior. All-target Clippy and production-share, ownership, authority,
no-fallback, and size guards pass.

The canonical broad inventory at this checkpoint is 49,241 reachable Mojo LOC and 196,662 Rust
production LOC, or 20.024563% Mojo. The 75% project target remains a forward migration goal.
