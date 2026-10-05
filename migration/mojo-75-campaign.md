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
Completed migrations delete the replaced Rust semantics, including feature-off
copies and test oracles. Older retained copies in the inventory are cleanup work,
not acceptable end states. When Mojo is unavailable, exclude the capability
explicitly rather than reimplementing its decisions in Rust.

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
was first isolated for parity, then deleted along with its test oracle and
resume retargeting copy. Runtime launch now enables Mojo by default and rejects
an explicit feature-off build at compile time.

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
Rust retains value acquisition/materialization. The former feature-off and test
oracles were removed in the later hard-replacement checkpoint below.

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
the ProviderTransformResult host contract. Any retained Rust parity oracle in
this older wave remains cleanup debt under the hard-replacement rule.

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
acquisition/materialization, provider result DTOs, time, transport, and typed
result mapping. The feature-off request implementation
and Rust test oracle have since been deleted; unavailable Mojo returns an
explicit unsupported result.

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
borrowed `WindowPair` reconstruction and account-identity comparison. A later
hard-replacement checkpoint removed the feature-off oracle.

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

Feature-on tests verified the map adapter against the canonical by-key Mojo
path. A later checkpoint deleted the remaining feature-off compatibility
implementation. Focused Mojo/default tests, all-target runtime-proxy Clippy, size guard,
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
redaction, and rendering. The old Rust tokenizer was later deleted after a
form-feed parity fix and a second differential check.

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

## Previous-response classifier Rust cleanup wave

The previous-response error classifier was tightened after its Mojo migration. Structured and text
classification now share one production adapter, and the leftover Rust tool-context string matcher
was removed from the Mojo production path and kept only inside the explicit feature-off/test
compatibility oracle. No second production classifier remains beside the Mojo owner.

Focused feature-on parity, feature-off invalid-previous-response tests, all-target runtime-proxy
Clippy, and production-share, ownership, authority, no-fallback, and size guards pass with Mojo
1.1.0.

The canonical broad inventory at this checkpoint is 49,241 reachable Mojo LOC and 196,646 Rust
production LOC, or 20.025866% Mojo. The 75% project target remains a forward migration goal.

## SSE line planning Mojo wave

Runtime SSE line classification now executes in a dedicated Mojo kernel. Mojo owns CR/LF trimming,
blank/comment detection, `data` field recognition, separator scanning, optional leading-space
handling, and the byte-span plan consumed by the Rust stream accumulator. The former production
Rust splitter/trimmer logic was removed; only the explicit non-Mojo compatibility implementation
remains isolated outside the Mojo production path.

Feature-on expectation tests cover LF/CRLF, comments, empty `data`, spaced values, ignored fields,
and invalid UTF-8 payload bytes. The full payload-detection suite passes with and without Mojo,
all-target runtime-proxy Clippy is clean, and production-share, ownership, authority, no-fallback,
and size guards pass with Mojo 1.1.0.

The canonical broad inventory at this checkpoint is 49,294 reachable Mojo LOC and 196,666 Rust
production LOC, or 20.041470% Mojo. The 75% project target remains a forward migration goal.

## SSE inspection transition Mojo wave

The precommit SSE inspection transition now runs in Mojo. Mojo owns terminal-signal precedence
(quota, rate limit, overload, previous-response failure) and the transition from hold to commit;
Rust retains stream accumulation, parsed event storage, retry-after transport data, and effect
materialization. The Mojo production path no longer carries the prior inline Rust decision tree.

Focused payload-detection tests and the Mojo transition corpus pass, all-target runtime-proxy
Clippy is clean, and production-share, authority, no-fallback, and size guards pass with Mojo 1.1.0.

The canonical broad inventory at this checkpoint is 49,342 reachable Mojo LOC and 196,736 Rust
production LOC, or 20.051366% Mojo. The 75% project target remains a forward migration goal.

## Rate-limit header Mojo wave

Official Codex `x-codex-rate-limit-reached-type` classification now executes in the rich Mojo
kernel. Rust retains header acquisition, UTF-8 validation, and policy-object materialization; the
case-insensitive rate-limit/quota discriminator and workspace-limit catalog are Mojo-owned in the
production path.

Focused feature-on and feature-off header-policy tests pass, all-target runtime-proxy Clippy is
clean, and production-share, ownership, authority, no-fallback, and size guards pass with Mojo
1.1.0.

The canonical broad inventory at this checkpoint is 49,391 reachable Mojo LOC and 196,752 Rust
production LOC, or 20.065978% Mojo. The 75% project target remains a forward migration goal.

## Gemini stream event transform Mojo wave

Gemini GenerateContent SSE event conversion now executes as one raw-input Mojo operation. Mojo
owns candidate/part discovery, function-call argument shaping, reasoning-vs-text delta selection,
and the normalized Responses event/value packet. The Rust Mojo production path now only handles
SSE framing and transform-result materialization; the prior inline production decision tree was
removed from that path.

Focused Gemini stream tests pass with Mojo 1.1.0, the complete Gemini unit subset passes with and
without the feature, and all-target provider-core Clippy plus production-share, ownership,
authority, no-fallback, and size guards are green locally.

The canonical broad inventory at this checkpoint is 49,480 reachable Mojo LOC and 196,722 Rust
production LOC, or 20.097318% Mojo. The 75% project target remains a forward migration goal.

## Provider error classification Mojo wave

Provider error classification now executes in Mojo for the production feature path. Mojo owns
status/code/text normalization and the auth, quota, rate-limit, not-found, transient, and fallback
classification precedence together with cooldown selection. Rust retains provider body/token
acquisition, classification materialization, and the explicit feature-off compatibility branch.

Focused Mojo classifier cases cover authentication, spend/quota limits, rate limits, unsupported
models, transient overloads, and generic 429 pass-through. Feature-on and feature-off provider
error tests pass, all-target provider-core Clippy is clean, and ownership, authority, no-fallback,
and size guards pass with Mojo 1.1.0.

The canonical broad inventory at this checkpoint is 49,645 reachable Mojo LOC and 196,796 Rust
production LOC, or 20.144781% Mojo. The 75% project target remains a forward migration goal.

## Continuation status policy Mojo wave

Runtime continuation lifecycle policy now executes through
`mojo/prodex_core/continuation_status.mojo`. Mojo owns monotonic event timestamps,
verified/suspect/dead transitions, confidence and streak saturation, stale/terminal
classification, merge replacement precedence, and retention/evidence ordering. The
`prodex-runtime-store` Rust side retains `BTreeMap` ownership, string materialization,
typed status reconstruction, and compaction orchestration.

The former Rust decision implementation was deleted from the production source rather
than retained as a fallback. `prodex-runtime-store` now links the Mojo runtime kernel
as a normal dependency; its legacy `mojo` feature remains only as a compatibility
feature name and does not select a Rust implementation.

Focused validation passed with Mojo 1.1.0: the 27-test runtime-store suite, all-target
runtime-store Clippy with warnings denied, the `prodex-mojo-core` runtime test set,
production-share, authority, no-fallback, size, Ratatui interaction, and diff checks.
At this checkpoint the canonical broad inventory is 50,150 reachable Mojo LOC and
196,787 Rust production LOC, or 20.308824% Mojo.

## Continuation binding compaction Mojo wave

Continuation binding retention and retention-key ordering now execute in the existing
`continuation_status.mojo` kernel. Rust keeps map traversal, bounded-key validation,
profile-conflict string detection, and removal/materialization; Mojo owns the decision
about whether a binding survives and the evidence tuple used to choose cold entries.

The redundant Rust evidence-sort wrapper was deleted after all remaining consumers
moved to the higher-level Mojo operations. No Rust fallback implementation was added.
The 27-test runtime-store suite and all-target Clippy pass with Mojo 1.1.0, together
with production-share, authority, no-fallback, size, and diff guards.

The canonical broad inventory is 50,203 reachable Mojo LOC and 196,769 Rust
production LOC, or 20.327406% Mojo.

## Runtime backoff duplicate Rust deletion wave

Profile backoff sort-key ordering, startup softening, and circuit timing now have one
production authority in the existing `runtime_health.mojo` kernel. Both
`prodex-runtime-proxy` and `prodex-runtime-store` delegate to the same Mojo primitives;
the former feature-off/test Rust oracle implementations and duplicate store arithmetic
were deleted rather than retained as fallbacks.

`prodex-runtime-proxy` now links `prodex_mojo_core/mojo-runtime` as a normal dependency
for these health/backoff semantics; its broader `mojo` feature still controls the
additional quota/rich surfaces. The no-fallback guard now covers both promoted backoff
files.

Focused validation passed with Mojo 1.1.0: 27 runtime-store tests, 316 runtime-proxy
library tests, and all-target Clippy with warnings denied for both crates. The canonical
broad inventory is 50,203 reachable Mojo LOC and 196,676 Rust production LOC, or
20.335063% Mojo.

## Runtime health feature-off replacement wave

The runtime-proxy health scorer, latency policy, inflight limits, and bump/recovery
decisions now use their existing `runtime_health.mojo` owners unconditionally. The
feature-off Rust implementations were removed. The exact pre-migration Rust
formulas served as temporary differential oracles and were deleted in a later
checkpoint.

`prodex_mojo_core/mojo-runtime` is already a mandatory runtime-proxy dependency after
the preceding backoff wave, so these paths no longer need a feature-gated fallback.
The runtime health policy ABI is now version 2 and uses unsigned 64-bit records. The
full-width differential corpus found that version 1 rejected `u64::MAX` elapsed time
and `usize::MAX` inflight limits even though the old Rust policy returned valid scores
and limits; version 2 preserves their exact outputs. The no-fallback guard covers all four files.

The pre-wave `HEAD` oracle suite passed 345 tests with `PRODEX_MOJO_REQUIRED=1` and
`--features mojo`. After migration, the default runtime-proxy suite passed 325 tests
and the feature-on suite passed 349 tests; the differential cases include 10,000 full-
width health-score, health-transition, and latency inputs plus all route/stage threshold
edges and `usize`/`u64` maximum boundaries. `cargo fmt --all -- --check` passed.

The canonical inventory moved from 50,203 Mojo LOC and 196,676 Rust LOC
(20.335062925562724%) to 50,205 Mojo LOC and 196,642 Rust LOC (20.33850927902709%),
for a net reduction of 34 Rust production LOC.

## Runtime quota scoring and profile ordering Mojo wave

Mojo now owns runtime quota scoring and pressure bands, ready-profile scheduling,
provider-priority ordering, and current-relative profile rotation. Production paths
call the Mojo kernels unconditionally. Rust retains quota-window observation,
profile-state reads, typed ABI mapping, and validated result reconstruction; the
pre-migration scoring and ordering implementations remain only as test oracles.

Provider-priority ties now use stable insertion ordering in Mojo, matching Rust's
stable sort behavior. The existing 256-profile bound keeps this sort bounded. The
runtime-quota crate links `mojo-runtime` by default. CI builds strict Mojo 1.1.0
archives for native targets, the scheduled full suite links its Linux archive, and
fresh benchmark calibration installs the pinned compiler and rejects fallback.

With `PRODEX_MOJO_REQUIRED=1` and Mojo 1.1.0, the default and `--features mojo`
runtime-quota suites each pass 30 tests; the profile-schedule ABI suite passes 3
tests, and the app profile-ranking test passes. Workspace Clippy passes with all
targets and features and warnings denied. `npm run test:changed`, docs lint,
workflow YAML parsing, full-Rust workflow tests, ownership, authority, no-fallback,
size, churn, and production-share guards pass.

The canonical broad inventory is 50,207 reachable Mojo LOC and 196,702 Rust
production LOC, or 20.334212200% Mojo. The 75% project target remains unmet.

## DeepSeek Responses request parameter Mojo wave

DeepSeek Responses translation now validates primitive generation fields,
`top_logprobs`, and stop sequences through one `ResponsesRequestParams` plan in
the existing `deepseek_request_policy_v1` kernel. The plan composes the existing
Mojo validators and preserves Rust's validation order. The `UserId` kernel now
owns the allowed ASCII identifier characters and 512-byte limit; Rust retains
Serde acquisition, Unicode trimming, typed result handling, and error text.

The active consumer is
`crates/prodex-provider-core/src/translators/deepseek/request.rs`, reachable
through `prodex-app/mojo-core -> prodex-provider-core/mojo`. Feature-off Rust
validation remains for Rust-only builds; feature-on parity tests use it only as
a test oracle. Mojo failures become request errors and never trigger Rust
recomputation. The `UserId` output buffer stays fixed at 514 bytes, even when
the accepted kernel input reaches its 4 MiB bound.

With Mojo 1.1.0 and `PRODEX_MOJO_REQUIRED=1`, the provider-core feature-on suite
passes 245 tests with 2 ignored, including request-boundary cases against the
Rust parameter oracle and user-ID output-capacity, 512/513-byte, non-ASCII, and
Unicode-trim edges. The feature-off provider-core suite passes 215 tests. The
Mojo core suite passes 81 tests; workspace all-target/all-feature Clippy and ownership,
authority, no-fallback, size, formatting, and diff checks pass.

The canonical broad inventory is 50,336 reachable Mojo LOC and 196,656 Rust
production LOC, or 20.37960743667811% Mojo. The 75% project target remains unmet.

## Telemetry metric-label privacy wave

`prodex-observability/mojo` now enables Mojo validation through
`prodex-domain/mojo-observability`. The production consumer is
`TelemetryAttribute::as_metric_label`; the Mojo kernel receives borrowed key/value
bytes and returns only a validation tag. Rust retains both strings and maps the
tag to the existing domain errors. Feature-off builds keep the separate Rust
implementation, and a Mojo boundary error fails closed without Rust recomputation.

Mojo preserves the existing byte bounds, printable-ASCII rules, normalized
privacy-sensitive key checks, and UUID/hex-ID value rejection. Differential tests
cover normalization, blocked substrings, ASCII and length boundaries, identifiers,
and every byte value. Feature-off and strict Mojo domain and observability tests
pass with Mojo 1.1.0; all-target workspace Clippy, formatting, docs, changed tests,
and Mojo ownership, authority, no-fallback, size, churn, and production-share
guards pass.

The canonical broad inventory is 50,451 reachable Mojo LOC and 196,717 Rust
production LOC, or 20.41162286380114% Mojo. The 75% project target remains unmet;
the report estimates 539,700 additional Mojo LOC at current Rust volume.

## Gemini Responses function-call history parts

Gemini Responses history with tool calls is outside the text-only Mojo request
kernel's supported shape, so Rust still traverses messages, parses arguments,
and correlates call IDs with tool names. The feature-on traversal now sends
canonical name, argument/response, and optional call-ID JSON fragments through
the existing Mojo `FunctionCallPart` and `FunctionResponsePart` operations.
Mojo owns those Gemini wire shapes; feature-off builds retain Rust construction.

The focused parity case runs through the same text-kernel decline and contents
dispatch as production. It verifies assistant calls, malformed-argument
defaults, mapped tool replies, IDs, and JSON versus plain-text response values
in both feature modes. No new ABI operation or dependency was needed.

The canonical inventory is 50,451 reachable Mojo LOC and 196,738 Rust
production LOC, or 20.409888789549697% Mojo. The existing Mojo source was already
counted, so this wiring wave adds no Mojo LOC and raises counted Rust adapter
volume by 21 LOC. The 75% target remains unmet; 539,763 additional Mojo LOC are
estimated at current Rust volume.

## OpenAI quota pool aggregation wave

The quota renderer now sends normalized OpenAI window rows to
`prodex_quota_openai_pool_aggregate_v1` in the new production owner
`quota_pool.mojo`. Mojo owns profile and per-window counts, remaining sums,
ready-window totals, and earliest-reset selection. Rust retains report
acquisition, window normalization, readiness evaluation, and rendering.
`i64::MAX` remains the no-reset sentinel. The OpenAI batch has no profile-count
cap; the existing 1,024-row limit remains specific to Gemini/Copilot main-quota
aggregation. Feature-off builds retain their Rust-only implementation.

With Mojo 1.1.0 and `PRODEX_MOJO_REQUIRED=1`, the Mojo core suite passes 7 tests
and the quota suite passes 71 tests; the Rust-only quota suite passes 59 tests.
The OpenAI aggregation boundary matches a Rust oracle over 2,000 generated
batches, and the renderer test aggregates 1,025 profiles. Workspace Clippy,
formatting, docs, ownership, authority, no-fallback, production-share, size, and
worktree-churn checks pass.

The canonical broad inventory is 50,540 reachable Mojo LOC and 196,886 Rust
production LOC, or 20.43% Mojo. The 75% target remains unmet; 540,118 additional
Mojo LOC are estimated at current Rust volume.

## External provider launch catalog merge

`external_catalog_models` now sends launch, dynamic, and provider IDs through the
existing `prodex_mojo_rich_catalog_merge_v1` operation. Mojo owns ordered,
non-empty, ASCII-case-insensitive ID deduplication. Rust retains catalog file
reading and parsing, metadata lookup, context limits, and model JSON construction.
The former feature-off Rust oracle was deleted in the hard-replacement checkpoint
below. Dynamic duplicates remain available to first-match
metadata lookup, while the Mojo index plan keeps the first model in output order.

The merge ABI accepts IDs across Rust's representable string range, including
values longer than 65,536 bytes. Its candidate count remains capped at 65,536;
external model catalogs stay within their existing 512-entry dynamic limit plus
the bounded static provider list.

Validation at that earlier checkpoint passed: strict Mojo core tests (84);
the long-ID Rust differential;
end-to-end duplicate metadata/order tests in Mojo and feature-off builds; and
seven external-provider catalog tests in both feature modes. `rtk cargo clippy
--locked --workspace --all-targets --all-features -- -D warnings`, `cargo fmt
--all -- --check`, `npm run docs`, `npm run test:changed`, Mojo ownership,
authority, no-fallback, production-share, size, and churn guards pass.

The canonical broad inventory is 50,541 reachable Mojo LOC and 196,879 Rust
production LOC, or 20.43% Mojo. The 75% project target remains unmet; 540,096
additional Mojo LOC are estimated at current Rust volume.

## OpenAI chat response translation wave

`translate_chat_response_to_responses` now sends the parsed completion tree
through one `openai_chat_response.mojo` operation. Mojo owns first-choice
selection, content extraction and fallback order, tool-call naming and argument
wrapping, usage aliases and defaults, and the Responses body. Rust retains
Serde parsing, clock acquisition, the result contract, and the feature-off Rust
implementation and test oracle. A Mojo boundary error does not trigger Rust
recomputation.

Validation passes with Mojo 1.1.0: the feature-on provider suite (248 passed,
two manual benchmarks ignored), the feature-off provider suite (216 passed),
5,000 generated Mojo-versus-Rust responses plus sparse/content/tool/usage edge
cases, and the `prodex-mojo-core` suite including three response ABI tests.
Workspace all-target Clippy, formatting, compatibility baseline and offline
replay, docs, changed tests, Mojo ownership/authority/no-fallback, size, churn,
and production-share guards pass.

The canonical broad inventory is 50,935 reachable Mojo LOC and 196,761 Rust
production LOC, or 20.563513338931593% Mojo. The 75% project target remains
unmet; 539,348 additional Mojo LOC are estimated at current Rust volume.

## OpenAI chat-compatible SSE event selection wave

The chat-compatible SSE bridge now passes each parsed event tree to operation
1 of prodex_mojo_openai_chat_response_v1. Mojo owns first-choice and
first-tool-delta selection, tool-over-text precedence, completion detection,
RTK argument wrapping, and Responses SSE serialization. Rust retains framing,
UTF-8 decoding, Serde parsing, [DONE] detection, result mapping, and the
feature-off implementation and differential oracle. Unsupported JSON events
remain unsupported; Mojo errors do not trigger Rust recomputation.

Validation passes with Mojo 1.1.0: 5,000 generated stream events and sparse,
precedence, malformed-field, and transport-boundary cases match the Rust
oracle byte-for-byte through both the Mojo ABI and provider translator. The
provider-core suites pass in feature-off and feature-on modes, the Mojo JSON
boundary suite passes, workspace all-target/all-feature Clippy passes, and the
ownership, authority, no-fallback, and production-share guards pass.

The canonical broad inventory is 51,019 reachable Mojo LOC and 196,735 Rust
production LOC, or 20.59% Mojo. The 75% project target remains unmet; 539,186
additional Mojo LOC are estimated at current Rust volume.

## DeepSeek chat-compatible SSE event selection wave

DeepSeek chat SSE events now pass their parsed JSON tree through operation 2 of
`prodex_mojo_openai_chat_response_v1`. Mojo owns first-choice and first-tool
delta selection, tool-over-text precedence, call-ID inclusion, and complete
Responses SSE event serialization. Rust retains framing, UTF-8 decoding, JSON
parsing, `[DONE]` handling, and result mapping. A later checkpoint deleted the
feature-off Rust oracle.
Invalid first tool deltas remain unsupported without text fallback; missing
text remains an empty text delta. Mojo errors do not trigger Rust recomputation.

The feature-on provider tests compare 5,000 generated events and sparse,
precedence, malformed-field, and transport-boundary cases against the Rust
oracle byte-for-byte through both the Mojo ABI and provider translator. The
provider-core suite passes with Mojo enabled (254 passed, two ignored) and
disabled (216 passed).

Validation passes: `cargo fmt --all -- --check`, `npm run docs`,
`npm run test:changed`, `rtk cargo clippy --locked --workspace --all-targets
--all-features -- -D warnings`, `npm run mojo:ownership`,
`npm run mojo:authority`, `node scripts/ci/mojo-no-fallback-guard.mjs`,
`node scripts/ci/mojo-production-share.mjs --check`, and `git diff --check`.

The canonical broad inventory is 51,082 reachable Mojo LOC and 196,707 Rust
production LOC, or 20.62% Mojo. The 75% project target remains unmet; 539,039
additional Mojo LOC are estimated at current Rust volume.

## Super override CLI argument-classification wave

Operation 10 of `prodex_mojo_launch_args_v1` now classifies Prodex flags in the
Codex argument tail, recognizes split and `--name=value` forms, and plans
separate-value consumption. The `--` boundary, known-flag value protection,
and non-UTF-8 passthrough match the previous scanner. Mojo returns only
override tags and argument indices. Rust keeps `OsString` ownership, typed
value validation, and `SuperArgs` updates. The feature-off Rust scanner remains
the oracle; a Mojo failure returns an error without recomputing in Rust.

Feature-on differential coverage compares every override kind and aliases,
empty and invalid values, unknown flags, `--`, and opaque arguments against the
Rust oracle. CLI tests pass with Mojo off (135 tests) and on (136 tests). The
frozen ownership inventory does not include `super_tail_extract.rs`, so this
wave adds no unsupported migration-volume claim.

Validation passes: `cargo fmt --all -- --check`,
`cargo test --locked -q -p prodex-cli`,
`PRODEX_MOJO_VERSION=1.1.0 cargo test --locked -q -p prodex-cli --features
mojo-core -- --test-threads=1`,
`PRODEX_MOJO_VERSION=1.1.0 cargo check --locked -q -p prodex-app --features
mojo-core`, `PRODEX_MOJO_VERSION=1.1.0 cargo clippy --locked --workspace
--all-targets --all-features -- -D warnings`, `npm run docs`,
`npm run test:changed`, the crate-boundary, secret-boundary, Mojo ownership,
authority, no-fallback, and production-share guards, and `git diff --check`.

The canonical broad inventory is 51,375 reachable Mojo LOC and 196,826 Rust
production LOC, or 20.70% Mojo. The 75% project target remains unmet; 539,103
additional Mojo LOC are estimated at current Rust volume.

## Telemetry metric-label ownership cleanup

`TelemetryAttribute` and its differential tests now live in
`prodex-observability`, their only in-repository consumer. The application uses
the observability-owned path. `prodex-domain` no longer depends on
`prodex-mojo-core` or exports telemetry types; its disabled Mojo governance
blocks are removed. `prodex-observability` no longer depends on the domain crate
and keeps its Mojo bridge optional behind the existing feature.

The observability boundary guard now checks the live crate API, nested metric
label module, Mojo ABI mapping, privacy validator, and active metric names. It
retains the legacy plan checks for the former source layout. Feature-off and
Mojo tests preserve safe-label, sensitive-key, identifier, and redaction
behavior. Mojo boundary errors remain fail-closed.

Validation passes: `cargo fmt --all -- --check`, domain and observability tests
with Mojo disabled and enabled, the feature-on `prodex-app` check, workspace
all-target/all-feature Clippy, `npm run docs`, `npm run test:changed`, crate,
domain, and observability boundary guards, Mojo ownership and authority guards,
the no-fallback and production-share guards, and `git diff --check`.

The canonical broad inventory is 51,375 reachable Mojo LOC and 196,784 Rust
production LOC, or 20.70% Mojo. The 75% project target remains unmet; 538,977
additional Mojo LOC are estimated at current Rust volume.

## Codex runtime feature configuration wave

`CodexRuntimeFeatureArgs` now calls `prodex_mojo_runtime_feature_plan_v1` for
web-search selection, rollout-budget eligibility and reminder filtering,
defaults, ordering and deduplication, current-time reminder enablement, and
system-proxy precedence. Rust retains Clap and `OsString` ownership, fixed-width
ABI mapping, validated result reconstruction, and TOML override rendering. The
feature-off Rust planner and test oracle were deleted after parity; builds
without `mojo-core` reject requested runtime feature overrides. Mojo errors
propagate without Rust recomputation.

The first Mojo-enabled test run exposed a reminder deduplication overwrite: the
descending scan wrote into unread lower indices. Mojo now deduplicates forward
and reverses only the unique prefix. The fixed-seed parity suite covers this
case and compares 2,000 complete rendered argument plans.

Validation passes:

- `PRODEX_MOJO_VERSION=1.1.0 rtk cargo test --locked -q -p prodex-cli --features mojo-core -- --test-threads=1` (139 passed)
- `PRODEX_MOJO_VERSION=1.1.0 rtk cargo test --locked -q -p prodex-cli -- --test-threads=1` (137 passed)
- `PRODEX_MOJO_VERSION=1.1.0 rtk cargo check --locked -q -p prodex-app --features mojo-core`
- `PRODEX_MOJO_VERSION=1.1.0 rtk cargo test --locked -q -p prodex-app --features mojo-core --lib --no-run`
- `PRODEX_MOJO_VERSION=1.1.0 rtk cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --all -- --check`, `npm run docs`, and `npm run test:changed`
- `npm run mojo:ownership`, `npm run mojo:authority`, `node scripts/ci/mojo-no-fallback-guard.mjs`, `npm run mojo:production-share`, `node scripts/ci/secret-boundary-guard.mjs`, and `git diff --check`

The canonical broad inventory is 51,569 reachable Mojo LOC and 197,051 Rust
production LOC, or 20.742096371973293% Mojo. The 75% target remains unmet;
539,584 additional Mojo LOC are estimated at current Rust volume.

## Gemini chat response message shaping reuse wave

The live buffered Gemini rewrite consumer,
`runtime_gemini_generate_buffered_response_parts`, now routes assembled chat
assistant messages through the existing `StreamAssistantMessage` operation
(operation 29) and tool-call items through `ChatFunctionCallItem` (operation
21) in `prodex_mojo_gemini_response_kernel_v1`. No Mojo operation or ABI
version was added. Rust retains provider-part collection, the blocked-tool
callback, and typed tool-call input normalization. A raw string
`functionCall.id` still takes precedence over the generated fallback, including
empty and whitespace-only values. Feature-off builds retain Rust shaping; Mojo
errors do not trigger Rust recomputation.

Validation passes: provider-core tests with Mojo disabled (216) and enabled
(254 passed, two ignored), the Mojo-enabled `prodex-app` Gemini filter (179),
workspace all-target/all-feature Clippy, formatting, docs, changed tests,
Mojo ownership/authority/no-fallback/production-share checks, the
secret-boundary guard, and `git diff --check`.

The canonical broad inventory is 51,569 reachable Mojo LOC and 197,032 Rust
production LOC, or 20.743681642471270% Mojo. The 75% target remains unmet;
539,527 additional Mojo LOC are estimated at current Rust volume.

## Anthropic Messages response planning wave

The production consumer is `AnthropicMessagesTranslator::transform_response`
in `crates/prodex-provider-core/src/translators/anthropic/messages.rs`. Its
existing rich Mojo planner now classifies Anthropic response block types,
selects text-bearing fields, and plans ordered Responses output in one bounded
call. The `v2` operation ABI requires version 7. Rust retains Serde acquisition, typed
result and rejection mapping, clock access, and web-search source merging.

The Rust classifier, planner, response envelope, rendering copies, and test
oracles were deleted. Without Mojo, response translation returns an explicit
unsupported result. Independent expected-value fixtures cover all supported
block types, empty and absent thinking text, malformed and missing fields,
unsupported types, and tool/server-tool validation order. On a classification
issue, Mojo returns the valid prefix so the caller materializes it before
reporting that issue; a regression case preserves the earlier 4 MiB
materialization error. Provider-boundary tests cover response shaping and
web-search source attachment. The stream and request feature-off copies are
separate remaining cleanup work.

Validation passed:

- `rtk cargo fmt --all --check`
- `rtk cargo test --locked -q -p prodex-provider-core --features mojo` (255 passed, 2 ignored)
- `rtk cargo test --locked -q -p prodex-provider-core --no-default-features` (213 passed)
- `PRODEX_MOJO_VERSION=1.1.0 rtk cargo test --locked -q -p prodex-mojo-core --features mojo-rich response_plan_abi_rejects_stale_version_and_bad_bounds` (passed)
- `PRODEX_MOJO_VERSION=1.1.0 rtk cargo test --locked -q -p prodex-app --features mojo-core --lib native_web_search_route_is_selected_only_for_native_modes_with_options -- --test-threads=1` (passed)
- `rtk cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `rtk npm run docs`
- `rtk npm run test:changed -- --base HEAD --no-untracked` (passed)
- `rtk node scripts/ci/mojo-ownership.mjs --check --json`
- `rtk node scripts/ci/mojo-ownership.mjs --self-test --check`
- `rtk node scripts/ci/mojo-authority-guard.mjs`
- `rtk node scripts/ci/mojo-authority-guard.mjs --self-test`
- `rtk node scripts/ci/mojo-no-fallback-guard.mjs`
- `rtk node scripts/ci/mojo-no-fallback-guard.mjs --self-test`
- `rtk node scripts/ci/mojo-production-share.mjs --check`
- `rtk node scripts/ci/secret-boundary-guard.mjs`
- `rtk git diff --check`
- `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0 PRODEX_MOJO_TARGET=aarch64-apple-darwin PRODEX_MOJO_TARGET_CPU=generic rtk cargo build --release --locked --target aarch64-apple-darwin -p prodex-mojo-core --features mojo-runtime,mojo-quota,mojo-rich` (passed; GNU archive-format warning from Linux `ar`)

The canonical broad inventory is 51,624 reachable Mojo LOC and 197,305 Rust
production LOC, or 20.74% Mojo. The 7% release floor passes; the 75% project
target remains unmet, with 540,291 additional Mojo LOC needed at the current
Rust volume.

## Retired audit decision ABI cleanup

The old `prodex_mojo_audit_decision_v1` export had no Rust caller after the
domain audit plane was retired. Its time-range, ordering, retention, expiry,
and hold decisions had no live production consumer; the current audit log has
a different contract. The unused export and its private operation constants
were deleted rather than counted as reachable production behavior. The
retention constants still used by gateway policy remain.

The canonical broad inventory is now 51,560 reachable Mojo LOC and 197,305
Rust production LOC, or 20.72% Mojo. The release floor and non-regression
check pass; the 75% project target remains unmet, with 540,355 additional
Mojo LOC needed at the current Rust volume.

## Hard-replacement cleanup checkpoint

The Anthropic request and stream bridges, OpenAI chat response and SSE bridge,
provider model fallback chain, Codex runtime feature planner, Critical Signal,
runtime tuning capacity defaults, Smart Context adaptive/calibrated/observed
planning, and quota capacity admission now have no Rust feature-off semantic
copy or Rust test oracle. Rust keeps input acquisition, typed ABI adaptation,
and host effects. Unsupported feature-off capabilities fail closed. Quota
capacity uses the versioned `prodex_quota_capacity_batch_v2` ABI; the old
scalar readiness export is gone. The provider fallback boundary accepts long
model identifiers and grows caller-owned output buffers as needed. Normal
`prodex-quota` builds enable Mojo by default, so app runtime selection retains
capacity decisions. Normal `prodex-app` builds now enable `mojo-core` so the
default Super shortcut retains Smart Context rewrites. Explicit feature-off
builds exclude these Mojo-owned capabilities.

Independent expected-value and caller-boundary tests replace the deleted
oracles. Final focused source runs pass: provider-core 209 tests without Mojo
and 256 with Mojo (two ignored), quota 44 without Mojo and 72 with Mojo,
Mojo-core quota capacity 3 tests, and runtime-proxy pressure snapshot 2 tests.
The `prodex-app` runtime proxy filter also passes 360 tests with its default
features after the quota default was updated.
The provider SPI guard was aligned with the active retry-plan contract after
the older response-plan contract had been retired. The historical ownership
guard passes at 423/4,227 eligible Rust semantic LOC migrated (10.01%).
The modified Rich and quota Mojo roots compile to objects with pinned Mojo
1.1.0 for all six release target triples. Only Linux x86_64 ran these tests;
cross-target object compilation is not native runtime evidence.

The runtime doctor now enables its Mojo log parser by default. Its former Rust
message tokenizer, marker-known classifier, timeline phase, selection bucket,
and route action copies and differential oracles are deleted. The explicit
feature-off build omits log summarization while retaining unrelated doctor
capabilities. After deletion, 31 default, 33 all-feature, and 15 feature-off
doctor tests pass; focused all-feature Clippy and format checks also pass.

The current canonical broad inventory is **51,633 reachable Mojo LOC** and
**196,700 Rust production LOC**, totaling **248,333 LOC**: **20.79% Mojo**.
The 7% release floor and audited non-regression check pass. The 75% project
target remains unmet; at the current Rust volume the report estimates
**538,467 additional Mojo LOC** are needed.

## Quota, rehydration, and doctor hard replacements

Quota error and blocked-status labels now always use the existing Mojo
classifiers, including in `--no-default-features` builds. The feature-off Rust
classifiers and their helpers are deleted. The quota suite passes 72 default
and 44 feature-off tests; focused all-target Clippy passes with warnings denied.

Smart Context rehydration now delegates required-first ordering and artifact
presence lookup to a bounded Mojo ABI before the existing Mojo admission plan.
The Rust sort, feature-off planner, and differential oracle are deleted; the
feature-off proxy excludes this API. Fixed expected-value tests cover ordering,
duplicate IDs, missing artifacts, minimal tier, budget overflow, and the 256-item
ABI limit. Runtime-proxy suites pass 349 Mojo-enabled and 298 feature-off tests.
The changed Mojo root compiles to objects for all six release target triples;
only Linux x86_64 runtime execution was tested. Runtime smoke, offline upstream
baseline, capture replay, ownership, authority, no-fallback, and changed-test
checks pass. The full app runtime-proxy filter passed 364 serial tests on a
rerun after a temporary concurrent Gemini compiler error was fixed.

Default runtime-doctor builds now use the Mojo route planner for profile
summaries. Its Rust summary planner and differential oracle are deleted;
feature-off builds omit that capability while keeping unrelated doctor views.
Independent caller-boundary expectations cover profile order, freshness,
health decay, quota bands, circuits, and backoff. The doctor suite passes 32
default, 33 all-feature, and 13 feature-off tests; focused all-target Clippy
passes with warnings denied.

The canonical report for committed `HEAD` (`80995ade6`) counts **51,783 reachable
Mojo LOC** and **196,573 Rust production LOC**, totaling **248,356 LOC**:
**20.85% Mojo**. The 7% release floor and non-regression check pass. The 75%
project target remains unmet; **537,936 additional Mojo LOC** are required at
the current Rust volume.

## Super override hard replacement

Super override classification now calls the existing Mojo scanner in default,
Mojo-enabled, and `--no-default-features` CLI builds. The Rust scanner and its
differential oracle were deleted. Rust still parses typed values, preserves
`OsString` arguments, and applies the scan result. Caller-boundary expectations
cover all 37 override kinds, precedence, passthrough, and partial mutation.
The CLI suite passes 135 unit and 2 integration tests by default and without
default features, and 139 unit and 2 integration tests with `mojo-core`.
The Mojo launch ABI tests, workspace Clippy, ownership, authority, and
no-fallback guards pass after deletion.

The canonical report for this checkpoint counts **51,783 reachable Mojo
LOC** and **196,508 Rust production LOC**, totaling **248,291 LOC**:
**20.86% Mojo**. The 7% release floor and non-regression check pass. The 75%
project target remains unmet; **537,741 additional Mojo LOC** are required at
the current Rust volume.

## Gemini schema and tool-choice hard replacement

Gemini function-schema sanitization and tool-choice mapping now call the existing
Mojo kernels in both default and `mojo` provider builds. The Rust schema
sanitizer, composition helper, feature-off tool-choice mapper, and test oracle
were deleted. Rust keeps Serde acquisition and typed validation at the request
boundary. Caller and ABI tests cover wrong field types, validation order,
Unicode, duplicate array entries, large inputs, and nested-versus-outer tool
names. The existing Mojo rule for an empty `anyOf` followed by a single
`oneOf` remains the canonical output. Provider tests pass 213 without the
feature and 259 with it (two ignored); five ABI tests and workspace Clippy
pass after deletion.

The canonical report for this checkpoint counts **51,788 reachable Mojo LOC**
and **196,470 Rust production LOC**, totaling **248,258 LOC**:
**20.86% Mojo**. The 7% release floor and non-regression check pass. The 75%
project target remains unmet; **537,622 additional Mojo LOC** are required at
the current Rust volume.

## Response-forwarding classifier hard replacement

Proxy response header skipping, SSE content-type detection, token-usage event
loggability, and generation-start detection now call the existing Mojo
classifier in all feature combinations. The Rust feature-off classifier and
test oracle were deleted. Mojo uses the shared Unicode trim and UTF-8 validator
for header names, matching the prior Rust `trim()` boundary. Caller tests cover
fixed event and header values, Unicode, and malformed ABI input. Proxy suites
pass 303 tests without Mojo and 353 with it. Runtime smoke, the 364-test serial
app proxy filter, hot-path and manifest guards, offline upstream baseline and
capture replay, changed tests, workspace Clippy, Mojo ownership/authority,
and no-fallback guards pass after deletion. The changed Mojo root compiles to
objects for all six release triples; native execution was Linux x86_64 only.

The canonical report for this checkpoint counts **51,784 reachable Mojo LOC**
and **196,462 Rust production LOC**, totaling **248,246 LOC**:
**20.86% Mojo**. The 7% release floor and non-regression check pass. The 75%
project target remains unmet; **537,602 additional Mojo LOC** are required at
the current Rust volume.

## Quota pool aggregation hard replacement

Quota pool summaries now call the existing Mojo main and OpenAI aggregation
kernels in default and `--no-default-features` builds. The Rust feature-off
aggregators, obsolete wrappers, and differential oracles were deleted. Fixed
caller expectations cover mixed providers, absent and failed reports, summary
ordering, and reset sentinels. Mojo ABI expectations cover the 1,024-row main
limit, OpenAI aggregation bounds, and invalid percentages. After deletion,
quota tests pass 72 default and 45 feature-off cases; four Mojo ABI tests,
focused Clippy, formatting, ownership, authority, and no-fallback guards pass.

The canonical report for this checkpoint counts **51,784 reachable Mojo LOC**
and **196,359 Rust production LOC**, totaling **248,143 LOC**:
**20.87% Mojo**. The 7% release floor and non-regression check pass. The 75%
project target remains unmet; **537,293 additional Mojo LOC** are required at
the current Rust volume.

## Gemini stream shaping hard replacement

Gemini SSE event selection, response events, and supported tool-call item
shaping now use the existing Mojo response kernel in both provider feature
modes. The matching Rust feature-off branches were deleted. Independent tests
cover event precedence, sparse calls, malformed payloads, Unicode, omitted
fields, special tool items, and stream completion. The stream tests were split
into a focused sibling file to satisfy the source-size guard. After deletion,
provider-core tests pass 214 default and feature-off cases, and 260 Mojo cases
with two ignored; 53 app Gemini SSE integration tests, changed tests, workspace
Clippy, formatting, secret-boundary, ownership, authority, and no-fallback
guards pass. Arbitrary thought-signature shapes, RTK argument normalization,
and tool-call ID filtering remain separate Rust-owned work because the existing
Mojo operation does not represent those inputs.

The canonical report for this checkpoint counts **51,784 reachable Mojo LOC**
and **196,327 Rust production LOC**, totaling **248,111 LOC**:
**20.87% Mojo**. The 7% release floor and non-regression check pass. The 75%
project target remains unmet; **537,197 additional Mojo LOC** are required at
the current Rust volume.

## Capacity, provider, and runtime hard replacements

OpenAI model-capacity decisions now use the existing Mojo plan in both quota
feature modes. Rust feature-off classifiers and model-capacity test oracles were
deleted. The no-default build still fails closed for unavailable admission
capacity; Luna reserve model slugs now use the same Mojo normalization as the
default build instead of the former exact-only feature-off comparison.

Gemini response and chat tool-call items now use the existing Mojo response
kernel in both feature modes. DeepSeek chat SSE translation likewise calls its
existing Mojo kernel unconditionally. Their Rust feature-off item builders,
DeepSeek SSE serializer, and differential oracles were deleted. Permanent
caller-boundary tests assert complete expected JSON and SSE bytes, precedence,
Unicode, malformed shapes, and error behavior. The shared Serde JSON document
builder is available in both provider feature modes.

Smart Context body-token estimation now calls Mojo in both runtime-proxy feature
modes. The Rust estimator and test oracle were deleted, together with two
unused proxy wrappers. Runtime profile-health score, latency, inflight, and
transition Rust test oracles were also deleted; fixed expected values remain in
proxy and Mojo ABI tests. The no-fallback guard now covers these completed
operations. No Mojo source or ABI changed in this checkpoint.

After deletion, provider-core tests pass 217 without default features and 260
with Mojo (two ignored); quota tests pass 45 without default features and 72 by
default. Runtime-proxy tests pass 300 without default features and 349 with
Mojo. The 364-test serial app runtime-proxy filter and runtime smoke pass.

The canonical report counts **51,784 reachable Mojo LOC** and **196,021 Rust
production LOC**, totaling **247,805 LOC**: **20.90% Mojo**. The 7% release
floor and non-regression check pass. The 75% project target remains unmet;
**536,279 additional Mojo LOC** are required at the current Rust volume.

## Request and Smart Context hard replacements

Kiro request validation and body rewriting now call the existing Mojo kernels in
both provider feature modes. The in-file Rust oracle and two orphaned request
control/validation modules were deleted. The shared Responses-to-chat request
bridge likewise calls one Mojo transform in both modes; its Rust translator,
validator, input-content oracle, and unused text helpers were deleted. Its
supported-parameter report also comes from Mojo. Permanent expected-value
fixtures cover rejection precedence, wrong types, tool history, Unicode,
duplicate keys, ordering, and large text.

DeepSeek's completed response event, output item, tool-call item, and delta JSON
builders now call the existing Mojo kernel in both modes; five separate stream
projection and metadata operations remain outside this wave. Smart Context
affinity rewrite permission, rollout planning, and regression self-checks also
use their existing Mojo kernels in both runtime-proxy feature modes. Their Rust
feature-off algorithms and differential oracles were deleted; fixed caller
expectations cover safety blocks, canary precedence, and saturating token math.
The ownership and no-fallback guards reject retained Rust semantic states and
reintroduced copies for these completed operations. An attempted Gemini request
wave was withheld because its text-only Mojo kernel cannot yet preserve tool
history, non-text inputs, and custom tools; the existing Gemini behavior remains.

After Rust deletion, provider-core tests pass 217 cases without default features
and 256 with Mojo (two manual tests ignored). Runtime-proxy tests pass 301
without default features and 347 with Mojo. The 364-test serial app proxy filter,
runtime smoke, offline upstream baseline/replay, formatter, workspace Clippy,
Mojo ownership/authority/no-fallback, and runtime hot-path/manifest guards pass.

The canonical report counts **51,784 reachable Mojo LOC** and **195,436 Rust
production LOC**, totaling **247,220 LOC**: **20.95% Mojo**. The 7% release
floor and non-regression check pass. The 75% project target remains unmet;
**534,524 additional Mojo LOC** are required at the current Rust volume.

## Gemini status and usage hard replacement

Gemini finish-reason and prompt-block status mapping, plus response usage JSON
building, now call the existing Mojo response kernel in both provider feature
modes. The Rust feature-off mappings, usage builder, and differential oracles
were deleted. Independent caller tests cover the supported and unknown reasons,
Unicode, absent or invalid totals, explicit zero, and saturating token sums.
Invalid `totalTokenCount` is treated as absent before the Mojo call, matching
the previous non-Mojo behavior. The no-fallback guard covers both files.

Provider-core tests pass 223 cases by default and without default features,
and 261 with Mojo (two manual tests ignored). Formatter, ownership, authority,
no-fallback, and production-share checks pass after Rust deletion.

The canonical report counts **51,784 reachable Mojo LOC** and **195,432 Rust
production LOC**, totaling **247,216 LOC**: **20.95% Mojo**. The 7% release
floor passes; the 75% project target remains unmet, with **534,512 additional
Mojo LOC** required at the current Rust volume.

## Kiro stream and runtime doctor hard replacement

Kiro stream event shaping, content extraction, and tool-activity sanitization
now call the existing Mojo kernel in both provider feature modes. The Rust
feature-off stream builders and sanitizers were deleted. Caller tests retain
explicit expected values for redaction, Unicode truncation, and large escaped
content below the current 4 MiB Kiro ABI limit.

Runtime doctor next-step guidance, policy suggestions, and final diagnosis now
use Mojo regardless of the optional `mojo` feature. Their four Rust fallback
modules and temporary parity oracles were deleted. Differential checks covered
fixture logs, every registered marker with and without fields, and pairs of
priority markers before deletion. Mojo diagnosis now matches the former
default-build behavior for missing fields, compatibility warnings, compact
exits, provider markers, and Gemini retry markers. Permanent caller tests
assert complete guidance and diagnosis values. The no-fallback guard covers
the replaced paths and rejects restored fallback modules.

After deletion, provider-core tests pass 224 cases by default and without
default features, and 262 with Mojo (two manual tests ignored). Runtime-doctor
tests pass 18 without default features and 37 by default and with Mojo;
`prodex-mojo-core --features mojo-rich` passes 47 tests.

The canonical report counts **51,785 reachable Mojo LOC** and **195,359 Rust
production LOC**, totaling **247,144 LOC**: **20.95337131388988% Mojo**. The 7%
release floor and non-regression check pass. The 75% project target remains
unmet; **534,292 additional Mojo LOC** are required at the current Rust volume.

## Runtime doctor state-plan hard replacement

Quota freshness and degraded-route score decay now call the existing Mojo state
plan in all doctor feature modes. Their Rust feature-off implementations were
deleted. A signed-time parity test found that the Mojo boundary rejected a
negative stale grace accepted by the old Rust helper; state-plan ABI v2 now
accepts that input for quota planning and uses the shared saturating subtraction
kernel. Fixed caller tests cover grace boundaries, extreme timestamps, and
score decay. The no-fallback guard protects both promoted files.

After deletion, runtime-doctor tests pass 20 cases without default features and
39 with default or Mojo features; `prodex-mojo-core --features mojo-rich`
passes 47 tests. The changed Mojo module compiles to objects for all six
release triples; native execution was tested on Linux x86_64 only. The
canonical report counts **51,779 reachable Mojo LOC** and
**195,295 Rust production LOC**, totaling **247,074 LOC**:
**20.956879315508715% Mojo**. The 7% floor and non-regression check pass;
the 75% project target remains unmet, with **534,106 additional Mojo LOC**
required at the current Rust volume.

## Provider catalog, DeepSeek request, and runtime selection hard replacements

Provider catalog identity, picker ordering and deduplication, discovered-model
merging, reasoning resolution, and model lookup now use the existing Mojo
kernels in both provider feature modes. The Rust fallbacks and parity oracles
were deleted; the public `ProviderModelSpec` matcher now forwards to Mojo.
The Rust adapter trims model lookup input before the bounded Mojo query, so
IDs with over 4 KiB of leading Unicode whitespace retain their established
behavior. Mojo still owns identity matching and selection.
Independent expected-value tests cover ordering, aliases, Unicode, and long
inputs.

The provider-core DeepSeek Responses-to-chat translator now uses the raw Mojo
kernel in both feature modes. Non-object input items retain empty user messages,
Unicode whitespace-only tool choices are omitted, and kernel errors become
normal transform rejections. The old Rust request builder, parameter helpers,
message builders, and temporary comparison oracle were deleted. The existing
`deepseek_bridge/request_params.rs` feature-off parameter-validation helpers
remain separate cleanup work; they were not the body-shaping implementation
replaced in this wave. The app's complex live-request rewrite is another
production path and remains separate migration work. The Mojo build script
watches `deepseek.mojo`, `json_view.mojo`, and `gemini_config.mojo` so changes to
imported sources rebuild the archive instead of using stale output.

Runtime proxy selection affinity, quota-release eligibility, WebSocket reuse,
continuation priority, direct-current fallback, and soft-affinity decisions now
call Mojo in both feature modes. Their Rust fallback/oracle module was deleted.
A new versioned WebSocket stale-reuse entrypoint compares seconds and
nanoseconds, preserving the former Rust boundary for sub-millisecond idle
durations. Expected-value tests retain hard-affinity precedence, quota states,
and stale-reuse thresholds; the request and stream commit paths were untouched.

After deletion, provider-core tests pass 233 cases in default and no-default
modes and 267 with Mojo (two manual tests ignored). Runtime-proxy tests pass
303 cases in default and no-default modes and 348 with Mojo. The 364-case
serial app runtime-proxy filter, runtime smoke, offline upstream baseline and
five replay fixtures, formatter, full-workspace Clippy with warnings denied,
ownership/authority/no-fallback, hot-path, manifest, and crate-boundary guards
pass. `npm run ci` passes on the combined checkpoint. Object compilation of
`candidate_decision.mojo` and the DeepSeek-importing `rich_abi.mojo` passes for
all six release targets; native execution evidence is Linux x86_64 only.

The canonical report counts **51,907 reachable Mojo LOC** and **194,946 Rust
production LOC**, totaling **246,853 LOC**: **21.027494095676374% Mojo**. The
7% release floor and non-regression check pass; the 75% project target remains
unmet, with **532,931 additional Mojo LOC** required at the current Rust volume.

## DeepSeek parameters, prompt-cache affinity, and proxy quota scoring

DeepSeek request parameter validation and normalization now use the existing
Mojo policy and kernel in every provider feature mode. The Rust feature-off
validators and their test oracle were deleted. Mojo preserves validation
precedence, the 16-stop limit, and Unicode-trimmed user IDs up to 512 bytes.
The Rust boundary still acquires JSON fields and translates typed kernel results
into the established provider errors.

Prompt-cache affinity hashing and owner preference now use Mojo in every proxy
feature mode; the Rust hash implementation was deleted. The Mojo kernel now
matches the previous FNV offset and Unicode trimming, including keys longer
than 4 KiB. The Rust adapter splits batches above the 256-profile ABI limit
without reimplementing the hash or preference. Proxy quota pressure banding and
route scoring likewise use Mojo in every feature mode. Their Rust fallback and
pressure-band test oracle were deleted, and both feature modes share one Rust
ABI adapter. Snapshot and precommit policy paths remain separate work.

After deletion, provider-core tests pass 235 cases in default and no-default
modes and 268 with Mojo (two manual tests ignored). Runtime-proxy tests pass
305 cases in default and no-default modes and 350 with Mojo. The 364-case
serial app runtime-proxy filter, runtime smoke, offline upstream baseline and
five replay fixtures, formatter, ownership/authority/no-fallback, hot-path,
manifest, and crate-boundary guards pass. The changed Mojo kernels compile to
objects for all six release targets; native execution was tested on Linux
x86_64 only.

The canonical report counts **51,921 reachable Mojo LOC** and **194,791 Rust
production LOC**, totaling **246,712 LOC**: **21.045186290087226% Mojo**. The
7% release floor and non-regression check pass; the 75% project target remains
unmet, with **532,452 additional Mojo LOC** required at the current Rust volume.

## Quota display policy hard replacement

The quota renderer now uses the existing Mojo kernels in every feature mode for
float rounding, remaining percentage, window status, and pressure-band mapping.
The corresponding Rust feature-off calculations and test oracles were deleted.
Expected-value tests cover percentage bounds, half-step rounding, absent
windows, and pressure precedence. The owning crate passes 46 tests without
default features and 72 with default or Mojo features. Formatter and the
no-fallback guard pass after deletion.

The canonical report counts **51,921 reachable Mojo LOC** and **194,782 Rust
production LOC**, totaling **246,703 LOC**: **21.04595404190464% Mojo**. The
7% release floor and non-regression check pass; the 75% project target remains
unmet, with **532,425 additional Mojo LOC** required at the current Rust volume.

## Runtime health, observability, and Retry-After hard replacement

Runtime-store profile-health sort keys now use the existing Mojo batch in every
feature mode. The Rust feature-off scorer was deleted; expected-value tests cover
route coupling, stable batch order, the 256-row ABI boundary, saturation, and
extreme timestamps. The live app selection catalog remains the caller.

Observability metric names, typed labels, and metric-label privacy validation
now use their existing Mojo kernels in every feature mode. The Rust catalog,
validator, and parity oracle were deleted. Boundary tests assert exact public
names, labels, sensitive-key rejection, identifier rejection, and redaction.

The proxy Retry-After numeric conversion now uses the existing Mojo parser in
every feature mode. Its Rust number parser and test oracle were deleted; 19
explicit expected-value cases cover rounding, caps, overflow, and malformed
numbers. The token/header acquisition and error-policy commit rules remain in
their existing Rust owners. `prodex-runtime-proxy` now declares `mojo-rich` as a
normal dependency instead of relying on feature unification through tests.
The no-fallback guard covers all three promoted paths, and the ownership
manifest records the unconditional runtime-store adapter.

After deletion, runtime-store tests pass 29 cases in default, no-default, and
Mojo modes; observability passes 11 in each mode; runtime-proxy passes 306 in
default and no-default modes and 350 with Mojo. The 364-case serial app proxy
filter, runtime smoke, offline upstream baseline and five replay fixtures,
formatter, workspace Clippy, docs, changed-tests, ownership/authority/no-fallback,
hot-path, manifest, crate-boundary, and `npm run ci` checks pass on Linux x86_64.
No Mojo kernel changed in this wave; native macOS/Windows execution was not run.

The canonical report counts **51,921 reachable Mojo LOC** and **194,714 Rust
production LOC**, totaling **246,635 LOC**: **21.051756644434082% Mojo**. The
7% release floor and non-regression check pass; the 75% project target remains
unmet, with **532,221 additional Mojo LOC** required at the current Rust volume.

## Gemini, registry, DeepSeek messages, and proxy payload/log hard replacement

Gemini quota bucket numerics now use bounded Mojo batches in every feature mode;
the Rust numeric fallback was deleted and rendering covers batches beyond the
1,024-row ABI limit. The provider implementation registry now uses its Mojo plan
and alias resolver in every mode, and its 362-line Rust registration copy was
deleted. Rust still materializes typed provider descriptors and translators.

DeepSeek thinking/content normalization, tool-call adjacency, and metadata merge
now use the existing Mojo message kernel in every mode. The feature-off Rust
message implementation, adjacency module, and differential oracle were removed.
The shared Serde JSON-array builder is available in every mode. Input-item
shaping remains a separate incomplete migration: the Mojo path emitted invalid
JSON for a Unicode content array during parity testing, so its Rust path was
not deleted.

The runtime proxy now uses Mojo in every mode for SSE line/commit planning,
previous-response error classification, and structured log tokenization. The
replaced Rust scanners, classifiers, feature-off branches, and test oracles were
deleted. A differential log test exposed form feed as a Rust whitespace byte;
the owning Mojo parser now matches it. Expected-value tests retain SSE signal
precedence, malformed lines, log fields, and quoted form-feed values. The
no-fallback guard covers the promoted paths.

After deletion, provider-core tests pass 242 cases in default and feature-off
modes and 271 with Mojo (one manual test ignored). Quota tests pass 73/47/73
across default, feature-off, and Mojo modes. Proxy tests pass 311/311/350 for
the log replacement; the SSE and previous-response replacements passed their
own three-mode tests, runtime smoke, serial app proxy tests, and offline replay.
The form-feed Mojo ABI test and ownership, authority, no-fallback, documentation,
and staged churn guards pass. Native execution evidence is Linux x86_64; no
macOS or Windows runtime execution was run for this checkpoint.

The canonical staged-source report counts **51,921 reachable Mojo LOC** and
**194,411 Rust production LOC**, totaling **246,332 LOC**:
**21.077651299871718% Mojo**. The 7% release floor and non-regression check
pass; the 75% project target remains unmet, with **531,312 additional Mojo
LOC** required at this Rust volume.

## Runtime proxy error-policy hard replacement

HTTP and stream error classification, code and message signals, and the official
rate-limit header classification now use the existing Mojo kernels in every
feature mode. The duplicate Rust rule table, JSON walker, signal predicates,
feature-off branches, and differential test oracle were deleted. Rust retains
header acquisition, phase-aware action materialization, retry-after handling,
and conservative pass-through on an invalid Mojo result.

Before deletion, differential tests exercised the Mojo and Rust decisions.
After deletion, runtime-proxy tests pass 315 cases in default and feature-off
modes and 353 with Mojo. Expected-value tests cover malformed JSON, competing
signals, generic 429 pass-through, and committed header behavior. Focused
Clippy and formatter checks pass. Native execution evidence is Linux x86_64;
no macOS or Windows runtime execution was run for this checkpoint.

The canonical source report counts **51,921 reachable Mojo LOC** and
**194,057 Rust production LOC**, totaling **245,978 LOC**:
**21.107985266975096% Mojo**. The 7% release floor and non-regression check
pass; the 75% project target remains unmet, with **530,250 additional Mojo
LOC** required at this Rust volume.

## Compatibility-surface hard replacement

`runtime_detect_request_compatibility_surface` now uses
`compatibility_surface.mojo` in both feature modes. The feature-off Rust
classifier and test oracle were deleted. Rust still acquires route, header,
and JSON facts for the Mojo plan. Tool labels above the 1,024-item ABI limit
are sent in bounded batches, and only their Mojo-derived tool flags are merged.

Before deletion, differential tests compared request stages and HTTP/WebSocket
transport against the Rust implementation. After deletion, runtime-proxy tests
pass 327 cases in default and feature-off modes and 360 with Mojo. Expected-value
tests cover metadata, malformed and mistyped JSON, and 1,025 tool labels;
focused Clippy, formatter, and size guards pass. Native execution evidence is
Linux x86_64; macOS and Windows runtime execution was not run.

The canonical source report counts **51,921 reachable Mojo LOC** and
**193,891 Rust production LOC**, totaling **245,812 LOC**:
**21.122239760467348% Mojo**. The 7% release floor and non-regression check
pass; the 75% project target remains unmet, with **529,752 additional Mojo
LOC** required at this Rust volume.

## Previous-response attempt-outcome hard replacement

Previous-response fallback, stale-continuation, WebSocket affinity, and bounded
retry decisions now use the existing Mojo plan in every feature mode. The
feature-off Rust decision module, retry schedule, and differential oracle were
deleted. Rust retains typed input and output mapping for the proxy callers.

Before deletion, differential tests covered route and affinity combinations,
fallback shapes, turn-state presence, and retry indices. Permanent expected-value
tests cover fail-closed continuation, session affinity, WebSocket owner binding,
and the bounded retry schedule. Native validation is Linux x86_64; macOS and
Windows runtime execution was not run.

The canonical source report counts **51,921 reachable Mojo LOC** and
**193,869 Rust production LOC**, totaling **245,790 LOC**:
**21.12413035518125% Mojo**. The 7% release floor and non-regression check
pass; the 75% project target remains unmet, with **529,686 additional Mojo
LOC** required at this Rust volume.

## DeepSeek input-item hard replacement

Input-item shaping now calls the DeepSeek Mojo kernel in every feature mode.
The feature-off Rust item builder and its field helpers were deleted. Rust
still performs replay filtering, Serde handoff, and typed ABI mapping. The Mojo
content parser now emits escaped JSON newlines and skips mistyped `text` fields
when a valid `input_text` or `output_text` field is present.

Differential tests established parity before removing the Rust builder.
Permanent expected-value tests cover Unicode and escaping, field precedence,
message order, empty content, large content, and malformed kernel input. The
input-item tests moved to a focused sibling module and the old size allowlist
cap was lowered. Provider-core tests pass in default, feature-off, and Mojo
modes; focused Clippy, formatter, size, and no-fallback guards pass. Native
execution evidence is Linux x86_64; macOS and Windows runtime execution was
not run.

The canonical source report counts **51,921 reachable Mojo LOC** and
**193,616 Rust production LOC**, totaling **245,537 LOC**:
**21.145896545123545% Mojo**. The 7% release floor and non-regression check
pass; the 75% project target remains unmet, with **528,927 additional Mojo
LOC** required at this Rust volume.

## DeepSeek buffered-response hard replacement

Buffered DeepSeek responses now use the existing Mojo response kernel in every
feature mode. The feature-off Rust response-object assembly was deleted; Rust
still extracts upstream fields, translates tool calls, and supplies typed JSON
to Mojo. Expected whole-response tests cover Unicode text and tool arguments,
usage and metadata, malformed tool arguments, and missing fields. The focused
response and end-to-end caller tests pass in default, feature-off, and Mojo
modes. Native execution evidence is Linux x86_64; macOS and Windows runtime
execution was not run.

The canonical source report counts **51,921 reachable Mojo LOC** and
**193,611 Rust production LOC**, totaling **245,532 LOC**: **21.15% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **528,912 additional Mojo LOC** required at this Rust
volume.

## Quota window capacity hard replacement

Quota window rendering and selection now call the existing capacity Mojo
kernel in both feature modes. The feature-off false and empty-result stubs
were deleted. Both modes share the existing Rust input adapter; it splits
provider-supplied windows into Mojo batches of at most 256 while preserving
their order. Expected-value tests cover invalid and missing windows, reset
pressure, admission holds, Unicode labels, and 256/257/513-row boundaries.
Native execution evidence is Linux x86_64; macOS and Windows runtime
execution was not run.

The canonical source report counts **51,921 reachable Mojo LOC** and
**193,609 Rust production LOC**, totaling **245,530 LOC**: **21.15% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **528,906 additional Mojo LOC** required at this Rust
volume.

## WebSocket response and event policy hard replacement

Committed-profile promotion, precommit hold and transport retry, direct-current
retry reset, and WebSocket event-kind classification now use the existing Mojo
plans in every feature mode. The feature-off Rust predicates and three event
tables were deleted. The app delegates its corresponding promotion and retry
decisions to the shared Mojo-backed proxy API. Expected-value tests cover
affinity inputs, fallback reasons, all known event kinds, unknown and oversized
kinds, and the app's retry policy. The no-fallback guard now rejects renewed
feature-off paths in both shared WebSocket policy modules. Native execution
evidence is Linux x86_64; macOS and Windows runtime execution was not run.

The canonical source report counts **51,921 reachable Mojo LOC** and
**193,587 Rust production LOC**, totaling **245,508 LOC**: **21.15% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **528,840 additional Mojo LOC** required at this Rust
volume.

## Smart Context token-budget hard replacement

Budget-tier selection, memory-capsule token budgets, and capsule admission now
use the existing Mojo kernels in every feature mode. The feature-off Rust
thresholds, budget gates, and capsule selector were deleted. Rust maps the
typed policy inputs, orders capsule records, and splits lists beyond the
65,536-item ABI bound into Mojo calls while carrying the remaining budget.
Independent expected-value tests cover threshold and safety boundaries,
Unicode ordering, integer overflow, malformed ABI inputs, and a 65,537-item
caller input. The budget tests live in a focused sibling file. Native
execution evidence is Linux x86_64; macOS and Windows runtime execution was
not run.

The canonical source report counts **51,921 reachable Mojo LOC** and
**193,567 Rust production LOC**, totaling **245,488 LOC**: **21.15% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **528,780 additional Mojo LOC** required at this Rust
volume.

## Gemini request classifier and built-in tool hard replacement

The Gemini simple-request classifier, native project stamping, built-in tool
grouping, and unsupported-tool removal now use the existing Mojo request
kernels in both feature modes. Their feature-off Rust classifiers and mapping
helpers were deleted. Rust canonicalizes JSON before byte-oriented Mojo scans
so escaped keys, duplicate keys, and object order keep the previous wire
behavior. Caller tests cover Unicode, malformed input, tool ordering, and
wrong-type `computerUse` precedence. Native execution evidence is Linux
x86_64; macOS and Windows runtime execution was not run.

The canonical source report counts **51,919 reachable Mojo LOC** and
**193,497 Rust production LOC**, totaling **245,416 LOC**: **21.16% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **528,572 additional Mojo LOC** required at this Rust
volume.

## Smart Context volatile normalization hard replacement

Command-output and static-context volatile normalization now call the existing
Mojo kernel in every feature mode. The 657-line feature-off Rust parser and its
private oracle were deleted. The typed adapter retries once when replacements
expand a bounded input beyond its initial output buffer. Expected-output tests
cover ANSI escapes, Unicode path boundaries, timestamps, counters, IDs, and a
2.4 MiB input that expands to 8.8 MiB. The production static-context caller
limits its input to 256 KiB; the Mojo normalization ABI retains its 4 MiB input
limit. Native execution evidence is Linux x86_64; macOS and Windows runtime
execution was not run.

The canonical source report counts **51,975 reachable Mojo LOC** and
**193,504 Rust production LOC**, totaling **245,479 LOC**: **21.17% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **528,537 additional Mojo LOC** required at this Rust
volume.

## Runtime precommit budget hard replacement

The precommit attempt and elapsed-time budget now come from the Mojo planner in
every feature mode. The feature-off Rust planner and the Rust test oracle were
deleted. The versioned v2 ABI accepts unsigned 64-bit inputs and uses 128-bit
intermediate arithmetic to preserve saturating behavior at large profile
counts. Expected-value tests cover standard, pressure, continuation, elapsed,
and platform-width boundaries. The Mojo kernel compiled to objects for all six
release targets. Native execution evidence is Linux x86_64; macOS and Windows
runtime execution was not run.

The canonical source report counts **51,980 reachable Mojo LOC** and
**193,469 Rust production LOC**, totaling **245,449 LOC**: **21.18% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **528,427 additional Mojo LOC** required at this Rust
volume.

## Runtime candidate-selection hard replacement

Optimistic-current admission, availability classification, and ready/fallback
candidate ordering now use the Mojo plan in every feature mode. The feature-off
Rust selector and test oracle were deleted. Mojo also returns the per-candidate
inflight soft-limit decision through the versioned v2 ABI; Rust validates tags
and indices, then maps the plan to profile records. The v1 export remains for
existing ABI consumers. Expected-value tests cover hard rejection precedence,
profile affinity, quota and load signals, and 257/513-candidate ordering. Native
execution evidence is Linux x86_64; macOS and Windows runtime execution was
not run.

The canonical source report counts **52,070 reachable Mojo LOC** and
**193,475 Rust production LOC**, totaling **245,545 LOC**: **21.21% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **528,355 additional Mojo LOC** required at this Rust
volume.

## Runtime proxy preset hard replacement

The runtime policy crate now resolves all proxy presets through the existing
Mojo defaults kernel in every feature mode. The feature-off Rust preset table
was deleted, and the Mojo runtime dependency is mandatory for this policy.
Public preset and override tests cover low, default, many-terminals, and
aggressive settings. Native execution evidence is Linux x86_64; macOS and
Windows runtime execution was not run.

The canonical source report counts **52,072 reachable Mojo LOC** and
**193,473 Rust production LOC**, totaling **245,545 LOC**: **21.21% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **528,347 additional Mojo LOC** required at this Rust
volume.

## Static-context ordering and noise hard replacement

Static item keys, ordering, and volatile metadata classification now use the
Mojo static-item and normalization kernels in every feature mode. The Rust
order-key, bounded-selection, and noise-classifier implementations, including
the temporary parity oracle, were deleted. Expected-value tests cover Unicode
whitespace, malformed metadata, separator precedence, duplicate items,
ordering ties, and a 2 MiB line. Both touched Mojo kernels compiled to objects
for all six release targets. Native execution evidence is Linux x86_64; macOS
and Windows runtime execution was not run.

The canonical source report counts **52,114 reachable Mojo LOC** and
**193,391 Rust production LOC**, totaling **245,505 LOC**: **21.23% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **528,059 additional Mojo LOC** required at this Rust
volume.

## Provider error classifier hard replacement

Provider error class and cooldown decisions now call the versioned Mojo kernel
in every feature mode. The feature-off Rust classifier and temporary parity
oracle were deleted. Rust still parses provider error bodies and selects among
classifications from structured codes. A 1,368-case differential
corpus established parity; permanent expected-value tests cover precedence,
Unicode whitespace, malformed text, and 1 MiB inputs. The touched Mojo kernel
compiled to objects for all six release targets. Native execution evidence is
Linux x86_64; macOS and Windows runtime execution was not run.

The canonical source report counts **52,154 reachable Mojo LOC** and
**193,327 Rust production LOC**, totaling **245,481 LOC**: **21.25% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **527,827 additional Mojo LOC** required at this Rust
volume.

## External catalog dedup hard replacement

External provider launch catalog deduplication now uses the existing Mojo
merge kernel. The feature-off Rust implementation and test oracle were deleted;
feature-off use returns an explicit unsupported error. Expected-value tests
cover Unicode whitespace, IDs longer than 65,536 bytes, first dynamic metadata,
model ordering, CLI precedence, and opaque OS argument preservation. Native
execution evidence is Linux x86_64; macOS and Windows runtime execution was
not run.

The canonical source report counts **52,154 reachable Mojo LOC** and
**193,329 Rust production LOC**, totaling **245,483 LOC**: **21.25% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **527,833 additional Mojo LOC** required at this Rust
volume.

## DeepSeek buffered-response tool-call hard replacement

Buffered DeepSeek tool calls now use the versioned Mojo response kernel for
namespace splitting, tool-search and custom-call shaping, and thought-signature
placement in every feature mode. The Rust feature-off formatter, namespace
splitter, and signature-shaping helper were deleted. Rust retains Serde acquisition,
malformed-JSON diagnostics, and the shared shell-command wrapper. Response
operations have a bounded 16 MiB input limit; oversized input returns a
controlled provider error rather than a Rust fallback or a kernel panic.
Expected-value tests cover Unicode, namespace separators, malformed inputs,
the 4 MiB boundary, oversized arguments, and buffered response propagation.
Native execution evidence is Linux x86_64; macOS and Windows runtime execution
was not run.

The canonical source report counts **52,316 reachable Mojo LOC** and
**193,322 Rust production LOC**, totaling **245,638 LOC**: **21.30% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **527,650 additional Mojo LOC** required at this Rust
volume.

## Smart Context fingerprint-delta hard replacement

The public fingerprint-delta path now uses the existing Mojo plan in every
feature mode. The feature-off Rust map/delta implementation and differential
test oracle were deleted. Rust supplies dense, ordered key and content-hash
IDs to the plan and maps validated indices back to typed fingerprints.
Expected-value tests cover duplicate keys, equal hashes, additions, removals,
changed content, and stable kind/key ordering. The production caller is the
static-context change observation. Native execution evidence is Linux x86_64;
macOS and Windows runtime execution was not run.

The canonical source report counts **52,316 reachable Mojo LOC** and
**193,285 Rust production LOC**, totaling **245,601 LOC**: **21.30% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **527,539 additional Mojo LOC** required at this Rust
volume.

## Gemini buffered-response hard replacement

Buffered Gemini response envelope and message assembly now use a versioned Mojo
kernel in every feature mode. The feature-off Rust assembler and temporary
oracle were deleted. Rust retains Serde acquisition, part/usage/metadata
preparation, and status application. The 16 MiB input ceiling returns a
controlled provider or runtime response error; unexpected kernel errors also
return controlled errors rather than panicking. Expected-value caller tests
cover text, tools, Unicode, metadata, malformed input, and the size boundary.
The updated rich ABI compiled to objects for all six release target triples.
Native execution evidence is Linux x86_64; macOS and Windows runtime execution
was not run.

The canonical source report counts **52,367 reachable Mojo LOC** and
**193,345 Rust production LOC**, totaling **245,712 LOC**: **21.31% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **527,668 additional Mojo LOC** required at this Rust
volume.

## Provider chat-tool hard replacement

The public Responses-to-chat tool bridge now uses its existing Mojo JSON kernel
in every feature mode. The Rust tool-choice, tool-expansion, namespace, web-search,
feature-off, and differential-oracle implementations were deleted. Rust retains
Serde tree acquisition and output materialization. The existing Mojo ABI and
public API remain unchanged. Before deletion, 5,000 seeded differential JSON
trees and large expansion cases established parity. Permanent expected-value
tests cover precedence, Unicode, malformed input, MCP ordering and deduplication,
web-search options, and 1,024-tool expansion. The no-fallback guard rejects
restoration of the removed modules or feature-off routing.

The canonical source report counts **52,367 reachable Mojo LOC** and
**192,874 Rust production LOC**, totaling **245,241 LOC**: **21.35% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **526,255 additional Mojo LOC** required at this Rust
volume.

## Runtime Doctor marker-registry hard replacement

Runtime Doctor marker recognition now uses the existing versioned Mojo
classifier. The Rust marker registry, enum mappings, marker-subset constants,
and test-only descriptors were deleted; the parser's Mojo call adapter remains.
Before deletion, differential checks confirmed all 167 historical marker IDs.
A permanent expected-value caller test covers those IDs, unknown and long
markers, malformed JSON, and invalid UTF-8. The source marker guard now checks
emitted markers against Mojo directly, and the no-fallback guard rejects
restoration of the Rust registry. The removed public Rust marker registry API
had no workspace callers; external source consumers of that API must adapt.

The canonical source report counts **52,367 reachable Mojo LOC** and
**192,819 Rust production LOC**, totaling **245,186 LOC**: **21.36% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **526,090 additional Mojo LOC** required at this Rust
volume.

## Status quota-summary hard replacement

The `prodex status` quota summary now uses the versioned Mojo pool kernel for
per-profile source precedence, availability, remaining totals, and earliest
reset selection. Rust retains report/cache acquisition, existing window
normalization, and typed input/output mapping. The Rust summary accumulator
and temporary differential oracle were deleted. Caller tests cover successful
reports, usable and stale cached snapshots, missing windows, incompatible
profiles, and report ordering. The ownership manifest records the new operation,
and the no-fallback guard rejects restoration of the Rust accumulator.
The updated kernel compiled to objects for all six release targets; native
execution evidence remains Linux x86_64.

The canonical source report counts **52,469 reachable Mojo LOC** and
**192,941 Rust production LOC**, totaling **245,410 LOC**: **21.38% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **526,354 additional Mojo LOC** required at this Rust
volume.

## Anthropic stream feature-off hard replacement

Anthropic Messages SSE translation now calls the existing Mojo stream kernel in
every provider-core feature mode. The feature-off unsupported stub was removed;
the no-fallback guard rejects its return. Caller tests cover stream events,
malformed framing, Unicode tool IDs and deltas, error events, and invalid UTF-8
replacement without the `mojo` feature. No new Mojo source or ABI was needed.

The canonical source report counts **52,469 reachable Mojo LOC** and
**192,934 Rust production LOC**, totaling **245,403 LOC**: **21.38% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **526,333 additional Mojo LOC** required at this Rust
volume.

## DeepSeek request-tool validator hard replacement

DeepSeek tool-shape, tool-choice-shape, and web-search option/context validation
now use the existing Mojo request-policy kernel in every provider-core feature
mode. The feature-off Rust shape validators were deleted. The Mojo policy now
preserves prior nested-name, null-alias, and Unicode-blank behavior. Rust keeps
Serde acquisition, bounded 4 MiB kernel input, and provider error mapping.
Tests cover first-error precedence, Unicode, and the exact input limit. The
no-fallback guard rejects restoration of the removed validator modules.
Strict-schema normalization, function-name and parameter checks, and function
tool deduplication remain separate Rust responsibilities for a later wave.
The existing Mojo ABI and operation IDs did not change.

The canonical source report counts **52,509 reachable Mojo LOC** and
**192,726 Rust production LOC**, totaling **245,235 LOC**: **21.41% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **525,669 additional Mojo LOC** required at this Rust
volume.

## Super expose alias scan hard replacement

The `prodex super|s ... expose` option scan now uses operation 11 on the
versioned Mojo launch-argument ABI. The Rust option table and scanner were
deleted. Rust preserves opaque `OsString` values and reassembles the selected
arguments. Caller tests cover paired and inline option values, the `--`
boundary, and non-UTF-8 arguments on Unix. The ownership record extends the
existing launch-argument authority, and the no-fallback guard rejects a
restored Rust scanner; no shared-entry authority exception was needed.

The canonical source report counts **52,546 reachable Mojo LOC** and
**192,707 Rust production LOC**, totaling **245,253 LOC**: **21.43% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **525,575 additional Mojo LOC** required at this Rust
volume.

## Smart Context rewrite-budget hard replacement

Smart Context telemetry decisions and adaptive rewrite-budget adjustments now
use the existing Mojo kernels in every runtime-proxy feature mode. The Rust
decision, scaling, and feature-off copies were deleted. The budget ABI is v2
so it preserves distinct inline and tool-output limits; exact pass-through
still leaves its rehydration limit unchanged. Caller tests cover safety,
quality-risk, rounding, saturation, and available-context boundaries. The
no-fallback guard now rejects feature-off or Rust-copy restoration in these
adapters. The changed kernel compiled to objects for all six release targets;
native execution evidence remains Linux x86_64.

The canonical source report counts **52,558 reachable Mojo LOC** and
**192,542 Rust production LOC**, totaling **245,100 LOC**: **21.44% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **525,068 additional Mojo LOC** required at this Rust
volume.

## Gemini generation-config hard replacement

Gemini generation configuration, thinking-level selection, request-map fields,
and body envelopes now use the existing Mojo request kernels in every provider
feature mode. The feature-off Rust field mappers and thinking-config copy were
deleted, as were duplicate Rust Mojo adapters. Kernel input and output errors,
including requests over the 4 MiB fragment limit, now reach the caller as
errors instead of panics or silently omitting tool configuration. Public Rust
request helpers return `Result` rather
than a bare value; all workspace callers were updated. Caller tests cover
aliases, null precedence, Unicode, candidate count, and the exact size limit.
The no-fallback guard rejects restoration of these Rust decisions. The changed
kernel compiled to objects for all six release targets; native execution
evidence remains Linux x86_64.

The canonical source report counts **52,579 reachable Mojo LOC** and
**192,373 Rust production LOC**, totaling **244,952 LOC**: **21.47% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **524,540 additional Mojo LOC** required at this Rust
volume.

## DeepSeek request rejection hard replacement

The public unsupported-request-field and beta-completion rejection functions
now use the existing Mojo `RequestFields` and `BetaFields` policies in every
provider-core feature mode. The feature-off Rust policy and its helpers were
deleted with their cfg routing. Rust retains JSON serialization and tagged
error formatting; non-object values keep their prior no-op behavior. The
bounded Mojo adapter returns errors for oversized inputs and kernel failures
instead of panicking. A no-default-features caller test checks exact rejection
messages, provider labels, precedence, malformed field types, non-object
inputs, and the input limit. The no-fallback guard requires both Mojo
operations and rejects restored Rust copies or cfg routing; the Mojo ABI and
operation IDs did not change.

The canonical source report counts **52,579 reachable Mojo LOC** and
**192,370 Rust production LOC**, totaling **244,949 LOC**: **21.47% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **524,531 additional Mojo LOC** required at this Rust
volume.

## Provider error request-member rejection hard replacement

Request-member rejection now uses the versioned Mojo provider-error ABI in
every provider feature mode. Rust retains bounded Serde parsing, lossy UTF-8
fallback, and typed JSON-tree construction; the former Rust normalization,
marker matching, and recursive rejection logic were deleted. Oversized inputs
and ABI errors fail closed without triggering an unsupported-field retry.
Development parity covered 1,024 cases before the Rust oracle was deleted.
Expected-value provider and app caller tests cover structured errors, nested
values, malformed text, and Gemini/web-search fallback behavior. The ownership
manifest and no-fallback guard record Mojo as the sole semantic authority. The
kernel compiled to objects for all six release targets; native execution
evidence remains Linux x86_64.

The canonical source report counts **52,825 reachable Mojo LOC** and
**192,353 Rust production LOC**, totaling **245,178 LOC**: **21.55% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **524,234 additional Mojo LOC** required at this Rust
volume.

## DeepSeek reasoning hard replacement

DeepSeek and Gemini-compatible reasoning-shape validation, effort mapping, and
thinking detection now use the existing Mojo request policy and reasoning
kernel in every provider-core feature mode. The feature-off Rust mapper and
its helper functions were deleted. The Rust adapter retains bounded JSON
serialization, tagged-error formatting, and output application. Valid
non-object JSON remains a no-op. Unrelated payloads over 4 MiB remain valid;
oversized reasoning fields return a controlled error. Caller tests cover exact
mappings, provider labels, Unicode whitespace, malformed fields, and both
size boundaries. The
no-fallback guard rejects restored Rust copies or feature-off routing. The
Mojo ABI did not change; native test evidence is Linux x86_64.

The canonical source report counts **52,810 reachable Mojo LOC** and
**192,361 Rust production LOC**, totaling **245,171 LOC**: **21.54% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **524,273 additional Mojo LOC** required at this Rust
volume.

## Smart Context exactness hard replacement

The exactness decision and ordered reason bits now use the existing Mojo plan
in every runtime-proxy feature mode. The feature-off Rust decision and the
Rust test oracle were deleted. Fixed caller expectations cover each affinity
reason, combined precedence, whitespace-only identifiers, and ignored
rehydration references. The no-fallback guard requires the Mojo call and
rejects a restored Rust copy. The Mojo ABI did not change; native execution
evidence remains Linux x86_64.

The canonical source report counts **52,810 reachable Mojo LOC** and
**192,332 Rust production LOC**, totaling **245,142 LOC**: **21.54% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **524,186 additional Mojo LOC** required at this Rust
volume.

## Runtime quota snapshot and precommit gate hard replacement

Quota window status, snapshot reset/hold/usability, blocking reset timing, and
precommit gate decisions now use the existing Mojo plans in every runtime-proxy
feature mode. The feature-off Rust decisions and the 265-line Rust test oracle
were deleted.
Fixed caller expectations cover exhausted snapshots before and after reset,
unknown-window reprobes, and source-sensitive gate behavior. The no-fallback
guard rejects a restored oracle or feature-off path. The Mojo ABI did not
change; native execution evidence remains Linux x86_64.

The canonical source report counts **52,810 reachable Mojo LOC** and
**192,293 Rust production LOC**, totaling **245,103 LOC**: **21.55% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **524,069 additional Mojo LOC** required at this Rust
volume.

## Smart Context adaptive budget feature consistency

The adaptive budget policy now calls the existing Mojo plan in every
runtime-proxy feature mode. The feature-off `None` path and its unavailable
test were replaced with a fixed caller expectation for an unknown token
window. The no-fallback guard requires the Mojo call and rejects feature-off
decision routing. The Mojo ABI did not change; native execution evidence
remains Linux x86_64.

The canonical source report counts **52,810 reachable Mojo LOC** and
**192,291 Rust production LOC**, totaling **245,101 LOC**: **21.55% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **524,063 additional Mojo LOC** required at this Rust
volume.

## DeepSeek simple-request eligibility hard replacement

DeepSeek simple-request eligibility now uses the existing Mojo request policy
in every provider-core feature mode. The feature-off Rust classifier and input
item helper were deleted. Rust parses and normalizes JSON, checks existing
response bindings, and maps the typed ABI result. Fixed caller cases cover
supported inputs, tools, aliases, escaped fields, malformed JSON, Unicode,
callback behavior, and the 16 MiB policy bound. The no-fallback guard requires
the Mojo call and rejects a restored Rust classifier. The Mojo ABI did not
change; native execution evidence remains Linux x86_64.

The canonical source report counts **52,820 reachable Mojo LOC** and
**192,289 Rust production LOC**, totaling **245,109 LOC**: **21.55% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **524,047 additional Mojo LOC** required at this Rust
volume.

## DeepSeek metadata composition hard replacement

Response-format output and request metadata composition now call the existing
Mojo kernel in every provider-core feature mode. The feature-off Rust map
mutation and note construction were deleted. The Mojo adapter preserves an
explicit empty provider metadata bucket and leaves non-object existing
metadata unchanged when adding a thinking-mode tool-choice note. Fixed caller
tests cover those boundaries alongside degraded response-format metadata. A
bounded ABI failure retains the prior metadata rather than panicking.
The no-fallback guard requires both Mojo operations and rejects feature-off
routing. The Mojo ABI did not change; native execution evidence remains Linux
x86_64.

The canonical source report counts **52,820 reachable Mojo LOC** and
**192,285 Rust production LOC**, totaling **245,105 LOC**: **21.55% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **524,035 additional Mojo LOC** required at this Rust
volume.

## DeepSeek strict schema normalization hard replacement

Strict function-tool schema normalization now uses the existing Mojo writer
in every provider-core feature mode. The feature-off Rust recursive sanitizer
and object helper were deleted. Rust retains caller-facing validation errors
until a typed Mojo diagnostic can replace that separate responsibility. Fixed
caller expectations cover nested objects, arrays, `anyOf`, and nested error
paths. The Mojo writer now accepts both empty and populated `anyOf` arrays;
the previous closing-bracket condition rejected both. The no-fallback guard
requires Mojo normalization and rejects restored Rust sanitizer functions.
The authority guard distinguishes operations sharing one exported kernel by
operation ID and still rejects duplicate IDs.
The Mojo ABI did not change; native execution evidence remains Linux x86_64.

The canonical source report counts **52,820 reachable Mojo LOC** and
**192,190 Rust production LOC**, totaling **245,010 LOC**: **21.56% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **523,750 additional Mojo LOC** required at this Rust
volume.

## Anthropic web-search result and live-item hard replacement

Anthropic web-search result sources, last-matching-call updates, and live
search-call items now use the existing Mojo request kernel. The Rust source
filter and live JSON builder were deleted. Prodex-app calls the provider-core
adapter so the live path remains available without the app's optional
`mojo-core` feature; provider-core uses Mojo in both feature modes. Fixed
caller tests cover malformed source entries, repeated call IDs, incomplete
stream input, and exact live item shapes. Kernel errors become controlled
stream failures. The rich ABI version is unchanged; native execution evidence
remains Linux x86_64.

The canonical source report counts **53,106 reachable Mojo LOC** and
**192,222 Rust production LOC**, totaling **245,328 LOC**: **21.65% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **523,560 additional Mojo LOC** required at this Rust
volume.

## Gemini system-instruction request hard replacement

Gemini Responses system instructions now use the existing provider-constraints
Mojo kernel to extract and join system and contextual-user text and build the
Gemini `systemInstruction` value in every feature mode. Rust serializes the
request and maps the bounded kernel result. Fixed caller and kernel tests cover
mixed text, empty and malformed shapes, and the feature-off route. The ABI
version is unchanged; native execution evidence remains Linux x86_64.

The canonical source report counts **53,528 reachable Mojo LOC** and
**192,206 Rust production LOC**, totaling **245,734 LOC**: **21.78% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **523,090 additional Mojo LOC** required at this Rust
volume.

## Kiro chat-response body hard replacement

Kiro Responses-to-Chat Completions body mapping now uses the existing Mojo
response owner in every provider feature mode. The Rust mapper and feature-off
copy were removed. Rust retains bounded serialization, ABI invocation, and
typed error handling; the local Kiro route now propagates rewrite failures
without panicking. Fixed boundary tests cover complete output, refusals,
first-message behavior, and response size limits. Streaming, Anthropic
projection, and separate response helpers remain outside this checkpoint.
The rich ABI remains version 6.

The canonical source report counts **53,535 reachable Mojo LOC** and
**192,269 Rust production LOC**, totaling **245,804 LOC**: **21.78% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **523,272 additional Mojo LOC** required at this Rust
volume.

## Anthropic response identifier fallback hard replacement

The existing Anthropic response-envelope Mojo operation now chooses the
default response ID and model when the upstream fields are missing or are not
strings. Rust passes the optional serialized fields without applying a second
fallback decision. Fixed Mojo and provider caller tests cover missing and
non-string fields, while the existing string path remains unchanged. The rich
ABI version is unchanged; native execution evidence remains Linux x86_64.

The canonical source report counts **53,549 reachable Mojo LOC** and
**192,257 Rust production LOC**, totaling **245,806 LOC**: **21.79% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **523,222 additional Mojo LOC** required at this Rust
volume.

## Gemini request text-part hard replacement

Gemini Responses text parts now use the existing Mojo request-content operation
in every provider feature mode. Rust retains request traversal and typed error
propagation. A text part beyond the 4 MiB Mojo boundary now returns a
controlled request error instead of panicking; fixed caller tests cover wire
shape and the oversized boundary. The existing ABI version is unchanged;
native execution evidence remains Linux x86_64.

The canonical source report counts **53,549 reachable Mojo LOC** and
**192,284 Rust production LOC**, totaling **245,833 LOC**: **21.78% Mojo**.
The 7% release floor and non-regression check pass; the 75% project target
remains unmet, with **523,303 additional Mojo LOC** required at this Rust
volume.

## Kiro response helper hard replacement

Kiro model-list and not-found shapes, invalid-request and unsupported-path
errors, Chat Completions finish-reason selection, Anthropic tool-use blocks,
Anthropic stop-reason mapping, and Anthropic message assembly now use the
existing Kiro Mojo kernel in every provider feature mode. The former
feature-off Rust copies and helper functions were deleted. Rust retains model
catalog lookup, bounded JSON serialization/decoding, runtime metadata
attachment, and exact raw-value preservation for malformed JSON field types
that the current string-valued ABI cannot carry without normalization. The
no-fallback guard now treats the response adapter as unconditional Mojo and
requires all seven promoted Kiro operations.

Focused validation passes 40 Kiro provider tests in the default feature mode
and the same 40 tests with `--features mojo`. The canonical source report
counts **53,551 reachable Mojo LOC** and **192,259 Rust production LOC**,
totaling **245,810 LOC**: **21.785525% Mojo**. The 7% release floor and
non-regression requirement remain satisfied; the 75% project target remains
unmet, with **523,226 additional Mojo LOC** required at this Rust volume.

## Kiro ACP shape hard replacement

The twelve existing Kiro ACP request/response, assistant, plan, error, session,
metadata, and incomplete-details shape operations now run through the Kiro Mojo
kernel in every provider feature mode. The duplicate feature-off Rust JSON
builders and metadata truncation copy were deleted; Rust retains JSON
serialization/decoding at the ABI plus the separate stop-reason extraction and
incomplete-reason classification helpers that do not yet have Mojo operations.
The no-fallback guard treats the ACP adapter as unconditional Mojo and requires
operations 32 through 43.

Focused ACP validation passes 13 tests in the default provider feature mode and
the same 13 tests with --features mojo. The canonical source report counts
**53,551 reachable Mojo LOC** and **192,235 Rust production LOC**, totaling
**245,786 LOC**: **21.787653% Mojo**. The 7% release floor and non-regression
requirement remain satisfied; the 75% project target remains unmet, with
**523,154 additional Mojo LOC** required at this Rust volume.

## DeepSeek stream shape hard replacement

DeepSeek stream tool-call/chunk/choice projections, stream response metadata,
stream response envelope and assistant-message shaping, and buffered response
metadata wrapping now use the existing DeepSeek Mojo kernel in every provider
feature mode. The feature-off Rust copies were deleted from the three stream
and response adapters. Rust remains responsible for JSON deserialization,
provider usage conversion, tool-call serialization, validation, and collecting
fields before invoking the kernel. The no-fallback guard now makes these files
unconditional and requires operations 16, 17, 26, and 32 through 36.

The full DeepSeek provider-core suite passes 43 tests in the default feature
mode and the same 43 tests with --features mojo. The canonical source report
counts **53,551 reachable Mojo LOC** and **192,219 Rust production LOC**,
totaling **245,770 LOC**: **21.789071% Mojo**. The release floor and ownership
non-regression checks pass; the 75% project target remains unmet, with
**523,106 additional Mojo LOC** required at this Rust volume.

## Gemini translator and request-content hard replacement

Gemini translator validation, text-content projection, raw translator request
assembly, generic content values, function-call parts, and function-response
parts now use the existing Gemini Mojo request kernels in every provider
feature mode. The feature-off Rust local-media scan, candidate/body orchestration
copy, direct request serialization, and three JSON content builders were
deleted. The tool-history compatibility path remains in Rust only when the
TextContents kernel intentionally declines non-text histories. The now-unused
Rust response_format mapper file was removed as dead production source.

The Gemini translator suite passes 43 tests in the default feature mode and 44
tests with --features mojo. The canonical source report counts **53,551
reachable Mojo LOC** and **192,116 Rust production LOC**, totaling **245,667
LOC**: **21.798207% Mojo**. The release floor and ownership non-regression
checks pass; the 75% project target remains unmet, with **522,797 additional
Mojo LOC** required at this Rust volume.

## Gemini bridge adapter fallback removal

Gemini NativeProject, RequestBodyWithoutTool, SimpleRequest, and
ValidateCandidateCount now dispatch through the shared bridge request helpers
in every feature mode. The duplicate direct ABI calls and the translator-level
candidate-count/request-body fallback helpers and re-exports were deleted.
Nine wrapper modules involved in this chain are now feature-independent and are
covered by the unconditional no-fallback guard.

The Gemini translator suite passes 43 tests in the default feature mode and 44
tests with --features mojo. The canonical source report counts **53,551
reachable Mojo LOC** and **192,108 Rust production LOC**, totaling **245,659
LOC**: **21.798916% Mojo**. Release-floor and ownership checks pass; the 75%
project target remains unmet, with **522,773 additional Mojo LOC** required at
this Rust volume.

## Provider response feature-off fallback hard replacement

Gemini response-part planning now always calls `gemini_sse_state.mojo`, and
Gemini citation/web-search grounding shapes always call the existing Gemini
response kernel operations 27 and 28. The Rust response-part oracle and the
feature-off citation/web-search JSON builders were deleted. OpenAI Chat
Completions response and SSE translation now call `openai_chat_response.mojo`
in every provider-core feature mode; feature-off unsupported branches were
deleted. Anthropic Messages request/response translation and web-search result
handling likewise use the existing Mojo request/response owners unconditionally,
and their feature-off unsupported copies were removed. The no-fallback guard
now treats these six caller files as unconditional Mojo surfaces.

The complete provider-core suite passes 332 tests in the default feature mode
and 333 tests with `--features mojo`, plus seven integration tests in each mode.
The no-fallback, ownership, authority, and broad production-share checks pass.
The canonical source report counts **53,551 reachable Mojo LOC** and **192,038
Rust production LOC**, totaling **245,589 LOC**: **21.81% Mojo**. The 75%
project target remains unmet, with **522,563 additional Mojo LOC** required at
this Rust volume.

## Smart Context, critical-signal, and quota feature-off hard replacement

Smart Context observed accounting, calibration, usage summaries, pressure
snapshots, and rehydration adapters now call their existing Mojo owners in
every `prodex-runtime-proxy` feature mode. The feature-off `None` accounting
paths were deleted, and the two runtime-proxy Mojo adapters they depend on are
now always compiled. Critical-signal counting, diffing, and lost-range planning
are likewise Mojo-backed in every `prodex-context` feature mode; the Mojo core
dependency is no longer optional, `critical_signal_available()` is always true,
and the empty feature-off behavior was deleted. OpenAI model-capacity planning
now uses the real Mojo-backed quota admission classification even with
`prodex-quota --no-default-features`, replacing the hard-coded feature-off
`regular_blocked = true` value. The quota window admission adapter is therefore
also always compiled.

Focused validation passes in both feature modes: `prodex-context` 3/3 default
and 3/3 with `--features mojo`; `prodex-quota` 80/80 default and 53/53 with
`--no-default-features`; Smart Context tests 68/68 in default runtime-proxy and
100/100 with `--features mojo`. No-fallback, ownership, authority, and broad
production-share checks pass. The canonical broad source report now counts
**53,551 reachable Mojo LOC** and **192,027 Rust production LOC**, totaling
**245,578 LOC**: **21.81% Mojo**. The 75% project target remains unmet, with
**522,530 additional Mojo LOC** required at this Rust volume.

## Prodex app feature-off fallback hard replacement

`prodex-app` now carries `prodex_mojo_core` as an unconditional dependency and
uses the existing Mojo owners even under `--no-default-features`. Rust feature-off
semantic copies were removed for operational event source/detail planning, ping
JSONL validation and failure classification, dynamic main-model catalog planning,
main model/effort resolution, Super Expose route/tool/tunnel policy, Gemini SSE
tool-call indexing, external catalog index planning, and the DeepSeek native
web-search capability gate. The obsolete log-summary Rust oracle was deleted.
Doctor now reports the compiled Mojo runtime instead of a synthetic feature-off
Rust implementation. The no-fallback guard promotes these app surfaces so a
`not(feature = "mojo-core")` semantic branch cannot be restored.

Focused validation passes `prodex-app` checks with default features and with
`--no-default-features`. No-default focused suites pass for ping (28), log stream
(4), main-model prompt/catalog (10), Super Expose (78), external provider catalog
(12), Gemini SSE (53), DeepSeek local rewrite (18), and doctor (26, serialized).
Default focused suites pass for ping (28), log stream (4), main-model prompt/catalog
(10), and Super Expose (78). No-fallback, ownership, authority, formatting, and
broad production-share checks pass. The canonical source report counts **53,551
reachable Mojo LOC** and **191,477 Rust production LOC**, totaling **245,028 LOC**:
**21.86% Mojo**. The 75% project target remains unmet, with **520,880 additional
Mojo LOC** required at this Rust volume.

## CLI runtime-feature and launch feature-off hard replacement

Codex runtime-feature configuration now invokes the existing Mojo planner in every
`prodex-cli` feature mode. The feature-off request detector/error path and all
`mojo-core` compilation gates around the adapter and renderer were deleted, while
the public compatibility feature remains harmless. `prodex-runtime-launch` now
links `prodex_mojo_core` unconditionally, so `--no-default-features` executes the
same Mojo launch planner instead of failing compilation; the Mojo launch tests are
therefore compiled in both modes. The no-fallback guard now treats the CLI runtime
feature adapter and runtime-launch root as unconditional Mojo surfaces.

Focused validation passes 6/6 runtime-feature tests in both default and no-default
`prodex-cli`, and 64/64 `prodex-runtime-launch` library tests in both modes. The
no-fallback guard and no-default compile checks pass. The canonical source report
counts **53,551 reachable Mojo LOC** and **191,476 Rust production LOC**, totaling
**245,027 LOC**: **21.86% Mojo**. The 75% project target remains unmet, with
**520,877 additional Mojo LOC** required at this Rust volume.

## Governance classification activation and Rust-oracle retirement

Governance inspection finding classification now reaches the existing
`governance_inspection.mojo` kernel from `prodex-domain`. Finding-kind minimum
classification and the batch classification-floor check are Mojo-authoritative;
the duplicate Rust classification table and per-finding comparison loop were
deleted. The domain keeps typed enums, bounded finding storage, sorting, exact
model errors, and validated ABI tag reconstruction. Fixed caller tests cover all
twelve finding kinds plus weak-classification rejection.

The obsolete pre-Mojo quota score/band/scheduler and profile-order Rust test
oracles were also deleted after the production Mojo schedulers had independent
caller-boundary coverage. Likewise, generated signal-diff and rich catalog
configuration Rust oracles inside `prodex-mojo-core` were removed; fixed-value
Mojo tests remain. The no-fallback guard rejects restoration of the governance
Rust mapping and quota `_rust` scheduler/score/band implementations.

Focused validation passes the complete `prodex-domain` suite, 27/27
`prodex-runtime-quota` tests, the Mojo signal-diff self-test, the fixed rich
catalog configuration test, and governance inspection 4/4. Ownership reports
**46 authoritative operations**. The canonical broad source report counts
**53,551 reachable Mojo LOC** and **191,222 Rust production LOC**, totaling
**244,773 LOC**: **21.88% Mojo**. The 75% project target remains unmet, with
**520,115 additional Mojo LOC** required at this Rust volume.

## Profile identity planning migration

Profile identity metadata planning now uses a dedicated reachable Mojo kernel in
`mojo/prodex_core/profile_identity.mojo`. The migrated owner covers identity
matching, Unicode-aware email/account normalization, canonical identity keys,
profile-name derivation and validation, add/remove source policy, activation,
and managed/external home-deletion decisions. Rust retains JWT/base64 token
parsing, zeroized secret ownership, caller callbacks for unique-name availability,
collection reconstruction, and exact human-readable error mapping; no token or
secret payload crosses the new ABI.

The caller suite passes 22/22 tests, including Unicode White_Space trimming,
ASCII-only case folding compatibility, ambiguous legacy identity matching, exact
profile-name validation precedence, add-source conflicts, bulk-removal protection,
and home-deletion policy. The Mojo ABI has a direct smoke test, the no-fallback
guard requires the promoted caller functions to retain Mojo dispatch, and ownership
now reports **47 authoritative operations**. The canonical broad source report
counts **54,155 reachable Mojo LOC** and **191,548 Rust production LOC**, totaling
**245,703 LOC**: **22.04% Mojo**. The 75% project target remains unmet, with
**520,489 additional Mojo LOC** required at this Rust volume.

## Quota model policy migration

Quota model policy now extends the existing reachable `quota.mojo` owner rather
than creating a parallel module. Mojo owns quota auth-filter parsing and matching,
report-sort cycling, plan-capacity pressure scaling, and signed saturating pressure
scaling. Rust retains provider payload DTOs, serde/credential zeroization boundaries,
static display labels, and typed enum/tag mapping. No feature-off or recomputation
fallback remains in the promoted model-policy functions.

The complete `prodex-quota` suite passes **81/81** tests, including new Unicode
trim/alias, signed saturation, filter-case and sort-cycle boundaries; the direct
Mojo-core policy test also passes and focused clippy is warning-free. No-fallback
plus its self-test, ownership, and authority guards pass. Ownership reports **48
authoritative operations**. The canonical broad source report counts **54,391
reachable Mojo LOC** and **191,678 Rust production LOC**, totaling **246,069 LOC**:
**22.10% Mojo**. The 75% project target remains unmet, with **520,643 additional
Mojo LOC** required at this Rust volume.

## Redaction semantic migration

Sensitive-key classification, secret-like plain-text rewriting, and gateway
privacy redaction now use a dedicated reachable Mojo owner in
`mojo/prodex_core/redaction.mojo`. The former Rust implementations for normalized
secret-key matching, quoted/unquoted sensitive field handling, authorization
credentials, prefixed API keys, email-token masking, canonical UUID preservation,
and 13-19 digit group masking were deleted. Rust retains JSON-tree traversal, OS
argument/environment presentation, caller-owned bounded ABI buffers, UTF-8 result
mapping, and fail-closed replacement if the Mojo call fails.

Validation passes 10/10 `prodex-redaction` caller tests and the direct Mojo ABI
smoke test. Boundary fixtures cover cookies, BASIC/Bearer/Token credentials,
quoted fields, case-insensitive API-key prefixes, Unicode passthrough, large
non-secret inputs, email-shape boundaries, UUID preservation, and card-like digit
groups. The no-fallback and authority guards pass, and ownership now reports
**51 authoritative operations**. The canonical broad source report counts
**55,161 reachable Mojo LOC** and **191,404 Rust production LOC**, totaling
**246,565 LOC**: **22.37% Mojo**. The 75% project target remains unmet, with
**519,051 additional Mojo LOC** required at this Rust volume.

## Runtime state background policy migration

Runtime state persistence classification and background queue backpressure now use
a dedicated Mojo owner in `runtime_state_background.mojo`. The migrated policy
covers all 32 `RuntimeStateMutation` variants for save sections, continuation-
journal persistence, hot-continuation debounce, combined schedule planning, queue
threshold selection, saturating enqueue backlog, and queue-pressure decisions.
Rust retains the typed mutation/queue-kind tag adapter plus `Duration`, `Instant`,
`Mutex`, `Condvar`, atomic counters, job extraction, and worker-loop system
boundaries.

The complete `prodex-runtime-state` suite passes 18/18 tests, including an
exhaustive fixed-value matrix for every mutation variant and `usize::MAX` queue
boundaries; the direct Mojo ABI smoke test also passes. No-fallback, ownership,
and authority guards pass with **52 authoritative operations**. The canonical
broad source report counts **55,346 reachable Mojo LOC** and **191,506 Rust
production LOC**, totaling **246,852 LOC**: **22.42% Mojo**. The 75% project
target remains unmet, with **519,172 additional Mojo LOC** required at this Rust
volume.

## Runtime quota selection policy hard replacement

Quota-window usability, precommit-floor/guard decisions, quota pressure-band
reason selection, and soft-affinity quota admission/rejection now use the new
`prodex_runtime_quota_selection_policy_v1` operation in the existing
`candidate_decision.mojo` owner. The duplicate Rust branch logic in
`selection_policy.rs` was collapsed to typed route/status/band tag conversion
and stable reason-label reconstruction; the existing soft-affinity operation
reuses the same tag adapters.

Focused selection-policy validation passes 16/16 caller tests, including the
advisory positive-floor contract, exhausted weekly/five-hour windows, missing
quota sources, and all affinity routes. A direct Mojo ABI test covers response
and compact floors, critical-but-usable quota, exhausted windows, and unknown
quota. No-fallback, ownership, and authority guards pass with **53 authoritative
operations**. The canonical broad source report counts **55,427 reachable Mojo
LOC** and **191,610 Rust production LOC**, totaling **247,037 LOC**: **22.44%
Mojo**. The 75% project target remains unmet, with **519,403 additional Mojo
LOC** required at this Rust volume.

## State core policy and Codex config argument migration

prodex-state now delegates its deterministic core policy to
mojo/prodex_core/state_policy.mojo: provider route/quota capability selection
and pool priority, hard-binding conflict/winner policy, active-profile merge
precedence, last-run retention, and binding retention. Rust retains profile and
identity lookups, BTreeMap traversal, cloning, size pruning, filesystem-backed
state ownership, and validated enum/result reconstruction.

prodex-codex-config now delegates profile-v2 argument selection, -c/--config
override scanning (including last-value precedence and -- cutoff), profile-name
validation, and provider-value Unicode trim/quote normalization to
mojo/prodex_core/codex_config.mojo. Rust retains OsString ownership,
non-UTF-8 opacity, bounded config-file I/O, TOML parsing, profile path
construction, and source attribution.

Focused validation passes 11/11 prodex-state tests, 19/19
prodex-codex-config tests, and direct Mojo ABI smoke tests for both kernels.
The no-fallback self-test, ownership, and authority guards pass; ownership now
reports **57 authoritative operations**. The canonical broad source report counts
**55,946 reachable Mojo LOC** and **191,882 Rust production LOC**, totaling
**247,828 LOC**: **22.57% Mojo**. The 75% project target remains unmet, with
**519,700 additional Mojo LOC** required at this Rust volume.

## Runtime broker continuity migration

Runtime broker continuity semantics now use
mojo/prodex_core/runtime_broker_continuity.mojo. The migrated owner covers
continuity event/reason planning for text and JSON-derived log messages,
health-score decay, stale-verified continuation classification, known-route
classification, and route/internal/profile health-key classification. The custom
Rust JSON-string parser, log-field scanner, event classifier, timestamp-max
staleness helper, and score-decay arithmetic were deleted. Rust retains generic
serde_json tree decoding, map aggregation, caller-owned string slicing, metric DTO
construction, and process/registry boundaries.

Validation passes 32/32 prodex-runtime-broker tests and the direct Mojo ABI smoke
test. The no-fallback self-test and ownership guard pass; ownership now reports
**62 authoritative operations**. The canonical broad source report counts
**56,262 reachable Mojo LOC** and **191,977 Rust production LOC**, totaling
**248,239 LOC**: **22.66% Mojo**. The 75% project target remains unmet, with
**519,669 additional Mojo LOC** required at this Rust volume.

## Runtime proxy request metadata and path policy migration

Runtime proxy request metadata and path-routing semantics now use
mojo/prodex_core/runtime_proxy_request.mojo. The request metadata owner covers
previous_response_id, session/prompt-cache/turn/thread/window identifiers,
top-level versus client_metadata precedence, tool-output affinity, fresh-fallback
shape, and full-history reconstructability. The path owner covers Prodex/OpenAI
mount normalization (including legacy version segments), responses/chat/compact
classification, realtime/live path classification, route-lane selection, and
long-lived request policy. The former Rust JSON decision loops and path/version
classification helpers were deleted.

Rust retains serde_json parsing/tree adaptation, header precedence, request body
mutation/serialization, borrowed suffix/query reconstruction, and typed enum/String
result mapping. Validation passes the complete 348/348 prodex-runtime-proxy suite,
including mount normalization, realtime paths, lane classification, metadata
precedence, affinity/fallback shape, and full-history recovery. No-fallback
self-test, ownership, and authority guards pass; ownership reports **64
authoritative operations**. The canonical broad source report counts **56,677
reachable Mojo LOC** and **192,079 Rust production LOC**, totaling **248,756 LOC**:
**22.78% Mojo**. The 75% project target remains unmet, with **519,560 additional
Mojo LOC** required at this Rust volume.

## Runtime state quota snapshot policy migration

Runtime-state quota timing and snapshot policy now uses
mojo/prodex_core/runtime_state_quota.mojo. The migrated owner covers timestamp
touch persistence thresholds, probe-cache freshness, exhausted-window hold
classification and snapshot usability, plus probe-apply blocking-reset, quarantine,
and retry-backoff arithmetic. Generic snapshot equality/status projection and
candidate collection traversal remain Rust adapters.

Validation passes 19/19 prodex-runtime-state tests and the direct Mojo ABI smoke
test. No-fallback self-test, ownership, and authority guards pass; ownership now
reports **65 authoritative operations**. The canonical broad source report counts
**56,799 reachable Mojo LOC** and **192,288 Rust production LOC**, totaling
**249,087 LOC**: **22.80% Mojo**. The 75% project target remains unmet, with
**520,065 additional Mojo LOC** required at this Rust volume.

## Gemini translator tool-assembly consolidation

Gemini request translation now removes the remaining per-tool Rust assembly loop.
Custom, namespace, MCP, tool-search, and standard function declarations are normalized
through the existing chat-tools Mojo transform once per request; the Gemini raw
translator kernel then merges those function tools with built-in computer, code, web,
and URL tools directly in `provider_constraints.mojo`. The retired Rust
`gemini_apply_tools`, `gemini_tool_from_openai_tool`, and generic `sanitize_schema`
helpers were deleted. Rust retains JSON parsing, stable rejection-message mapping,
Serde materialization, and the chat-tools/Gemini kernel call boundaries.

Validation passes 70/70 focused Gemini tests in the default provider build and
71/71 with the Mojo feature enabled, including custom tools, duplicate/order and
Unicode schema arrays, built-ins, malformed tool precedence, and tool-choice behavior.
Clippy, no-fallback self-test, ownership, and authority guards pass; ownership remains
**65 authoritative operations** because this extends the existing
`gemini_translator_request_planning` ABI owner. The canonical broad source report
counts **56,853 reachable Mojo LOC** and **192,203 Rust production LOC**, totaling
**249,056 LOC**: **22.83% Mojo**. The 75% project target remains unmet, with
**519,756 additional Mojo LOC** required at this Rust volume.

## Profile export policy-limit migration

Profile-export count, size, password-length, PBKDF2 range, and Argon2 numeric
parameter policies now use mojo/prodex_core/profile_export_policy.mojo.
The ABI receives only profile/secret-file counts, byte lengths, and numeric KDF
metadata. Password bytes, auth JSON, secret-file contents, salt, nonce,
ciphertext, zeroization, KDF execution, and AES-GCM-SIV crypto remain Rust-owned.

Validation passes 56/56 prodex-profile-export tests plus the direct Mojo ABI
smoke test. Coverage includes exact count/size/KDF boundaries,
fail-before-password-lookup behavior, encrypted v1/v2 round trips, corrupted
ciphertext, known-answer compatibility, and redacted secret failures. Clippy,
prodex-app check, no-fallback self-test, ownership, and authority guards pass;
ownership now reports **66 authoritative operations**. The canonical broad source
report counts **56,950 reachable Mojo LOC** and **192,349 Rust production LOC**,
totaling **249,299 LOC**: **22.84% Mojo**. The 75% project target remains unmet,
with **520,097 additional Mojo LOC** required at this Rust volume.

## Smart-context repo-map semantic migration

Repo-map module-like declaration classification and source-path to module-name
normalization now use mojo/prodex_core/runtime_repo_map.mojo. Rust no longer
owns visibility/export/async declaration token parsing or a/b/./src/mod/lib/main/index
path normalization. Nearest-path selection, DTO aggregation, and bounded symbol
values remain Rust adapters.

Direct ABI fixtures cover comments/decorators, Rust visibility prefixes, JavaScript
export/class syntax, async functions, Unicode whitespace, diff prefixes, quotes,
Windows separators, src/mod/lib/main/index paths, and dotted-path behavior. Clippy,
prodex-app check, no-fallback self-test, ownership, and authority guards pass;
ownership now reports **68 authoritative operations**. The canonical broad source
report counts **57,218 reachable Mojo LOC** and **192,376 Rust production LOC**,
totaling **249,594 LOC**: **22.92% Mojo**. The 75% project target remains unmet,
with **519,910 additional Mojo LOC** required at this Rust volume.

## Runtime lineage semantic migration

Runtime hard-binding lineage validation, compact lineage key construction,
response/turn-state lineage encoding, prefix classification, and length-prefixed
decoding now use mojo/prodex_core/runtime_lineage.mojo through the safe
prodex_mojo_core::runtime_lineage adapter. Runtime-state retains DTO/Serde
ownership, normalized optional-string materialization, public owner/resolution
enums, and borrowed-slice reconstruction only; no Rust fallback semantic copy is
retained.

Validation passes 29/29 prodex-runtime-store tests, including lineage round trips
with colon-bearing response IDs, oversized identities, legacy empty fields,
conflict/unavailable resolution, and continuation compaction. Clippy, prodex-app
check, no-fallback self-test, ownership, and authority guards pass; ownership now
reports **71 authoritative operations**. The canonical broad source report counts
**57,471 reachable Mojo LOC** and **192,482 Rust production LOC**, totaling
**249,953 LOC**: **22.99% Mojo**. The 75% project target remains unmet, with
**519,975 additional Mojo LOC** required at this Rust volume.

## Session report metadata migration

Fixed JSON path precedence for session resume IDs, turn-context model/effort,
thread names, cwd/workdir, string timestamps, subagent parent thread IDs, and
model providers now uses mojo/prodex_core/session_report.mojo through the typed
prodex_mojo_core::json tree adapter. Rust retains serde_json decoding, numeric
timestamp fallback, chrono formatting, PathBuf/canonicalization, file discovery,
and report DTO mutation.

Validation passes 52/52 prodex-session-store tests, including blank type handling,
decoded Unicode whitespace fallback, nested metadata precedence, subagent parents,
latest turn settings, repair flows, large/compressed rollouts, and state-db paths.
Clippy, prodex-app check, no-fallback self-test, ownership, and authority guards
pass; ownership now reports **72 authoritative operations**. The canonical broad
source report counts **57,680 reachable Mojo LOC** and **192,548 Rust production
LOC**, totaling **250,228 LOC**: **23.05% Mojo**. The 75% project target remains
unmet, with **519,964 additional Mojo LOC** required at this Rust volume.

## Smart-context artifact-reference parser migration

Smart-context artifact reference validation, psc/psc2/prodex-artifact
normalization, alias validation/declaration/reference parsing, punctuation trimming,
and #/:/? line-range parsing now use
mojo/prodex_core/smart_context_artifact_ref.mojo through the safe
prodex_mojo_core adapter. Rust retains text tokenization, JSON traversal, alias-map
aggregation, DTO mapping, and marker/string materialization; the former Rust parser
helpers were deleted without a fallback branch.

Validation passes the direct ABI smoke test plus 7/7 smart-context rehydrate/dedupe
fixtures and explicit line-range preparation coverage. Extra ABI fixtures cover
short/long canonical IDs, quoted alias declarations, punctuation trimming, multiple
line ranges, invalid IDs, and alias references. Clippy, prodex-app check, no-fallback
self-test, ownership, and authority guards pass; ownership now reports
**73 authoritative operations**. The canonical broad source report counts
**57,977 reachable Mojo LOC** and **192,640 Rust production LOC**, totaling
**250,617 LOC**: **23.13% Mojo**. The 75% project target remains unmet, with
**519,943 additional Mojo LOC** required at this Rust volume.

## Smart-context semantic marker migration

Smart-context semantic marker parsing now uses
`mojo/prodex_core/smart_context_markers.mojo` for file-location and file-path
classification, diff path/span parsing, test-failure and test-symbol detection,
error-code extraction, and per-line command-kind classification. Rust retains
line/token iteration, diff-hunk end scanning, command-kind aggregation precedence,
DTO/string materialization, and semantic-range storage; the former branch parser
implementation in `semantic_index/markers.rs` was deleted without a fallback.

Validation passes the direct ABI smoke test, the semantic-line-index fixture, and
the bounded semantic-index fixture. Edge coverage includes Unicode whitespace
around exit/status codes, diff path quoting, file locations, bracketed compiler
errors, test symbols, and Python/cargo/npm command markers. Clippy, `prodex-app`
check, no-fallback self-test, ownership, and authority guards pass; ownership now
reports **74 authoritative operations**. The canonical broad source report counts
**58,441 reachable Mojo LOC** and **192,706 Rust production LOC**, totaling
**251,147 LOC**: **23.27% Mojo**. The 75% project target remains unmet, with
**519,677 additional Mojo LOC** required at this Rust volume.

## Runtime hard-binding remember policy consolidation

Runtime hard-binding remember conflict, identity-addition, refresh, and max-bound-time
decisions now reuse the existing Mojo state_core_policy binding merge planner.
Rust still owns BTreeMap mutation, persistence scheduling, logging, and binding
identity cloning; the previous nested Rust decision table was removed without adding
a fallback path.

Validation passes 8 affinity-persistence tests and 24 continuation-cleanup tests,
plus clippy, no-fallback self-test, ownership, and authority guards. This extends
the existing state-policy authority rather than creating a duplicate operation.
The current broad source report remains **58,441 Mojo LOC** and **192,708 Rust
production LOC**: **23.27% Mojo**.

## Runtime tuning feature-gate hard replacement

prodex-runtime-tuning now requires the Mojo runtime in every feature mode. The
previous optional prodex_mojo_core dependency and cfg(feature = mojo) API gating
were removed; the legacy mojo feature remains a no-op compatibility feature only.
No Rust arithmetic fallback was introduced.

Validation passes 8/8 tests with default features and the same 8/8 tests under
--no-default-features, plus no-default clippy, prodex-app check, no-fallback
self-test, ownership, and authority guards. This hardens the existing
runtime_tuning_capacity_defaults authority, so broad source share remains
**23.27% Mojo** rather than changing through filler LOC.

## Runtime route-decision reason migration

Known route-decision label lookup, reason-to-rejection-stage mapping, and safe
unknown-label normalization now use mojo/prodex_core/runtime_route_reason.mojo.
Rust keeps the stable public enums/Serde contract and validates numeric Mojo tags,
but the former label scan, stage match table, and lowercase/digit/underscore safety
policy were removed from production Rust.

Validation passes the direct Mojo ABI smoke test plus a fixed 34-label caller matrix
covering every known reason/stage pair, exact-known misses, safe unknown trimming,
and unsafe labels. Library clippy, prodex-app check, no-fallback self-test, ownership,
and authority guards pass; ownership now reports **77 authoritative operations**.
The canonical broad source report counts **58,583 reachable Mojo LOC** and
**192,771 Rust production LOC**, totaling **251,354 LOC**: **23.31% Mojo**. The
75% project target remains unmet, with **519,730 additional Mojo LOC** required at
this Rust volume.

## Route-decision safe identifier migration

Route-decision identifier trim and UTF-8-safe 96-byte truncation now use the
existing runtime_route_reason.mojo owner through a new span ABI. Rust retains
only owned String materialization and the stable trace DTO/Serde surface; the
byte-boundary loop is no longer duplicated in Rust.

Focused route-decision tests (5/5), the direct Mojo route-reason smoke test,
prodex-app check, no-fallback self-test, ownership, and authority guards pass.
The strict clippy command is currently blocked by two pre-existing dead-code
warnings in smart-context token-accounting test helpers; this wave introduces no
new clippy warning. Ownership now reports **78 authoritative operations**. The
canonical broad source report counts **58,622 Mojo LOC** and **192,796 Rust
production LOC**, totaling **251,418 LOC**: **23.32% Mojo**. The 75% project
target remains unmet, with **519,766 additional Mojo LOC** required at the
current Rust volume.

## DeepSeek metadata-shape policy migration

DeepSeek response-format classification, fixed request-metadata type gates,
degraded JSON-schema detection, and JSON-guidance detection now run through
request-policy Mojo operations 13/14/15. Rust retains dynamic
metadata.<provider_key> validation, stable provider error strings, Serde object
materialization, and the existing RequestMetadata/ResponseFormat serializer
kernels; the previous response-format match table and
to_ascii_lowercase().contains("json") scan were deleted.

Validation passes 6/6 focused metadata tests and 115/115 DeepSeek-focused tests,
including error precedence, nested text.format, supported/unsupported response
formats, decoded escaped JSON guidance, and thinking metadata behavior. Clippy,
prodex-app check, no-fallback self-test, ownership, and authority guards pass;
ownership now reports **81 authoritative operations**. The canonical broad source
report counts **58,835 Mojo LOC** and **192,900 Rust production LOC**, totaling
**251,735 LOC**: **23.37% Mojo**. The 75% project target remains unmet, with
**519,865 additional Mojo LOC** required at the current Rust volume.

## Provider usage extraction and cost migration

Provider token-usage precedence and usage-cost arithmetic now use
mojo/prodex_core/provider_usage.mojo. Rust continues to validate complete JSON
documents with serde and owns SSE framing, but OpenAI/Anthropic/Gemini usage
field alias selection, explicit-total versus input/output fallback, and
saturating microusd cost arithmetic no longer have Rust semantic copies.

Validation passes 6/6 focused usage tests, the direct Mojo usage smoke test,
337/337 prodex-provider-core unit tests plus 7 integration tests, clippy, and
prodex-app check. No-fallback self-test, ownership, and authority guards pass;
ownership now reports **84 authoritative operations**. The canonical broad
source report counts **59,114 Mojo LOC** and **192,965 Rust production LOC**,
totaling **252,079 LOC**: **23.45% Mojo**. The 75% project target remains unmet,
with **519,781 additional Mojo LOC** required at the current Rust volume.

## Super-expose protocol and tool-contract migration

Super-expose MCP protocol-version/header decisions, Content-Type/Accept media
matching, quote-aware JSON nesting limits, per-tool allowed argument keys, run-id
shape validation, and bounded optional-string Unicode control rejection now use
the existing mojo/prodex_core/super_expose.mojo owner. Rust retains URL origin
parsing, request-body field acquisition, HTTP/JSON-RPC response construction,
Serde iteration, UUID parsing, and stable field-specific error text. No Rust
semantic fallback is retained.

Validation passes 78/78 focused prodex-app Super-expose tests and 3/3 direct Mojo
protocol-policy fixtures, plus clippy, full prodex-app check, no-fallback self-test,
ownership, and authority guards. Ownership now reports **91 authoritative
operations**. The canonical broad source report counts **59,651 Mojo LOC** and
**193,104 Rust production LOC**, totaling **252,755 LOC**: **23.60% Mojo**. The
75% project target remains unmet, with **519,661 additional Mojo LOC** required
at the current Rust volume.

## Sub-agent CLI policy migration

Sub-agent maximum concurrency parsing/validation, preset-vs-custom classification,
reasoning-effort keyword parsing, and Unicode-trimmed nonempty model validation now
use mojo/prodex_core/sub_agent_policy.mojo. Provider parsing and credential-free
URL validation remain Rust-owned. Rust now retains only public DTO construction,
stable CLI error strings, and enum/result mapping; no Rust semantic fallback is
retained.

Validation passes 27/27 focused prodex-cli sub-agent tests and 1/1 direct Mojo
policy smoke test, plus clippy, full prodex-app check, no-fallback self-test,
ownership, and authority guards. Ownership now reports **95 authoritative
operations**. The canonical broad source report counts **59,815 Mojo LOC** and
**193,229 Rust production LOC**, totaling **253,044 LOC**: **23.64% Mojo**. The
75% project target remains unmet, with **519,872 additional Mojo LOC** required
at the current Rust volume.

## Runtime transcript semantic migration

Transcript event classification, response-item kind classification, bounded
operation-value validation/truncation, and tool-name sanitization now use
mojo/prodex_core/log_semantics.mojo through the safe prodex_mojo_core::log
adapter. Rust retains JSON extraction, secret redaction, transcript DTO
construction, tool output filtering, and stable source/error rendering. The
former Rust event/status tables and char-bound sanitization loops were removed.

Validation passes 3/3 direct Mojo transcript policy fixtures and 25/25 app log
tests, including MCP/sub-agent/tool events, response items, status/error
classification, ANSI/binary filtering, dedupe, and streaming transcript follow.
Clippy, prodex-app check, no-fallback self-test, ownership, and authority guards
pass; ownership now reports **99 authoritative operations**. The canonical broad
source report counts **60,107 Mojo LOC** and **193,359 Rust production LOC**,
totaling **253,466 LOC**: **23.71% Mojo**. The 75% project target remains unmet,
with **519,970 additional Mojo LOC** required at the current Rust volume.

## Super-expose dispatch validation migration

Post-header Super-expose dispatch validation now uses the
prodex_mojo_super_expose_dispatch_validation_v1 entry in the existing
super_expose.mojo owner. Notification/no-id handling, invalid request-id
classification, initialize params/protocolVersion requirements, and tools/call
params/name/arguments shape decisions no longer have Rust semantic copies. Rust
retains JSON parsing, request-id materialization, MCP header validation ordering,
audit timing, tool argument allowlist, and exact HTTP/JSON-RPC response
construction.

Validation passes the direct dispatch decision matrix and 78/78 focused
Super-expose app tests, plus clippy, full prodex-app check, no-fallback self-test,
ownership, and authority guards. Ownership now reports **100 authoritative
operations**. The canonical broad source report counts **60,178 Mojo LOC** and
**193,424 Rust production LOC**, totaling **253,602 LOC**: **23.73% Mojo**. The
75% project target remains unmet, with **520,094 additional Mojo LOC** required
at the current Rust volume.

## Super provider config serialization migration

Super external-provider alias classification, TOML string literal escaping, and
the ordered 14-entry local/external Codex provider override table now use
mojo/prodex_core/super_provider_config.mojo. Rust retains credential-free URL
validation, provider metadata lookup, Copilot/model token-limit calculation,
API-key ownership, and OsString argument interleaving. No Rust fallback table or
escaping implementation remains.

Validation passes the direct Mojo kernel fixture, all 143 prodex-cli unit tests,
and both Copilot runtime-arg integration tests, plus clippy, full prodex-app
check, no-fallback self-test, ownership, and authority guards. Ownership now
reports **103 authoritative operations**. The canonical broad source report
counts **60,525 Mojo LOC** and **193,580 Rust production LOC**, totaling
**254,105 LOC**: **23.82% Mojo**. The 75% project target remains unmet, with
**520,215 additional Mojo LOC** required at the current Rust volume.

## Runtime broker version guard and log-throughput policy migration

Runtime broker binary identity presence/matching, SHA-first/version-fallback replacement
reasoning, version mismatch detection, current-identity selection, compatible/deferred/
replaced guard planning, and prodex --version token parsing now use the existing
mojo/prodex_core/runtime_broker_continuity.mojo owner. Rust retains DefaultHasher
broker-key hashing, PathBuf ownership, registry/health/process DTO projection, identity
cloning, and stable public reason/outcome enums. No Rust fallback semantic copy is retained.

Log throughput counter-reset/append decisions, completed generation output rate, and
250ms minimum live-stream delta rate now use
mojo/prodex_core/log_throughput_policy.mojo. Rust retains Instant timestamps, VecDeque
sample storage/pruning, stream/profile maps, sticky display state, and TUI rendering.

Validation passes the full prodex-runtime-broker suite (32/32), version-guard direct
adapter fixtures, 26/26 throughput/app tests, the throughput direct kernel fixture,
clippy, prodex-app check, no-fallback self-test, ownership, and authority guards.
Ownership now reports **109 authoritative operations**. The canonical broad source
report counts **60,972 Mojo LOC** and **193,959 Rust production LOC**, totaling
**254,931 LOC**: **23.92% Mojo**. The 75% project target remains unmet, with
**520,905 additional Mojo LOC** required at the current Rust volume.


## Gemini SSE identifier and metadata merge migration

Gemini SSE output-text/media/citation item IDs now use the existing Gemini response
kernel identifiers directly from the provider-core production boundary. The runtime
copy of the UUID-derived identifier formatter was deleted. Repeated Gemini response
metadata now delegates to the parsed-JSON Mojo merge operation: top-level incoming
fields replace existing fields, nested objects merge one level, an incoming object
replaces an existing scalar, and a non-object incoming value preserves an existing
object exactly as the previous runtime behavior did. Rust retains Serde acquisition,
JSON materialization, and the streaming state container; no Rust semantic fallback
remains.

Focused validation passes the provider-core merge fixture plus the Gemini SSE
later-chunk metadata and missing-response-id caller tests. cargo fmt --all -- --check
passes. The canonical broad report on this checkpoint counts **61,023 reachable Mojo
LOC** and **194,026 Rust production LOC**, totaling **255,049 LOC**: **23.93% Mojo**.
The 75% project target remains unmet, with **521,055 additional Mojo LOC** required at
the current Rust volume.


## Kiro response tool-call presence migration

Kiro response tool-call presence now uses operation 50 of the existing Kiro Mojo
response kernel. Rust acquires only the optional output-item type strings and
serializes that bounded projection; Mojo parses the projection and decides whether
any function_call item is present. The previous Rust any/type comparison was deleted,
and finish-reason derivation continues to consume the Mojo-owned result without a Rust
semantic fallback.

Focused validation passes the tracked Kiro response integration fixture covering
tool-call, non-tool, empty-response, and length finish-reason behavior. The canonical
broad report counts **61,055 reachable Mojo LOC** and **194,036 Rust production LOC**,
totaling **255,091 LOC**: **23.93% Mojo**. The 75% project target remains unmet, with
**521,053 additional Mojo LOC** required at the current Rust volume.


## Runtime external-provider classification migration

Runtime launch external-provider classification now has one authoritative Mojo
classifier in super_provider_config.mojo. Anthropic/Claude, Copilot aliases,
DeepSeek, Gemini, Gemini OAuth, Kiro, Gemini native, and Antigravity are classified
once with the exact case-insensitive semantics used by the previous Rust callers.
The repeated Rust alias tables in provider summary, API-key mode, local-rewrite
dispatch, profile-home selection, and launch selection were deleted. Rust retains
auth acquisition, profile/state access, option construction, and stable error text;
there is no Rust semantic fallback for provider-name classification.

Focused validation passes the direct Mojo classifier test plus the runtime provider
mode, alias-to-local-rewrite, Gemini OAuth guidance, rotation-summary, and native
Gemini profileless-selection tests. The canonical broad report counts **61,086
reachable Mojo LOC** and **194,091 Rust production LOC**, totaling **255,177 LOC**:
**23.94% Mojo**. The 75% project target remains unmet, with **521,187 additional Mojo
LOC** required at the current Rust volume.


## Runtime model-provider identifier migration

The internal Codex model-provider identifiers prodex-local, prodex-deepseek,
prodex-gemini, prodex-anthropic, prodex-copilot, and prodex-kiro now have one
authoritative exact case-insensitive classifier in super_provider_config.mojo.
Repeated Rust identifier comparisons were deleted from runtime launch rewrite
selection, Gemini/DeepSeek/local catalog enablement, external catalog selection,
custom quota labels, and quota profile matching. Rust retains effective-config
acquisition and maps the Mojo enum into host-side launch/catalog/quota actions;
custom provider names and Amazon Bedrock aliases remain outside this operation.

Focused validation passes the direct Mojo classifier test plus Gemini, DeepSeek,
local, Anthropic/Copilot/Kiro external-catalog, and provider-mode caller tests.
The canonical broad report counts **61,109 reachable Mojo LOC** and **194,117 Rust
production LOC**, totaling **255,226 LOC**: **23.94% Mojo**. The 75% project target
remains unmet, with **521,242 additional Mojo LOC** required at the current Rust
volume.


## Sub-agent launch validation hard replacement

Sub-agent model nonempty validation at the app boundary now reuses the existing
Mojo model policy instead of repeating Unicode-trim semantics in Rust. The Local
provider versus URL-presence rule is now operation 4 of sub_agent_policy.mojo and
is consumed both while resolving a Super sub-agent and while validating the child
launcher spec. Operation 5 now owns the exact recursion-marker identity and the
1..65536 task-byte limit. The corresponding Rust conditionals were deleted.

Rust still acquires ProviderId/Option values, validates URL syntax, absolute paths,
required optional-tool identifiers, filesystem confinement, locks, and process
execution. Focused validation passes the direct sub-agent Mojo contract plus app
tests for empty model rejection, Local endpoint requirements, child config/tool
validation, and fail-closed recursion-marker policy. The canonical broad report
counts **61,125 reachable Mojo LOC** and **194,166 Rust production LOC**, totaling
**255,291 LOC**: **23.94% Mojo**. The 75% project target remains unmet, with
**521,373 additional Mojo LOC** required at the current Rust volume.


## Runtime provider-profile alias duplicate cleanup

The remaining runtime OAuth profile matcher now consumes the authoritative
runtime_external_provider_class Mojo classifier instead of repeating the
Gemini, Anthropic/Claude, Copilot/GitHub-Copilot, and Kiro alias table in Rust.
Rust retains profile collection lookup and enum matching only; there is no
provider-name fallback classifier in this path.

Focused runtime classifier coverage and the Mojo no-fallback/ownership/authority guards pass. The canonical broad report remains **61,125 Mojo LOC** and **194,167 Rust production LOC**, totaling **255,292 LOC**: **23.94% Mojo**.


## Gemini and DeepSeek runtime catalog identity cleanup

Runtime Gemini and DeepSeek model-catalog generation now reuses the authoritative
rich_catalog Mojo merge planner for trim/blank filtering and exact ASCII-folded
deduplication. Built-in metadata lookup now uses the authoritative catalog resolver
instead of Rust eq_ignore_ascii_case scans. Rust retains candidate collection, accepted
index materialization, catalog JSON construction, and file writes; no Rust duplicate
identity/dedup policy remains in these two generators.

Focused catalog-write tests pass for DeepSeek, Gemini, and a custom Gemini launch model.
The canonical broad report counts **61,125 Mojo LOC** and **194,186 Rust production LOC**,
totaling **255,311 LOC**: **23.94% Mojo**.


## DeepSeek runtime scalar token policy migration

DeepSeek runtime strict-tools boolean token aliases and web-search mode aliases now
use dedicated scalar classifiers in the existing super_provider_config Mojo owner.
Rust preserves empty/Unicode-whitespace rejection and the stable caller-specific error
messages, then maps the Mojo result into the public runtime enums; the former lowercase
match tables were deleted and there is no Rust fallback classification.

Direct Mojo adapter coverage passes together with five strict-tools caller tests and one
web-search-mode caller test. The canonical broad report counts **61,177 Mojo LOC** and
**194,238 Rust production LOC**, totaling **255,415 LOC**: **23.95% Mojo**. The 75%
project target remains unmet.


## Runtime boolean duplicate cleanup

The runtime-config strict boolean parser, compatibility shadow flag, and Presidio
auto-start switch now reuse the authoritative runtime boolean Mojo classifier. Each
Rust caller preserves its existing trim, empty/whitespace, default, and error behavior;
the repeated lowercase alias tables were deleted without fallback recomputation.

Two focused caller-boundary tests pass for strict/compat runtime flags and Presidio
auto-start values. The canonical broad report counts **61,177 Mojo LOC** and **194,237
Rust production LOC**, totaling **255,414 LOC**: **23.95% Mojo**.


## Gemini sticky-fresh OAuth boolean cleanup

PRODEX_GEMINI_STICKY_FRESH_OAUTH now reuses the authoritative runtime boolean
Mojo classifier after preserving its historical trim/default behavior. The Rust
lowercase false-token table was deleted; unknown and empty compatibility values
still preserve the previous enabled default.


## CI and strict-TUI truth-token migration

The exact case-insensitive 1|true|yes environment truth contract now lives in
super_provider_config.mojo. Session scrolling, shared inline-TUI gating, and
profile scrolling all call the same Mojo classifier. The previous Rust lowercase
match tables were deleted. This intentionally remains distinct from the broader
runtime boolean classifier: on is still false for the CI/strict-TUI contract.
Rust retains environment acquisition, defaults, terminal detection, and rendering.


## Super bundled model dedup cleanup

Bundled OpenAI model merge now reuses the authoritative rich-catalog Mojo merge
planner for exact ASCII-folded canonical-ID deduplication. The former Rust
BTreeSet lowercase duplicate filter was deleted. Aliases are intentionally omitted
from this merge input to preserve the historical canonical-ID-only behavior; Rust
retains choice DTO materialization and insertion position.


## Sub-agent dynamic catalog dedup cleanup

Dynamic sub-agent model catalogs now reuse the authoritative rich-catalog Mojo merge
planner per source for case-insensitive canonical-ID deduplication. The long-lived Rust
BTreeSet lowercase index was deleted. Rust still loads profile/catalog files, filters
selectability, preserves source precedence and model limits, and materializes accepted
JSON entries. Aliases are intentionally omitted from the merge input to preserve the
historical canonical-ID-only behavior.


## Exact catalog identity migration

The rich catalog now exposes a separate exact case-insensitive resolver that does not
trim the query. Super main-model canonical/alias matching and runtime provider dynamic
model lookup use this Mojo operation, deleting their Rust eq_ignore_ascii_case scans.
The existing trimmed resolver remains unchanged for callers whose contract intentionally
normalizes surrounding whitespace. Focused coverage fixes mixed-case canonical and alias
matching while proving that a space-padded query remains a miss.


## External dynamic catalog exact-identity cleanup

Copilot/Kiro external dynamic catalog lookup now reuses the exact non-trimming Mojo
catalog resolver for launch-model context lookup and per-entry metadata selection. The
two Rust eq_ignore_ascii_case scans were deleted. Rust retains catalog-file parsing,
context-window arithmetic, provider metadata fallback, and final JSON materialization.


## Built-in profile import source migration

Exact case-insensitive classification for the built-in claude, copilot, and kiro
profile import source names now lives in super_provider_config.mojo. The three Rust
eq_ignore_ascii_case copies were deleted. The classifier deliberately does not trim
input, preserving the previous path-component contract; Rust retains one-component
path validation, path existence checks, and provider-specific import execution.


## Runtime OpenAI model policy migration

OpenAI runtime provider-name recognition, large-context model-family classification,
and max-context-window preference now use one scalar-policy entry in
super_provider_config.mojo. Models-cache slug/id lookup now reuses the authoritative
trimmed rich-catalog resolver. The previous Rust lowercase/prefix tables and manual
case-insensitive cache scan were deleted. Rust retains config/file acquisition, JSON
projection, numeric context-window extraction, and Codex override materialization.


## Kiro model endpoint exact-identity cleanup

Kiro model endpoint lookup now reuses the exact non-trimming rich-catalog Mojo
resolver. The Rust eq_ignore_ascii_case scan over model IDs was deleted. Mixed-case
model IDs still resolve, while space-padded IDs remain a miss exactly as before; Rust
retains response JSON ownership and the existing Mojo model-not-found shape.


## Copilot and Kiro profile catalog dedup cleanup

Copilot runtime-token model catalogs and Kiro profile model catalogs now reuse the
authoritative rich-catalog Mojo merge planner for case-insensitive canonical-ID
deduplication. The two Rust lowercase BTreeSet filters were deleted. Both callers
normalize IDs before the Mojo boundary, so first-win ordering, whitespace behavior,
hard limits, provider-specific normalization, and downstream validation remain
unchanged.


## Super exact casefold equality migration

A bounded exact ASCII-casefold equality primitive now lives in rich_catalog.mojo and
preserves empty strings plus non-trimming whitespace semantics. Super catalog and
prompt flows use it for source-ID fallback matching, visibility, model/alias identity,
reasoning-effort defaults, remembered selections, and prompt selection. The repeated
Rust eq_ignore_ascii_case decisions in these production paths were deleted; Rust keeps
iteration, DTO ownership, prompt rendering, and the case-sensitive first-pass source
lookup needed to preserve duplicate-case ordering.


## Gemini assistant guardrail text migration

Gemini assistant wait/poll narration, future tool-intent detection with exact token
boundaries, success-claim markers, tool failure/version verification, process-exit-zero
recognition, and command-output-only phrase policy now use the dedicated
gemini_guardrails.mojo kernel. The Rust ASCII-lowercase phrase tables and token scan
were deleted. Rust retains conversation JSON extraction, tool-output collection, typed
label mapping, and orchestration over the latest verification outputs; no Rust semantic
fallback remains for these text classifications. Direct kernel coverage plus four
Gemini SSE regressions pass. The canonical broad report counts **61,715 Mojo LOC** and
**194,504 Rust production LOC**, totaling **256,219 LOC**: **24.09% Mojo**.


## Gemini exact-output guardrail migration

Gemini exact-output required-command extraction, latest tool-output slicing, diff-tail
trimming, PRODEX marker matching, Unicode-whitespace command normalization, and
normalized exact/substring command matching now run in gemini_guardrails.mojo. Rust
retains only conversation/tool-call JSON acquisition, command-string collection, scratch
buffer ownership, and returned-string materialization. The former Rust marker parser,
normalizer, token extractor, and command matching implementation were deleted with no
semantic fallback. The full focused exact-output caller suite passes. The canonical
broad report counts **62,249 Mojo LOC** and **194,562 Rust production LOC**, totaling
**256,811 LOC**: **24.24% Mojo**.


## Generic exact ASCII identity cleanup

The exact non-trimming ASCII-casefold Mojo primitive now replaces additional Rust
eq_ignore_ascii_case decisions in Presidio fail-mode enforcement, goal-resume command
recognition, Kiro Codebase Memory executable/server matching, ChatGPT account labels,
OpenAI residency and tmux detection, Kiro/provider GET model routing, Smart Context
exact mode, and Super-expose Prodex/Codex process-role detection. Callers retain any
historical trim step plus path/header/process acquisition and routing behavior; no
Rust semantic equality fallback remains in these migrated paths.


## Bedrock and runtime header identity cleanup

Amazon Bedrock provider aliases plus runtime HTTP/websocket header identity now reuse
the authoritative exact ASCII-casefold Mojo primitive. Rust eq_ignore_ascii_case copies
were deleted from resume/provider display, websocket upgrade detection, Content-Length,
authorization/account headers, generic local-rewrite header lookup, turn-state/cookie
filtering, Sec-WebSocket-Key lookup, and Smart Context turn-metadata detection. Rust
retains header parsing/validation, forwarding, provider resolution, and routing actions.


## Remaining catalog/header exact-identity cleanup

Copilot runtime-token <redacted> catalog-key detection, sub-agent visibility, DeepSeek
conversation namespace headers, Gemini compact metadata header replacement, and
Super-expose trusted-origin host matching now reuse the authoritative exact ASCII-casefold
Mojo primitive. The remaining Rust eq_ignore_ascii_case/lowercase copies in these paths
were deleted. Rust retains JSON/catalog acquisition, URL parsing, namespace selection,
header mutation, and caller-specific routing behavior.


## Generic ASCII casefold relation migration

The rich catalog kernel now exposes bounded ASCII-casefold starts-with, ends-with,
and contains relations, and exact equality now accepts large Rust strings without the
previous 64 KiB adapter ceiling. Copilot request-role classification, Gemini data-URL
base64 detection, URI MIME suffixes, Code Assist help/domain/trusted-host matching, and
Smart Context application/json content-type recognition now delegate to Mojo. Rust
retains JSON/URL/header acquisition, historical trim/split behavior, and result mapping;
no lowercase semantic fallback remains in these migrated paths.


## Provider quota, media, and Presidio casefold cleanup

Session-memory auto/default aliases, Gemini quota/rate-limit/terminal-quota codes, and
Gemini response-media extension mapping now use the authoritative exact ASCII-casefold
Mojo primitive. Presidio timeout, concurrency, and malformed-response text classification
uses the Mojo contains relation. The Rust lowercase/match tables were deleted while
caller-owned trim, JSON/error acquisition, numeric parsing, and output-label mapping stay
in Rust. Mixed-case and whitespace regressions pass with no Rust semantic fallback.


## Runtime SSE, rate-limit, and Gemini model-memory cleanup

Gemini/provider SSE content-type detection and buffered rate-limit markers now use the
authoritative Mojo casefold contains relation. Gemini normalized error response headers
reuse exact Mojo equality, and the Gemini OAuth selected-model path now calls the
Mojo-backed provider_model_allows_session_memory policy instead of repeating the
empty/auto/default lowercase table. Rust retains HTTP/body acquisition, UTF-8 checks,
prefetch routing, header mutation, and model-memory state ownership. Focused quota,
header, and model-memory regressions pass with no Rust semantic fallback.


## Optional-tools policy hard replacement

Optional-tool name/alias classification, descriptor kind/capability policy, Super default
membership, and current/legacy manifest-tree compatibility now live in the dedicated
optional_tools_policy.mojo owner. Playwright incompatible-version error recognition reuses
the authoritative Mojo casefold-contains relation. The Rust alias/descriptor/default/
manifest tables and lowercase error classifier were deleted. Rust retains filesystem/PATH
discovery, process probes, semver parsing/comparison, hashing, health DTOs, and strict
FFI-to-enum/capability decoding; there is no Rust semantic fallback.


## Login profile slug sanitizer migration

Login/API-key profile slug sanitization now runs in the existing profile_identity.mojo
owner. Mojo owns trim, ASCII lowercase, allowed-character preservation, @ to underscore,
one-dash-per-Unicode-scalar replacement, leading/trailing separator trimming, and the
api_key fallback. The Rust sanitizer implementation was deleted; Rust retains URL/host
acquisition, profile-name availability checks, suffix search, and filesystem state.


## Confirmation token policy migration

Manual redeem and profile-export yes/no confirmation parsing now share the dedicated
confirmation_policy.mojo kernel. Mojo owns Unicode-whitespace trimming, ASCII-casefold
classification of y/yes/n/no, redeem default-no behavior, caller-supplied empty-input
defaults, and unknown-token rejection. The two Rust lowercase match tables were deleted;
Rust retains terminal input, prompt loops, retry text, and the resulting user action.


## Gemini tooling normalization and alias policy migration

Gemini tool-name normalization, dotted-prefix stripping, double-underscore namespace
suffix extraction, alias-family expansion, mutating-tool classification, canonical
run_shell_command output naming, and Gemini-3 model/toolset detection now live in the
dedicated gemini_tooling_policy.mojo owner. The Rust normalization, alias match tables,
mutation table, and model lowercase classifier were deleted. Rust retains BTreeSet/string
materialization plus JSON declaration mutation driven by the Mojo policy outputs.


## Runtime-proxy exact identity cleanup

Runtime-proxy GET/method checks, generic request/session/turn-metadata headers, transport
and connection-token filtering, response Content-Type/header lookup, turn-state metadata
extraction, Retry-After and rate-limit header lookup, stable/free-form log-key classes,
and websocket error-code identity now reuse the authoritative exact ASCII-casefold Mojo
primitive. The Rust eq_ignore_ascii_case copies were deleted; the byte-level percent-dot
URL parser remains Rust-owned because it is syntax parsing rather than string policy.


## Provider scalar-policy migration

Reasoning-effort token parsing, Copilot model prompt-token limits, and Gemini boolean
token classification now share the provider_constraints.mojo scalar-policy ABI. Mojo
owns Unicode-whitespace trimming, ASCII-casefold token/model identity, effort enum
classification, boolean aliases, and the complete model-to-token-limit table. The three
Rust lowercase/match tables were deleted; Rust retains typed enum/Option mapping and JSON
value acquisition only, with no Rust semantic fallback.


## Runtime scalar-config migration

Runtime log-format parsing, runtime-proxy preset aliases, Super web-search mode,
current-time clock-source parsing, and Codex OpenAI provider identity now share the
runtime_tuning.mojo scalar-config ABI. Mojo owns each caller's historical trim/no-trim
contract, ASCII-casefold classification, many-terminals alias, unknown-token handling,
and OpenAI identity. The Rust lowercase/match tables were deleted; Rust retains enum
mapping, CLI error strings, and config/policy DTO ownership only.


## Stored profile identity-match migration

Stored-profile matching for Copilot, Kiro, Gemini, and Anthropic now reuses the existing
profile_identity.mojo owner. The kernel distinguishes trimmed case-sensitive equality,
trimmed ASCII-casefold equality, strict optional presence, wildcard optional auth-method
matching, and Kiro's historical empty-string-as-absent semantics. The duplicated Rust
trim/casefold/optional matching helper was deleted. Rust retains provider enum dispatch
and combines typed Mojo boolean results only; there is no semantic Rust fallback.


## Quota report and external metadata identity cleanup

Quota report auth/provider label matching now uses the authoritative exact ASCII-casefold
Mojo primitive, preserving mixed-case matches while retaining whitespace sensitivity.
Static external-provider model metadata lookup now uses the exact rich-catalog Mojo
resolver instead of a Rust eq_ignore_ascii_case scan. Rust retains quota snapshot/DTO
ownership, provider filtering, static metadata tables, and fallback description mapping.


## ASCII casefold substring-position migration

The rich-catalog Mojo kernel now exposes case-insensitive substring search with the
original byte offset. Gemini retry-delay and runtime Retry-After message parsing use
that offset directly instead of allocating lowercase shadow strings; uppercase S/MS
suffixes are classified by the existing Mojo starts-with relation. Runtime transport
failure message rules also use Mojo contains. Rust retains numeric duration parsing,
retry policy mapping, IO-error kind mapping, and ordered rule precedence.


## Codex binary and WebSocket no-proxy casefold cleanup

Codex version-label validation, recursive Prodex wrapper filename/text detection, and
WebSocket NO_PROXY exact/suffix host matching now reuse the authoritative Mojo exact,
starts-with, ends-with, and contains relations. Rust lowercase shadow strings were
deleted. Rust retains filesystem/executable probing, version parsing, host normalization,
port parsing, wildcard policy, and wrapper process resolution.


## Session-store and runtime-launch identity migration

prodex-session-store and prodex-runtime-launch now enable mojo-rich and use the
authoritative exact ASCII-casefold primitive for exact session-ID resolution, exact
resume-store matching, local-provider identity, and NO_PROXY bypass deduplication. The
corresponding Rust eq_ignore_ascii_case copies were deleted. Session prefix matching
remains Rust Unicode-lowercase intentionally because its semantics are broader than the
ASCII exact-identity contract.


## Usage-limit and runtime error casefold cleanup

Goal/usage-limit status, error-type/code identity, canonical usage-limit messages,
usage-limit prefixes, Gemini compact error-text classification, and runtime token
invalidation detection now reuse the authoritative rich-catalog ASCII-casefold Mojo
primitives. The Rust lowercase shadow strings and eq_ignore_ascii_case tables were
deleted from these production paths. Rust retains JSON traversal, historical trimming,
recovery state transitions, HTTP/error acquisition, and final domain-specific labels.


## Quota render casefold policy cleanup

Quota reset-message marker search, case-insensitive Codex reset-header lookup, and
unauthorized error classification now reuse the authoritative rich-catalog Mojo
find/equality/contains primitives. The Rust lowercase shadow string and header
eq_ignore_ascii_case scan were deleted. Rust retains JSON/time parsing, reset-window
selection, numeric conversion, and final quota status/detail rendering.


## Runtime cookie identity cleanup

Runtime cookie relay now reuses the authoritative exact ASCII-casefold Mojo primitive
for Cookie header detection and Secure, Path, Max-Age, and Expires attribute names. The
Rust eq_ignore_ascii_case copies were deleted. Host lowercase canonicalization remains
Rust-owned because it materializes a normalized map key rather than deciding equality.
Mixed-case boundary coverage preserves the previous HTTP cookie behavior.


## Presidio identity-policy cleanup

Prodex Presidio config validation now reuses the authoritative exact ASCII-casefold Mojo
primitive for open/closed fail-mode validation, fail-closed selection, exact trusted-host
matching, and localhost recognition. The Rust eq_ignore_ascii_case/lowercase decision
copies were deleted. Trusted-host lowercase canonicalization remains Rust-owned because
it materializes normalized persisted/runtime values rather than deciding equality.


## Copilot profile-export host identity cleanup

Profile export/import Copilot model-endpoint selection now reuses the authoritative exact
ASCII-casefold Mojo primitive for github.com, http://github.com, and https://github.com
identity. The three Rust eq_ignore_ascii_case copies were deleted. URL trimming and
enterprise GHE suffix rewriting remain Rust-owned because they transform the endpoint.


## Secret backend identity cleanup

SecretBackendKind parsing now reuses the authoritative exact ASCII-casefold Mojo
primitive for file/keyring classification. The Rust to_ascii_lowercase match table was
deleted while the existing padded-value rejection and exact error branch remain intact.
Mixed-case parser coverage preserves the previous public FromStr contract.

## Gemini Code Assist policy hard replacement

Gemini Code Assist endpoint normalization and plan-tier labeling now run in the
dedicated `gemini_code_assist_policy.mojo` owner. Mojo owns Unicode-whitespace
trimming, trailing-slash removal, exact tier-ID aliases, `-tier` stripping,
ASCII lowercase normalization, and case-insensitive plan-name classification.
Rust retains environment/JSON acquisition, DTO ownership, HTTP orchestration,
and string materialization only. The previous Rust normalization and tier-label
tables were deleted; Mojo errors are not recomputed in Rust.

Focused validation passes cargo fmt, the Mojo core build check, the prodex-app
Mojo-core check, git diff --check, the Mojo authority guard, and the no-fallback
guard. The canonical broad report counts 63,304 Mojo LOC and 195,676 Rust
production LOC, totaling 258,980 LOC: 24.443586377326433% Mojo.

## Kiro Anthropic response hard replacement

Kiro Responses-to-Anthropic message projection now runs through one bounded raw-JSON
Mojo rewrite. Mojo owns output traversal, tool/text ordering, tool argument JSON decode
and invalid-input fallback, raw ID/name/usage preservation, stop-reason precedence,
defaults, and final Anthropic message assembly. Rust now serializes the caller-owned
Serde value, invokes the raw ABI, and decodes the returned JSON only; the previous Rust
walk/reconstruction path was deleted with no fallback recomputation.

The wave also removes the now-dead generic AnthropicToolUseBlock and AnthropicResponse
Kiro kernel operations, updates the no-fallback guard and ownership inventory to the
new raw ABI, and fixes two existing Mojo chat-response edge cases exposed by the new
focused suite: valid non-object top-level values now retain default response shaping,
and leading non-object output items no longer stop first-message text discovery.

Validation passes 50 focused Kiro translator tests, 36 prodex-mojo-core rich tests,
focused clippy with warnings denied, cargo fmt, git diff --check, the Mojo ownership,
authority, and no-fallback guards. Ownership reports 137 authoritative operations and
92.73% Mojo in the eligible semantic inventory. The canonical broad report counts
63,642 Mojo LOC and 195,667 Rust production LOC, totaling 259,309 LOC:
24.54291983695128% Mojo.

## Gemini full request-contents hard replacement

Gemini Responses request-content shaping now uses one authoritative TextContents
Mojo transform for the full production path. Mojo owns role dispatch, system and
contextual-user instruction extraction, assistant/tool/user grouping, prior tool-name
lookup by call ID, JSON argument and tool-response decoding/validation, media/data-URL
projection, MIME inference, Unicode text handling, malformed-field fallbacks, and final
Gemini content ordering. Rust now retains request serialization, one bounded Mojo call,
error mapping, and final Serde materialization only.

The former Rust `items.rs`, `system_instruction.rs`, and `text.rs` production modules
were deleted rather than retained as fallback/oracle paths. Focused Unicode, reversed
tool-response, media-interleaving, malformed-input and contextual-instruction regressions
pass; the complete `prodex-provider-core` Mojo suite passes 357 library tests plus 7
integration tests. Focused all-target Clippy passes with warnings denied. A fresh direct
Mojo object build of `provider_constraints.mojo` passes, as do the no-fallback self-test,
authority guard, ownership guard and canonical production-share guard.

The canonical broad report now counts **64,606 reachable Mojo LOC** and **195,342 Rust
production LOC**, totaling **259,948 LOC**: **24.85343222490652% Mojo**. The 75% project
target remains unmet, with **521,420 additional Mojo LOC** required at the current Rust
volume. The eligible semantic ownership report is **92.92% Mojo (32,756 / 35,250)**;
that narrower metric does not substitute for the broad 75% project target.

## DeepSeek common request-plan hard replacement

DeepSeek Responses-to-Chat request planning now uses one Mojo-owned common request
plan for parameter rejection, response-format degradation, final body shaping, and
continuation metadata projection. Mojo owns the `parallel_tool_calls=false` rejection,
response-format type classification and JSON-schema-to-json_object degradation flag,
`previous_response_id` extraction, turn/session metadata inclusion, and the final
continuation object. Rust retains request JSON acquisition, stable public error text,
ProviderTransformResult construction, and Serde materialization only; the former Rust
policy branches and metadata assembly were deleted with no fallback recomputation.

Validation passes direct Mojo object compilation, `cargo fmt`, `git diff --check`,
focused provider-core Mojo compilation, 8 focused request-transform tests, focused
Clippy with warnings denied, and the Mojo authority, no-fallback, ownership, and
production-share guards. The canonical broad report counts **64,744 Mojo LOC** and
**195,343 Rust production LOC**, totaling **260,087 LOC**: **24.893208810897892% Mojo**.
The 75% broad project target remains in progress.

## Copilot raw request-policy hard replacement

Copilot request sanitization and capability-shape detection now run in the existing
`provider_constraints.mojo` raw JSON owner through a dedicated versioned ABI. Mojo owns
recursive `encrypted_content` removal with compaction preservation, escaped JSON-key
matching, Chat/Responses agent-role classification with the historical trim/casefold
contracts, and exact vision-payload detection for Responses and Chat request shapes.
Rust retains raw byte ownership, canonical-model rewriting, ABI status mapping, and the
public invalid-JSON default behavior only; the previous recursive Serde traversal and
agent/vision Rust helpers were deleted with no semantic fallback implementation.

Validation passes a fresh direct Mojo object build, 5 Copilot provider characterization
tests, 2 direct ABI adapter tests, focused provider-core Clippy with warnings denied,
`cargo fmt`, `git diff --check`, and the Mojo authority, no-fallback, ownership, and
production-share guards. The canonical broad report counts **65,212 Mojo LOC** and
**195,275 Rust production LOC**, totaling **260,487 LOC**: **25.03464664263476% Mojo**.
The 75% broad project target remains in progress.

## Kiro chat-message item hard replacement

Kiro Chat-to-Responses single-message expansion now reuses the raw JSON message/tool
helpers in `kiro.mojo` through a dedicated kernel operation. Mojo owns role defaults,
message text extraction, assistant tool-call and legacy function-call projection,
tool/function output shaping, call-id/name/arguments defaults, and missing-content
behavior. The Rust item traversal and JSON assembly were deleted, together with the
now-unreachable Rust chat-message text helper; no Rust parity implementation remains.
The shared raw helper was also aligned so assistant messages without content emit no
message item and tool outputs without content produce the historical empty string.

Validation passes a fresh direct Kiro Mojo object build, all 14 focused Kiro request
tests, focused provider-core Clippy with warnings denied, `cargo fmt`, `git diff --check`,
and the Mojo authority, no-fallback, ownership, and production-share guards. The
canonical broad report counts **65,244 Mojo LOC** and **195,169 Rust production LOC**,
totaling **260,413 LOC**: **25.054048761006555% Mojo**. The 75% broad project target
remains in progress.

## Runtime-log recording policy hard replacement

`PRODEX_RUNTIME_LOG_RECORD` token classification now runs through the existing
`runtime_tuning.mojo` scalar-config ABI. Mojo owns Unicode-whitespace trimming,
ASCII-casefold matching, and the exact enabled aliases `1`, `true`, `yes`, and
`on`. `prodex-runtime-log` retains environment acquisition only; the previous
Rust lowercase/match table was deleted and no Rust semantic fallback remains.

Focused validation passes the real-Mojo scalar-config test, the complete
`prodex-runtime-log` library suite (17 tests), `cargo check`, formatting,
`git diff --check`, and the Mojo no-fallback/ownership/production-share guards.
The canonical broad report at this checkpoint counts **68,286 reachable Mojo
LOC** and **198,888 Rust production LOC**, totaling **267,174 LOC**:
**25.558624716476903% Mojo**. The 75% broad target remains in progress.

## Audit usage-ledger policy hard replacement

Usage-ledger deterministic policy is now Mojo-owned through
`audit_log_policy.mojo`. The migrated kernel owns usage-token normalization,
implicit total-token saturation, inclusive time-window aggregation, and
request/token/cost budget-limit classification. `prodex-audit-log` retains
Serde/file boundaries, calendar month-boundary acquisition, DTO packing, and
human-readable reason formatting only. The previous Rust normalization loop,
saturating aggregation loop, and threshold comparisons were deleted; there is
no feature-off or runtime Rust fallback.

Focused validation passes the real-Mojo policy test, all 17 `prodex-audit-log`
tests, clippy with warnings denied for both affected crates, and the Mojo
no-fallback guard including a new audit-usage regression rule. The canonical
broad report at this checkpoint counts **68,496 reachable Mojo LOC** and
**199,090 Rust production LOC**, totaling **267,586 LOC**:
**25.59775175083898% Mojo**. The 75% broad target remains in progress.

## Core temp and broker filename policy hard replacement

Foundational temp-file and runtime-broker filename semantics now run through
`core_file_policy.mojo`. Mojo owns root-temp ownership and PID parsing, stale
root-temp removal eligibility, runtime-log filename ownership, login-temp
ownership, runtime-broker artifact key slicing, and lease PID parsing. `prodex-core`
keeps only filesystem/path boundaries plus checked conversion of Mojo slice
ranges back into borrowed Rust strings. The previous Rust prefix/suffix/split
parsers and stale-removal predicate were deleted; no Rust fallback remains.

Focused validation passes the real-Mojo core-file policy test, all 11
`prodex-core` tests, clippy with warnings denied, `git diff --check`, and the
Mojo no-fallback guard with a dedicated regression rule. The canonical broad
report at this checkpoint counts **68,679 reachable Mojo LOC** and **199,191
Rust production LOC**, totaling **267,870 LOC**: **25.63892933139209% Mojo**.
The 75% broad target remains in progress.

## MCP Content-Length framing policy hard replacement

MCP stdio Content-Length semantics now run through `mcp_stdio_policy.mojo`.
Mojo owns case-insensitive Content-Length header recognition (including the
trimmed continuation-header path) and bounded decimal Content-Length parsing,
including optional leading `+`, Unicode surrounding whitespace, overflow, and
missing-separator classification. Rust retains streaming `BufRead`, bounded
buffer acquisition, JSON serde, and wire writes only. The former Rust
lowercase-prefix checks plus `split_once`/`parse::<usize>` semantic parser were
deleted and no Rust fallback remains.

Focused validation passes the real-Mojo MCP policy test, all 16
`prodex-mcp-stdio` tests, clippy with warnings denied, `git diff --check`, and
the Mojo no-fallback guard with a dedicated regression rule. The canonical
broad report at this checkpoint counts **68,820 reachable Mojo LOC** and
**199,268 Rust production LOC**, totaling **268,088 LOC**:
**25.67067530064755% Mojo**. The 75% broad target remains in progress.

## Update-notice install and cache policy hard replacement

Update-notice deterministic policy now runs through `update_notice_policy.mojo`.
Mojo owns npm/Cargo/standalone install-channel classification with slash/backslash
path equivalence, command-level notice emission policy, and saturating cache-age
freshness checks. Rust retains environment/path acquisition, command enum tagging,
SemVer parsing/comparison through the mature `semver` dependency, HTTP, files,
and user-facing rendering. The previous Rust path normalization/substring table,
notice gating match, and cache-age predicate were deleted with no Rust fallback.

Focused validation passes the real-Mojo policy test, all 11
`prodex-update-notice` tests including the existing Windows-path fixture, Clippy
with warnings denied, `git diff --check`, and the dedicated Mojo no-fallback
rule. The canonical broad report at this checkpoint counts **68,957 reachable
Mojo LOC** and **199,417 Rust production LOC**, totaling **268,374 LOC**:
**25.69436681645763% Mojo**. The 75% broad target remains in progress.

## Shared Codex attachment scanner hard replacement

Session attachment path scanning now runs through `shared_attachment_policy.mojo`.
Mojo owns image-tag path range extraction (including escaped JSON attributes),
inline clipboard and attachment marker discovery, path-byte/JSON-escape boundary
rules, trailing-dot trimming, clipboard filename classification, persistable
attachment filename policy, and rollout filename classification. Rust retains
filesystem `Path` interpretation, path containment/security checks, metadata,
copying, zstd, and file I/O. The former Rust marker tables, path continuation
scanner, byte classifier, image path-attribute parser, and filename policy were
deleted; no Rust fallback remains.

Focused validation passes the real-Mojo scanner policy test, all 75
`prodex-shared-codex-fs` tests, Clippy with warnings denied, `git diff --check`,
and a dedicated no-fallback regression rule. The canonical broad report counts
**69,216 reachable Mojo LOC** and **199,403 Rust production LOC**, totaling
**268,619 LOC**: **25.76735078307938% Mojo**. The 75% broad target remains in
progress.

## Runtime cookie parsing hard replacement

Runtime-proxy cookie syntax and matching policy now runs through
`runtime_cookie_policy.mojo`. Mojo owns bounded Set-Cookie and caller-cookie
pair validation, cookie-name/value byte safety, Secure/Path/Max-Age/Expires
attribute classification, signed Max-Age parsing and precedence, default-path
planning, request-path matching, secure-scheme classification, and normalized
host materialization. Rust retains header segment acquisition, `chrono` RFC2822
Expires parsing, URL/transport ownership, timestamps, jar mutation, and final
header rendering. The replaced Rust split/byte-validator/Max-Age/default-path/
path-match/host-normalization implementations were deleted; no semantic Rust
fallback remains.

Focused validation passes the standalone Mojo build, the real-Mojo adapter test,
all 12 `prodex-runtime-cookies` tests, Clippy with warnings denied for the
adapter and consumer, `git diff --check`, and a dedicated no-fallback rule. The
canonical broad report at this checkpoint counts **69,648 reachable Mojo LOC**
and **199,578 Rust production LOC**, totaling **269,226 LOC**:
**25.8697154063872% Mojo**. The 75% broad target remains in progress.

## Runtime broker-log cache policy hard replacement

Runtime broker-log cache relation and eviction semantics now reuse the existing
`runtime_broker_continuity.mojo` owner. Mojo owns exact/append/rotated
fingerprint classification, monotonic timestamp ordering, LRU victim selection
with keep-entry preference, and continuity event-string classification. Rust
retains filesystem metadata acquisition, `Mutex`/`BTreeMap` ownership, bounded
log reads, metric map mutation, and path materialization. The duplicated Rust
fingerprint predicates, two `min_by_key` LRU selectors, and event-string match
table were deleted; no Rust semantic fallback remains.

Focused validation passes the real-Mojo continuity smoke, all 5
`prodex-runtime-broker-log` tests, Clippy with warnings denied for both affected
crates, `git diff --check`, and the expanded no-fallback guard. The canonical
broad report at this checkpoint counts **69,729 reachable Mojo LOC** and
**199,676 Rust production LOC**, totaling **269,405 LOC**:
**25.882593121879697% Mojo**. The 75% broad target remains in progress.

## Shared Codex history dedup and size-policy hard replacement

Shared Codex `history.jsonl` merge now delegates exact-line first-win deduplication
and merged-size admission to `shared_history_policy.mojo`. Rust retains bounded
file acquisition, Serde timestamp extraction, the historical timestamp/order
comparator, permissions, and atomic replacement. The previous Rust `BTreeSet`
dedup state and per-line saturating merged-size planner were deleted; there is
no Rust fallback for the migrated decisions. The timestamp sort remains Rust on
purpose because the historical comparator falls back to insertion order when
either side lacks `ts`, which is not a total-order key and must be preserved
exactly rather than silently redefined during this wave.

Focused validation passes the real-Mojo dedup/size test, all 75
`prodex-shared-codex-fs` tests, Clippy with warnings denied for both affected
crates, `git diff --check`, and a dedicated no-fallback guard. The canonical
broad report at this checkpoint counts **69,863 reachable Mojo LOC** and
**199,781 Rust production LOC**, totaling **269,644 LOC**:
**25.909347139190935% Mojo**. The 75% broad target remains in progress.

## External provider static catalog hard replacement

Static Anthropic, Copilot, and Kiro external-provider model metadata is now
Mojo-owned in super_provider_config.mojo. Mojo owns the canonical ordered
model table, indexed model materialization, and exact ASCII-casefold lookup for
the 9 Anthropic, 24 Copilot, and 2 Kiro static entries. Rust retains dynamic
account-catalog acquisition, provider-core reasoning/token metadata, JSON
materialization, filesystem persistence, and launch-config I/O. The previous
Rust models() table and Rust-side metadata lookup were deleted; no Rust
semantic fallback remains.

Validation in an isolated clean worktree passes the real-Mojo
super_provider_config test, 8 focused prodex-app external-provider catalog
tests, Clippy with warnings denied for prodex-mojo-core and prodex-app,
git diff --check, and the dedicated no-fallback guard. The canonical broad
report for this checkpoint counts **70,075 reachable Mojo LOC** and **199,710
Rust production LOC**, totaling **269,785 LOC**: **25.97438701187983% Mojo**.
The 75% broad target remains in progress.

## Terminal info summary rendering hard replacement

Deterministic terminal info summaries now render through info_render.mojo. Mojo
owns relative-duration text, quota-data summaries, runtime policy/log summaries,
runtime-tuning worker/budget/transport summaries, and pool-remaining text. Rust
retains the typed display DTOs, terminal/Ratatui I/O, and higher-level runway or
token-usage composition. The replaced Rust formatting branches and format
templates were deleted; no Rust semantic fallback remains.

Focused validation passes the real-Mojo info-render test, all 22
prodex-terminal-ui tests, Clippy with warnings denied for prodex-mojo-core and
prodex-terminal-ui, git diff --check, and the dedicated no-fallback guard. The
canonical broad report at this checkpoint counts **70,457 reachable Mojo LOC**
and **199,816 Rust production LOC**, totaling **270,273 LOC**:
**26.06882670485028% Mojo**. The 75% broad target remains in progress.

## Terminal process, load, and token-usage rendering hard replacement

The existing terminal info Mojo owner now also handles process summaries,
runtime-load summaries, and token-usage summaries including bounded per-profile
detail rendering. Rust retains arbitrary Display-to-string materialization and
typed DTO construction only. The previous Rust branch logic and formatting
templates for these summaries were deleted; no Rust fallback remains.

Focused validation passes the expanded real-Mojo info-render test, all 22
prodex-terminal-ui tests, Clippy with warnings denied for prodex-mojo-core and
prodex-terminal-ui, git diff --check, and the strengthened no-fallback guard.
The canonical broad report at this checkpoint counts **70,639 reachable Mojo
LOC** and **199,872 Rust production LOC**, totaling **270,511 LOC**:
**26.113170998591553% Mojo**. The 75% broad target remains in progress.

## DeepSeek static catalog hard replacement

Static DeepSeek model catalog metadata now lives in `super_provider_config.mojo`.
Mojo owns the canonical ordered seven-model table, indexed materialization, and
trimmed ASCII-casefold exact lookup for `auto`, `pro`, `flash`, the DeepSeek V4
variants, and the compatibility `deepseek-chat`/`deepseek-reasoner` entries.
Rust retains launch-model composition, typed metadata materialization, URL/JSON
and transport concerns, and user-facing fallback formatting for unknown dynamic
model ids. The previous Rust `DEEPSEEK_CATALOG_MODELS` table and Rust-side table
lookup were deleted; no Rust semantic fallback remains for the migrated static
catalog decisions.

Focused validation passes the real-Mojo `super_provider_config` test, Clippy
with warnings denied for `prodex-mojo-core`, `git diff --check`, and the expanded
no-fallback guard. A full `cargo check -p prodex-app` was also attempted but
exceeded the 120-second local connector window without emitting a compile
failure, so this checkpoint does not wait on that lane. The canonical broad
report at this checkpoint counts **70,743 reachable Mojo LOC** and **199,898
Rust production LOC**, totaling **270,641 LOC**: **26.139055058176698% Mojo**.
The 75% broad target remains in progress.

## Sub-agent child argv planning hard replacement

Sub-agent child process argument planning now runs through the existing
`sub_agent_policy.mojo` owner. Mojo determines the exact ordered action stream
for the Super command marker, recursion-disable flag, Presidio mode, repeated
required-tool pairs, OpenAI/local/named provider routing, optional model and
reasoning-effort overrides, and final `exec`/task placement. Rust retains only
typed ABI decoding, `OsString` materialization of dynamic values, validated
local URL/model/effort access, process spawning, locking, filesystem I/O, and
child lifecycle management. The previous Rust provider/presidio/tool/model/
effort argv branch planner was deleted; no Rust semantic fallback remains for
the migrated argv ordering decisions.

Focused validation passes the standalone Mojo build, the real-Mojo
`sub_agent_policy` adapter test, all 3 focused `prodex-app` child-argv tests,
Clippy with warnings denied for `prodex-mojo-core`, `git diff --check`, and the
expanded no-fallback guard. The canonical broad report at this checkpoint
counts **70,824 reachable Mojo LOC** and **200,012 Rust production LOC**,
totaling **270,836 LOC**: **26.150142521673633% Mojo**. The 75% broad target
remains in progress.

## Runtime overlay argv policy hard replacement

Runtime overlay CLI scanning and projection now run through
`runtime_overlay_policy.mojo`. Mojo owns overlay config-assignment extraction
for split and inline `-c`/`--config` forms, workspace-trust config selection,
`--remote`/`--no-daemon` transport flag classification, and fresh-Super
projection of config, feature, approval-bypass, and retained CLI arguments.
Rust retains TOML parsing/recursive merge/rendering, filesystem reads and
writes, `OsString` materialization, launch process ownership, and dynamic
feature/config value formatting. The previous Rust argv scanners and fresh
projection match loop were deleted; no Rust semantic fallback remains for the
migrated decisions.

Focused validation passes the standalone Mojo build, the real-Mojo
`runtime_overlay_policy` adapter test, the workspace-trust consumer test, two
`local_super` tests, two `private_companion` tests, the explicit-remote
transport test, and the hook-trust overlay test. Clippy with warnings denied for
`prodex-mojo-core`, `git diff --check`, and the expanded no-fallback guard also
pass. The canonical broad report at this checkpoint counts **71,154 reachable
Mojo LOC** and **200,195 Rust production LOC**, totaling **271,349 LOC**:
**26.222318858739115% Mojo**. The 75% broad target remains in progress.

## Operational log detail metadata hard replacement

Operational log detail metadata now lives with its existing ordering policy in
`observability_labels.mojo`. Mojo owns all 62 detail IDs plus each detail's
field key, rendered label, and plain/percent/endpoint format classification.
The Rust log-stream consumer no longer carries the duplicate
`OPERATIONAL_DETAIL_SPECS` table or its local format enum; it requests typed
metadata from `prodex-mojo-core` and retains only field lookup, redaction,
endpoint URL parsing, Unicode-safe bounding, and final string rendering. The
adapter caches the immutable 62-entry Mojo result after first load so the hot
log path does not cross the FFI once per detail on every event. No Rust semantic
fallback remains for operational detail metadata.

Focused validation passes the standalone `observability_labels.mojo` build,
the real-Mojo 62-entry adapter test, the `prodex-app`
`operational_event_summary_uses_mojo_detail_plan` consumer test, Clippy with
warnings denied for the `mojo-observability` adapter surface, `git diff
--check`, and the expanded no-fallback guard. The canonical broad report at
this checkpoint counts **71,454 reachable Mojo LOC** and **200,172 Rust
production LOC**, totaling **271,626 LOC**: **26.30602372379669% Mojo**. The
75% broad target remains in progress.

## Operational event source-label hard replacement

Operational event source labels now live with their existing classification in
`observability_labels.mojo`. Mojo maps the source ids produced by the event
classifier to the canonical request/MCP/agent/route/quota/retry/backoff/health/
error/model/upstream/stream/response/terminal/tool/load/smart/compact/event
labels, including the no-source case. Rust retains BTreeMap field acquisition,
typed plan materialization, and caller error plumbing only. The previous Rust
20-entry `SOURCES` table was deleted; no Rust semantic fallback remains for the
migrated source-label mapping.

Focused validation passes the standalone Mojo build, the real-Mojo source-label
adapter test, the representative `prodex-app` operational-event source consumer
test, Clippy with warnings denied for both affected library crates,
`git diff --check`, and the strengthened no-fallback guard. The canonical broad
report at this checkpoint counts **71,522 reachable Mojo LOC** and **200,207
Rust production LOC**, totaling **271,729 LOC**: **26.32107724975987% Mojo**.
The 75% broad target remains in progress.

## Quota display policy hard replacement

Quota display policy now runs through `quota.mojo`. Mojo owns report-sort
labels, standard quota-window classification and labels for 5h/weekly/monthly
windows, blocked quota status labels, and usage-auth sync source labels. Rust
retains enum/tag materialization, dynamic fallback rendering for arbitrary
`<seconds>s` windows, chrono timestamp formatting, Serde/BTreeMap ownership,
and terminal/report composition. The previous Rust sort-label match, window
threshold ranges, blocked-status match, and auth-sync source-label match were
deleted; no Rust semantic fallback remains for the migrated display decisions.

Focused validation passes the standalone Mojo quota build, the real-Mojo
quota-model policy adapter test, focused `prodex-quota` tests for standard
window labels, blocked OpenAI quota labels, and auth-sync source labels, Clippy
with warnings denied for both affected library crates, `git diff --check`, and
the strengthened no-fallback guard. The canonical broad report at this
checkpoint counts **71,653 reachable Mojo LOC** and **200,310 Rust production
LOC**, totaling **271,963 LOC**: **26.34659861819439% Mojo**. The 75% broad
target remains in progress.

## Provider precommit and credential-retry policy hard replacement

Provider precommit decision matrices now run through the existing
`provider_constraints.mojo` owner. Mojo owns provider health actions, provider
metric result classes, buffered and live fallback eligibility/class selection,
SSE prefetch eligibility, parsed-SSE progress actions, and the shared
credential-rotation retry budget/class policy used by OpenAI. Rust retains HTTP
and stream I/O, content-type/body acquisition, parsed SSE dynamic payload
materialization, provider-error classification calls, and execution of the
selected health/metric side effects. `ProviderErrorClass` is now explicitly
`repr(i64)` so its existing 0..5 ABI order remains stable. The former Rust
precommit decision matrices and OpenAI class/remaining-attempt retry match were
deleted; no Rust semantic fallback remains for those migrated decisions.

Focused validation passes the standalone `provider_constraints.mojo` build,
the real-Mojo provider scalar/precommit adapter test, `prodex-app --lib` check,
the focused OpenAI Mojo-backed credential-retry test, the generic-429
non-retry regression, Clippy with warnings denied for `prodex-mojo-core`,
`prodex-provider-core`, and `prodex-app`, `git diff --check`, and the expanded
no-fallback guard. The canonical broad report at this checkpoint counts
**71,813 reachable Mojo LOC** and **200,545 Rust production LOC**, totaling
**272,358 LOC**: **26.367134433356096% Mojo**. The 75% broad target remains in
progress.

## Native-first provider error-code classification hard replacement

Native-first SSE provider error codes now reuse the canonical
`provider_error.mojo` classifier. Rust retains SSE `data:` extraction, UTF-8 and
Serde JSON parsing, error-object field acquisition, and conversion of the Mojo-
backed provider classification into the local optional retry signal. The former
Rust match table for rate-limit, overload, not-found, authentication, and quota
error-code aliases was deleted. The canonical provider classifier was extended
with the native-first aliases (`rate_limit_error`, `not_found_error`,
`overloaded_error`, and `server_is_overloaded`), so there is no Rust semantic
fallback for the migrated code classification.

Focused validation passes the standalone `provider_error.mojo` build, the
provider-core Mojo classifier regression test, all 4 active native-first event
consumer tests in `prodex-app`, Clippy with warnings denied for
`prodex-provider-core` and `prodex-app`, `git diff --check`, and the strengthened
no-fallback guard. The canonical broad report at this checkpoint counts
**71,824 reachable Mojo LOC** and **200,528 Rust production LOC**, totaling
**272,352 LOC**: **26.37175420044648% Mojo**. The 75% broad target remains in
progress.

## Local-rewrite previous-response metadata hard replacement

Local-rewrite continuation lookup now reuses the canonical runtime request
metadata path. `runtime_local_rewrite_previous_response_id()` delegates to
`prodex-runtime-proxy::runtime_request_previous_response_id_from_bytes()`, whose
field selection, nonblank validation, and trimming are owned by
`runtime_proxy_request.mojo`. Rust retains JSON decoding into the canonical
request metadata bridge and the surrounding binding/state lookup. The duplicate
local Serde field lookup and trim/filter chain were deleted; no local Rust
semantic fallback remains for `previous_response_id` extraction.

Focused validation passes the local-rewrite wrapper regression, the canonical
runtime-proxy request-metadata test, Clippy with warnings denied for
`prodex-app`, `git diff --check`, and the strengthened no-fallback guard. The
canonical broad report at this checkpoint counts **71,824 reachable Mojo LOC**
and **200,519 Rust production LOC**, totaling **272,343 LOC**:
**26.372625696272713% Mojo**. The 75% broad target remains in progress.

## Native-first SSE prefetch eligibility hard replacement

Native-first SSE prefetch eligibility now runs through the existing
`provider_constraints.mojo` precommit owner. Mojo decides whether native
Anthropic-message mode, the HTTP status, SSE content type, and empty-prefix
state permit first-event prefetch. Rust retains header acquisition, semaphore
ownership, deadlines, stream reads, backlog preservation, and execution of the
prefetch/commit decision. The previous compound Rust eligibility condition was
deleted; no Rust semantic fallback remains for the migrated prefetch gate.

Focused validation passes the standalone `provider_constraints.mojo` build,
the real-Mojo provider scalar/precommit adapter test, all 4 active native-first
event consumer tests in `prodex-app`, Clippy with warnings denied for
`prodex-mojo-core` and `prodex-app`, `git diff --check`, and the strengthened
no-fallback guard. The canonical broad report at this checkpoint counts
**71,842 reachable Mojo LOC** and **200,546 Rust production LOC**, totaling
**272,388 LOC**: **26.374877013671675% Mojo**. The 75% broad target remains in
progress.

## Provider bridge metadata hard replacement

Static runtime provider-bridge metadata now lives in `provider_constraints.mojo`.
Mojo owns the rate-limit header prefix, human-facing rate-limit label,
chat-compatible adapter label, and function-tool name byte limit for all six
runtime bridge kinds. `RuntimeProviderBridgeKind` is now `repr(i64)` so the
existing bridge order is a stable Mojo ABI tag, which also removes the separate
Rust bridge-kind mapper used by precommit policy. Rust retains the typed
`ProviderId` mapping and execution/adapter boundaries. The four metadata match
tables and the duplicate precommit tag mapper were deleted; no Rust semantic
fallback remains for the migrated bridge metadata.

Focused validation passes the standalone `provider_constraints.mojo` build,
the real-Mojo provider scalar/precommit adapter test, the active provider-bridge
metadata consumer test, Clippy with warnings denied for `prodex-mojo-core` and
`prodex-app`, `git diff --check`, and the strengthened no-fallback guard. The
canonical broad report at this checkpoint counts **71,942 reachable Mojo LOC**
and **200,620 Rust production LOC**, totaling **272,562 LOC**:
**26.394728538827863% Mojo**. The 75% broad target remains in progress.

## Provider native-passthrough policy hard replacement

Provider bridge native-passthrough admission now runs through
`provider_constraints.mojo`. Mojo owns the unknown-route default, the OpenAI
Responses bridge exception, Models and Responses Compact exclusions, and the
final Native/Passthrough capability admission. `ProviderCapabilityStatus` is
now `repr(i64)` so the existing capability order is a stable ABI tag. Rust
retains route parsing/materialization, provider registry capability lookup, and
execution of the selected adapter path. The previous compound Rust passthrough
decision tree was deleted; no Rust semantic fallback remains for the migrated
admission policy.

Focused validation passes the standalone `provider_constraints.mojo` build,
the real-Mojo provider scalar/native-passthrough adapter test, the existing
provider native-passthrough matrix consumer test, Clippy with warnings denied
for `prodex-provider-core`, `prodex-mojo-core`, and `prodex-app`,
`git diff --check`, and the strengthened no-fallback guard. The canonical broad
report at this checkpoint counts **71,963 reachable Mojo LOC** and **200,647
Rust production LOC**, totaling **272,610 LOC**: **26.397784380616997% Mojo**.
The 75% broad target remains in progress.

## Runtime-log environment numeric policy hard replacement

Runtime-log numeric environment parsing now runs through the existing
`log_throughput_policy.mojo` retention owner. Mojo owns Rust-compatible unsigned
decimal parsing for retention env values, including optional leading `+`,
leading-zero acceptance, whitespace/sign rejection, overflow detection,
min/max admission, and fallback-to-default behavior. Rust retains `std::env`
lookup, target integer type conversion, filesystem locking/I/O, and retention
execution. The three Rust `parse::<u64>()` environment parsers were deleted; no
Rust semantic fallback remains for the migrated numeric policy.

Focused validation passes the standalone `log_throughput_policy.mojo` build,
the real-Mojo retention/throughput adapter test including exact parser edge
cases, all 17 `prodex-runtime-log` tests, Clippy with warnings denied for
`prodex-mojo-core` and `prodex-runtime-log`, `git diff --check`, and the
strengthened no-fallback guard. The canonical broad report at this checkpoint
counts **72,001 reachable Mojo LOC** and **200,661 Rust production LOC**,
totaling **272,662 LOC**: **26.40668666700897% Mojo**. The 75% broad target
remains in progress.

## Copilot quota display hard replacement

Copilot quota feature selection and rendering now run through `quota.mojo`.
Mojo owns the ordered feature keys (`chat`, `completions`), readiness/blocking
policy, status text, and complete `Main` quota rendering including optional
totals, separators, negative values, and the empty `-` state. Rust retains
BTreeMap ownership and lookup for the Mojo-selected keys, chrono reset-date
parsing, report/panel composition, and provider data acquisition. The previous
Rust feature-label table, blocked-feature planner, Ready/Blocked branch, and
main quota formatting pipeline were deleted; no Rust semantic fallback remains
for the migrated Copilot display policy.

Focused validation passes the standalone `quota.mojo` build, the real-Mojo
Copilot feature/display adapter tests, all 82 `prodex-quota` tests, Clippy with
warnings denied for `prodex-mojo-core` and `prodex-quota`, `git diff --check`,
and the strengthened no-fallback guard. The canonical broad report at this
checkpoint counts **72,202 reachable Mojo LOC** and **200,762 Rust production
LOC**, totaling **272,964 LOC**: **26.45110710569892% Mojo**. The 75% broad
target remains in progress.

## Gemini quota display hard replacement

Gemini quota display policy now runs through `quota.mojo`. Mojo owns bucket
label selection from model/token metadata, `models/` stripping, token-type
ASCII lowercasing, bucket summary rendering, exhausted/readiness aggregation,
minimum remaining-percent selection, status text, and complete main quota
rendering for percent, raw-amount, unknown, and empty states. Rust retains
provider JSON/Serde ownership, RFC3339 reset parsing via chrono, batch ABI
chunking, and report/panel composition. The previous Rust Gemini label,
blocked-bucket, summary, min-percent, status, and main-rendering branches were
deleted; no Rust semantic fallback remains for the migrated display policy.

Focused validation passes the standalone `quota.mojo` build, the real-Mojo
Gemini display adapter tests, all 82 `prodex-quota` tests, Clippy with warnings
denied for `prodex-mojo-core` and `prodex-quota`, `git diff --check`, and the
strengthened no-fallback guard. The canonical broad report at this checkpoint
counts **72,506 reachable Mojo LOC** and **200,864 Rust production LOC**,
totaling **273,370 LOC**: **26.52302739876358% Mojo**. The 75% broad target
remains in progress.

## Quota report sorting policy hard replacement

Quota report ordering now runs through `quota.mojo`. Mojo owns the Current,
Remaining, Profile, Auth, Account, and Plan sort-mode selection, active/status
rank ordering, reset-epoch ordering, Unicode-whitespace trimming plus ASCII
case-folded text comparison, and final comparator result. Rust precomputes one
typed sort record per report from provider-specific readiness/reset and view
data, then delegates every pairwise sort decision to Mojo; the stable name
tie-break remains Rust collection behavior. The previous Rust `match sort` and
`compare_text()` implementation were deleted; no Rust semantic fallback remains
for the migrated comparator.

Focused validation passes the standalone `quota.mojo` build, the real-Mojo
quota comparator adapter tests, the selected-column sort regression, the
current-sort blocked-window regression, Clippy with warnings denied for
`prodex-mojo-core` and `prodex-quota`, `git diff --check`, and the strengthened
no-fallback guard. The canonical broad report at this checkpoint counts
**72,623 reachable Mojo LOC** and **200,969 Rust production LOC**, totaling
**273,592 LOC**: **26.54427030030118% Mojo**. The 75% broad target remains in
progress.

## Quota workspace label policy hard replacement

Quota workspace label selection and shortening now run through `quota.mojo`.
Mojo owns workspace-name precedence, Rust-compatible Unicode whitespace
trimming, blank rejection, and Unicode-codepoint-safe workspace-id shortening
to 12 leading characters plus `...` plus 6 trailing characters when the
trimmed id exceeds 24 characters. Rust retains optional string ownership and
report composition only. The previous Rust trim/filter chain,
`short_workspace_id()`, `chars()` collection, and slicing formatter were
deleted; no Rust semantic fallback remains for the migrated workspace-label
policy.

Focused validation passes the standalone `quota.mojo` build, the real-Mojo
workspace-label adapter tests including Unicode trimming/truncation, the two
focused workspace report regressions, Clippy with warnings denied for
`prodex-mojo-core` and `prodex-quota`, `git diff --check`, and the strengthened
no-fallback guard. The canonical broad report at this checkpoint counts
**72,733 reachable Mojo LOC** and **201,013 Rust production LOC**, totaling
**273,746 LOC**: **26.569520650530052% Mojo**. The 75% broad target remains in
progress.

## Quota pool display policy hard replacement

Quota-pool display policy now runs through `quota.mojo`. Mojo owns Copilot main
remaining-percent selection across chat/completions totals, ready-pool summary
rendering, generic remaining-pool rendering, `Unavailable` behavior, window
separator/label formatting, profile-count text, and earliest-reset suffix
composition. Rust retains BTreeMap lookup of provider quota values, chrono reset
timestamp formatting, aggregate DTO ownership, and report field assembly. The
previous Rust Copilot percent calculation, ready-pool formatter, generic pool
formatter, and now-unused float-rounding wrapper were deleted; no Rust semantic
fallback remains for the migrated pool display policy.

Focused validation passes the standalone `quota.mojo` build, the real-Mojo
Copilot/pool display adapter tests, all 82 `prodex-quota` tests, Clippy with
warnings denied for `prodex-mojo-core` and `prodex-quota`, `git diff --check`,
and the strengthened no-fallback guard. The canonical broad report at this
checkpoint counts **72,922 reachable Mojo LOC** and **201,113 Rust production
LOC**, totaling **274,035 LOC**: **26.610469465579214% Mojo**. The 75% broad
target remains in progress.

## Audit query and display policy hard replacement

Audit query/display policy now runs through the existing
`audit_log_policy.mojo` owner. Mojo owns query filter-presence detection, exact
component/action/outcome matching, canonical filter-string rendering, bounded
search-scope rendering including saturating byte-range end calculation, and
Unicode-codepoint-safe detail truncation with ellipsis. Rust retains file
scanning/locking, path ownership, Serde JSON parsing/serialization, checksum
validation, and human output assembly around the Mojo-produced fragments. The
previous Rust filter conjunction, query formatter, search-scope formatter, and
`chars().take()` truncation implementation were deleted; no Rust semantic
fallback remains for the migrated query/display decisions.

Focused validation passes the standalone `audit_log_policy.mojo` build, the
real-Mojo audit policy adapter test including query/scope/Unicode truncation
contracts, all 17 `prodex-audit-log` tests, Clippy with warnings denied for
`prodex-mojo-core` and `prodex-audit-log`, `git diff --check`, and the
strengthened no-fallback guard. The canonical broad report at this checkpoint
counts **73,267 reachable Mojo LOC** and **201,306 Rust production LOC**,
totaling **274,573 LOC**: **26.683978395545083% Mojo**. The 75% broad target
remains in progress.

## Audit ledger metadata redaction hard replacement

Usage-ledger metadata text policy now runs through `audit_log_policy.mojo`.
Mojo owns Unicode whitespace trimming and 100-codepoint bounding for profile
names, account-id redaction to a `...` plus last-four-codepoint hint, and email
domain extraction using the final `@`, Unicode trimming, ASCII lowercasing, and
100-codepoint bounding. Rust retains optional-field ownership, Serde ledger
serialization, checksum/file I/O, and stores only the Mojo-produced redacted
metadata. The previous Rust profile `chars().take(100)`, reverse suffix
collection, and `rsplit_once('@')` domain transform were deleted; no Rust
semantic fallback remains for the migrated metadata policy.

Focused validation passes the standalone `audit_log_policy.mojo` build, the
real-Mojo metadata adapter tests including Unicode cases, all 17
`prodex-audit-log` tests, Clippy with warnings denied for `prodex-mojo-core` and
`prodex-audit-log`, `git diff --check`, and the strengthened no-fallback guard.
The canonical broad report at this checkpoint counts **73,463 reachable Mojo
LOC** and **201,357 Rust production LOC**, totaling **274,820 LOC**:
**26.73131504257332% Mojo**. The 75% broad target remains in progress.

## Audit budget evaluation reason policy hard replacement

Budget evaluation result policy now runs through `audit_log_policy.mojo`. Mojo
owns request/token/cost threshold evaluation, allowed/blocked state, ordered
reason selection, and exact reason rendering including current/max counters.
Rust retains selected-window summary materialization, configured limit DTOs,
key normalization, and serialization/API ownership, then only decodes the
Mojo-produced `allowed` flag and reason strings. The previous Rust flag branches
and `format!` reason pipeline were deleted; no Rust semantic fallback remains
for the migrated budget-evaluation decisions.

Focused validation passes the standalone `audit_log_policy.mojo` build, the
real-Mojo audit budget adapter tests, all 17 `prodex-audit-log` tests, Clippy
with warnings denied for `prodex-mojo-core` and `prodex-audit-log`,
`git diff --check`, and the strengthened no-fallback guard. The canonical broad
report at this checkpoint counts **73,555 reachable Mojo LOC** and **201,399
Rust production LOC**, totaling **274,954 LOC**: **26.751747565047243% Mojo**.
The 75% broad target remains in progress.

## Audit budget-window policy hard replacement

Budget-window start planning now runs through `audit_log_policy.mojo`. Mojo owns
Hour/Day/Week fixed-window selection, Rust-compatible Euclidean flooring for
positive and negative epochs with saturating `i64::MIN` behavior, and the
30-day fallback start used when calendar-month conversion is unavailable.
`BudgetWindow` is now `repr(i64)` for stable ABI tags. Rust retains only the
calendar-aware UTC month-start calculation through chrono and falls back to the
Mojo-produced 30-day boundary when chrono cannot materialize the timestamp. The
previous Rust `match self` fixed-window matrix and `floor_epoch()` helper were
deleted; no Rust semantic fallback remains for the migrated fixed-window
policy.

Focused validation passes the standalone `audit_log_policy.mojo` build, the
real-Mojo budget-window adapter tests including negative/extreme epochs, all 18
`prodex-audit-log` tests, Clippy with warnings denied for `prodex-mojo-core` and
`prodex-audit-log`, `git diff --check`, and the strengthened no-fallback guard.
The canonical broad report at this checkpoint counts **73,593 reachable Mojo
LOC** and **201,422 Rust production LOC**, totaling **275,015 LOC**:
**26.759631292838574% Mojo**. The 75% broad target remains in progress.

## Sub-agent reasoning compatibility hard replacement

Sub-agent explicit reasoning-effort compatibility now reuses the canonical
provider model reasoning resolver backed by Mojo `rich_catalog` policy. The app
passes the resolved/unknown model plus requested effort to
`provider_model_reasoning_resolution()` and only translates its typed
unsupported-effort result into the existing user-facing error. Unknown dynamic
models remain accepted, while catalogued models retain their scoped supported
effort contract. The previous Rust eight-way `SubAgentReasoningEffort` to
`ProviderReasoningEffort` mapper and `supported.contains()` compatibility branch
were deleted; no duplicate Rust semantic fallback remains.

Focused validation passes 3 active `prodex-app` resolver regressions including
unsupported catalogued and accepted dynamic models, the provider-core canonical
reasoning-resolution regression, Clippy with warnings denied for `prodex-app`,
`git diff --check`, and the strengthened no-fallback guard. The canonical broad
report at this checkpoint counts **73,593 reachable Mojo LOC** and **201,419
Rust production LOC**, totaling **275,012 LOC**: **26.759923203351125% Mojo**.
The 75% broad target remains in progress.

## Provider reasoning-effort label hard replacement

Canonical provider reasoning-effort labels now live in
`provider_constraints.mojo` beside the existing effort classifier. The Mojo
policy maps stable 0..8 effort tags to `none`/`minimal`/`low`/`medium`/`high`/
`xhigh`/`max`/`ultra` or no label for `Unknown`. `ProviderReasoningEffort` is now
`repr(i64)` and exposes the Mojo-owned label through a safe adapter. Provider
catalog reasoning and sub-agent effort suggestions reuse that label, with the
sub-agent side parsing it through its existing Mojo-owned effort parser. The
previous provider-core label match and app-level nine-way provider→sub-agent
effort mapping were deleted; no duplicate Rust semantic mapping remains.

Focused validation passes the standalone `provider_constraints.mojo` build,
the real-Mojo scalar/label adapter test, provider-core effort parsing and model
reasoning regressions, the sub-agent effort suggestion regression, Clippy with
warnings denied for `prodex-mojo-core`, `prodex-provider-core`, and
`prodex-app`, `git diff --check`, and the strengthened no-fallback guard. The
canonical broad report at this checkpoint counts **73,642 reachable Mojo LOC**
and **201,444 Rust production LOC**, totaling **275,086 LOC**:
**26.77053721381677% Mojo**. The 75% broad target remains in progress.

## Provider surface label policy hard replacement

Canonical provider surface labels now live in `provider_constraints.mojo`.
Mojo owns `ProviderId`, `ProviderWireFormat`, `ProviderEndpoint`, and
`ProviderCapabilityStatus` label tables behind stable `repr(i64)` ABI tags.
Rust keeps the public enums and typed call sites but no longer owns the
string-selection matches for provider ids, wire formats, endpoints, or
capability statuses. The previous four Rust label match tables were deleted; no
Rust semantic fallback remains for the migrated provider-surface metadata.

Focused validation passes the standalone `provider_constraints.mojo` build,
the real-Mojo provider scalar/surface-label adapter test, the provider-core
surface regression, Clippy with warnings denied for `prodex-mojo-core` and
`prodex-provider-core`, `git diff --check`, and the strengthened no-fallback
guard. After rebasing onto the released `0.435.1` catalog state, the canonical
broad report counts **73,750 reachable Mojo LOC** and **201,367 Rust production
LOC**, totaling **275,117 LOC**: **26.8067767531632% Mojo**. The 75% broad
target remains in progress.

## Websocket proxy string policy hard replacement

Websocket proxy string and NO_PROXY policy now lives in
`websocket_proxy_policy.mojo`. Mojo owns default port selection from the
websocket/HTTP scheme, Unicode-whitespace trimming and scheme-prefix insertion
for proxy URL candidates, NO_PROXY comma scanning, IPv4/hostname and bracketed
IPv6 host/port parsing with Rust-compatible unsigned-port semantics, exact and
domain-suffix matching, bracket normalization, and websocket authority
rendering. Rust retains socket address interleaving, URL/network parsing,
Basic-auth encoding, HTTP CONNECT I/O, and typed materialization. The previous
Rust trim/split/port parser, host matcher, normalization, and authority/URL
formatting branches were deleted; no Rust semantic fallback remains for the
migrated websocket proxy decisions.

Focused validation passes the standalone `websocket_proxy_policy.mojo`
build, the real-Mojo adapter contract test, all 10 focused
`prodex-runtime-proxy` websocket proxy regressions, Clippy with warnings denied
for `prodex-mojo-core` and `prodex-runtime-proxy`, `git diff --check`, and
the strengthened no-fallback guard. The canonical broad report at this
checkpoint counts **74,170 reachable Mojo LOC** and **201,548 Rust production
LOC**, totaling **275,718 LOC**: **26.900673876932228% Mojo**. The 75% broad
target remains in progress.

## Transport failure policy hard replacement

Transport-failure classification and display policy now live in
`transport_failure_policy.mojo`. Mojo owns the eleven canonical transport
failure labels, upstream-connect observability marker selection, ordered
case-insensitive message classification rules, and connect-vs-transport health
penalty weighting. `RuntimeTransportFailureKind` is now a stable `repr(i64)`
ABI tag. Rust retains the `std::io::ErrorKind` dependency boundary and typed
enum materialization only; the previous Rust message-rule table, label match,
marker match, and health-penalty match were deleted with no Rust semantic
fallback.

Focused validation passes the standalone `transport_failure_policy.mojo`
build, the real-Mojo adapter contract test, all 4 focused
`prodex-runtime-proxy` transport-failure regressions, Clippy with warnings
denied for `prodex-mojo-core` and `prodex-runtime-proxy`,
`git diff --check`, and the strengthened no-fallback guard. The canonical
broad report at this checkpoint counts **74,368 reachable Mojo LOC** and
**201,591 Rust production LOC**, totaling **275,959 LOC**:
**26.948930819433322% Mojo**. The 75% broad target remains in progress.

## Runtime launch notice rendering hard replacement

Runtime-launch scored-candidate notices, quota-preflight warnings, direct-provider
launch notices, and quota-inspection hints now render through
`info_render.mojo`. Mojo owns the four selected-profile status cases,
warning presence, exact selection text, provider/source notice text, and quota
inspection/bypass hint text. `prodex-terminal-ui` retains the public DTOs,
maps the typed status into the Mojo ABI tag, and materializes the returned
strings. The previous Rust `format!` branches and string literals were
deleted; the no-fallback guard now rejects restoration of those rendering
semantics in Rust.

Focused validation passes the real-Mojo terminal-ui runtime-launch tests (3),
the runtime-launch quota-plan caller tests (2), the `prodex-mojo-core`
info-render test, Clippy with warnings denied for `prodex-mojo-core`,
`prodex-terminal-ui`, and `prodex-runtime-launch`, the Mojo
authority and no-fallback guards, and `git diff --check`. An initial
`prodex-mojo-core` test invocation intentionally failed the build-script
precondition because `PRODEX_MOJO_REQUIRED=1` was supplied without a
Mojo subsystem feature; rerunning with `--features mojo-runtime` passed.
During the first boundary run, the test also caught a missing dispatcher arm for
the new ABI operations; the dispatcher was fixed before the passing validation.

The canonical broad report at this checkpoint counts **74,502 reachable Mojo
LOC** and **201,617 Rust production LOC**, totaling **276,119 LOC**:
**26.981844784314006% Mojo**. The release floor and ownership non-regression
checks pass; the 75% broad project target remains in progress.

## WebSocket executor observability-label hard replacement

WebSocket TCP-connect executor labels now use the existing
`observability_labels.mojo` owner. Mojo owns local-pressure class labels,
task labels, worker-thread prefixes, dispatcher-thread names, and enqueue,
dispatch, and reject overflow event names for both TCP-connect and DNS-resolve
tasks. The Rust enums are stable `repr(i64)` ABI tags and retain their
existing `&'static str` method contracts through cached Mojo adapter
results. The previous Rust match tables and string constants were deleted; no
Rust semantic fallback remains.

Focused validation passes the real-Mojo local-pressure round-trip regression,
the full task-kind observability-label regression, Clippy with warnings denied
for `prodex-mojo-core` and `prodex-runtime-proxy`, the Mojo
authority and no-fallback guards, and `git diff --check`. The runtime
proxy now enables the existing `mojo-observability` subsystem directly
so these production label calls cannot silently take a feature-off path.

The canonical broad report at this checkpoint counts **74,553 reachable Mojo
LOC** and **201,655 Rust production LOC**, totaling **276,208 LOC**:
**26.991615014771476% Mojo**. The release floor and ownership non-regression
checks pass; the 75% broad project target remains in progress.

## Runtime proxy observability-label hard replacement

Runtime proxy HTTP-error, quota, and WebSocket direct-fallback observability
labels now use the existing `observability_labels.mojo` owner. Mojo owns
HTTP error class/action labels; precommit quota-block reasons; quota pressure
band, window-status, and source labels; and direct-current WebSocket fallback
reason labels. The corresponding Rust enums now expose stable `repr(i64)`
tags where needed, while the Rust APIs preserve their existing
`&'static str` contracts through cached Mojo adapter results. The
WebSocket fallback planner also reuses that stable enum tag directly instead of
maintaining a second Rust match table.

The prior Rust string match tables were deleted, and the no-fallback guard now
rejects restoration of those semantic tables. Focused validation passes two
real-Mojo observability-label contract tests covering the HTTP and quota
families, the WebSocket direct-fallback label/planner regression, Clippy with
warnings denied for `prodex-mojo-core` and
`prodex-runtime-proxy`, the Mojo authority and no-fallback guards, and
`git diff --check`.

The canonical broad report at this checkpoint counts **74,626 reachable Mojo
LOC** and **201,652 Rust production LOC**, totaling **276,278 LOC**:
**27.011198864911428% Mojo**. The release floor and ownership non-regression
checks pass; the 75% broad project target remains in progress.

## Previous-response log-render hard replacement

The five previous-response production log formatters now assemble their output
in `log_semantics.mojo`. Mojo owns the request/session prefix variants,
transport/route inclusion, event text, blocked-vs-allowed fresh-fallback event,
profile/retry/detail placement, and optional `via` suffix. Rust retains
only typed field preparation and materialization that is not a competing
semantic renderer: `Option<&str>` debug materialization for the existing
`replay_turn_state` field and decimal materialization of the existing
`u128` delay value.

The previous Rust prefix/suffix helpers and string templates were deleted.
`prodex-mojo-core::log` exposes one typed render input over the new
Mojo ABI, and `prodex-runtime-proxy` enables the existing routing
feature required by that log adapter. The no-fallback guard rejects restoring
the deleted Rust message semantics.

Focused validation passes both existing previous-response log suites against the
real Mojo renderer, Clippy with warnings denied for `prodex-mojo-core`
and `prodex-runtime-proxy`, the Mojo authority and no-fallback guards,
and `git diff --check`. Development validation caught and fixed a Mojo
`UInt`/`UInt64` comparison mismatch, the missing
`mojo-routing` dependency feature, and an over-wide Rust adapter
signature before this checkpoint was accepted.

The canonical broad report at this checkpoint counts **74,955 reachable Mojo
LOC** and **201,753 Rust production LOC**, totaling **276,708 LOC**:
**27.088121774578255% Mojo**. The release floor and ownership non-regression
checks pass; the 75% broad project target remains in progress.

## Previous-response outcome labels and structured-log policy hard replacement

Previous-response fallback-shape, retry-reason, chain-reason, and
observability-outcome labels now use `observability_labels.mojo`. The
fallback-shape and route enums expose stable `repr(i64)` tags, removing
duplicate Rust tag-selection matches at the Mojo boundary. Rust still performs
validated enum reconstruction from Mojo outputs, but no longer owns the
canonical strings for these fields.

Structured runtime-log field policy now lives in `log_semantics.mojo`. Mojo
owns field skip/safe/free-form/stable-code/location classification, control
character sanitization and quote-required detection, plus URL/path location
secret stripping. Rust retains secret-like text redaction, typed field
materialization, and final structured-log assembly. The replaced Rust key
classifiers, stable-code checker, location scrubber, and quote predicate were
deleted rather than retained as a fallback.

The no-fallback guard requires the previous-response label families and the
structured-log Mojo ABI calls, and rejects restoration of the deleted Rust
semantics. Focused validation covers the previous-response outcome suite,
structured-log policy ABI contracts, structured-log quoting/redaction/location
caller regressions, Clippy with warnings denied for `prodex-mojo-core` and
`prodex-runtime-proxy`, the Mojo authority and no-fallback guards, and
`git diff --check`.

The canonical broad report at this checkpoint counts **75,310 reachable Mojo
LOC** and **201,796 Rust production LOC**, totaling **277,106 LOC**:
**27.177325644338268% Mojo**. The release floor and ownership non-regression
checks pass; the 75% broad project target remains in progress.

## Structured runtime-log policy hard replacement

Structured runtime-log field policy now runs through `log_semantics.mojo`.
Mojo owns field-key skip classification, the known-safe key set, free-form field
classification, stable-code validation, location-field classification,
control-character sanitization, quote-required decisions, query/fragment
stripping, and URL user-info redaction planning. Rust retains the
`redaction` crate boundary for secret-like text, Serde JSON escaping and
unescaping, borrowed/owned log DTOs, map materialization, and final string
assembly. The previous Rust key tables, stable-code scanner, location parser,
control-character mapper, and quote predicate were deleted; no Rust semantic
fallback remains.

Focused validation passes the direct real-Mojo structured-log policy regression,
the three structured-log runtime-proxy regressions, the typed log-event
round-trip regression, the production `prodex-app` structured-log
caller regression, Clippy with warnings denied for `prodex-mojo-core`
and `prodex-runtime-proxy`, the Mojo authority and no-fallback guards,
and `git diff --check`. The runtime-proxy dependency now directly
enables the Mojo routing feature needed by the log owner rather than relying on
feature unification elsewhere in the graph.

This landed in the same reviewed checkpoint as the previous-response
outcome-label hard replacement above. The canonical broad report for the
combined checkpoint counts **75,310 reachable Mojo LOC** and **201,796 Rust
production LOC**, totaling **277,106 LOC**: **27.177325644338268% Mojo**. The
release floor and ownership non-regression checks pass; the 75% broad project
target remains in progress.

## Route-reason label hard replacement

Canonical runtime route-decision reason labels now come from
`runtime_route_reason.mojo`. Mojo owns the 34 kind-to-label mappings,
and the same catalog is reused by exact label lookup so forward and reverse
mapping have one semantic owner. Rust keeps the public enum, validated tag
reconstruction, Serde integration, and `&'static str` materialization
through a cached Mojo adapter. The previous 34-entry Rust
`RuntimeRouteDecisionReasonKind::as_str()` match table was deleted with
no Rust fallback.

Focused validation passes the direct Mojo route-reason round-trip test across
all 34 labels, the runtime-proxy fixed label/stage caller regression, Clippy
with warnings denied for `prodex-mojo-core` and
`prodex-runtime-proxy`, the Mojo authority and no-fallback guards,
and `git diff --check`. Development validation caught two Mojo
`StringSlice` origin-typing errors in the initial helper design; the
final implementation avoids origin-dependent returns and keeps one internal
catalog used for both comparison and copying.

The canonical broad report at this checkpoint counts **75,472 reachable Mojo
LOC** and **201,807 Rust production LOC**, totaling **277,279 LOC**:
**27.218794066626035% Mojo**. The release floor and ownership non-regression
checks pass; the 75% broad project target remains in progress.

## Candidate skip-reason hard replacement

Ready/fallback candidate skip reasons and candidate quota-guard reason labels now
come directly from the existing `candidate_decision.mojo` plan. The
kernel already produced stable skip-reason tags; a new checked Mojo mapping
converts those tags to canonical route-reason kinds, and Rust materializes the
corresponding label through `runtime_route_reason.mojo`. The runtime
proxy now stores the Mojo-produced ready/fallback reasons instead of recomputing
them from availability and inflight state.

The duplicate Rust `RuntimeProfileAvailabilityState::skip_reason()`
policy, quota-guard literal mapper, runtime-proxy ready/fallback branches, and
the second app-level copy of those branches were deleted. `prodex-app`
only carries the already-produced reason values from
`prodex-runtime-proxy`; no Rust semantic fallback remains for this
selection explanation policy.

Focused validation passes 17 runtime-proxy selection-plan tests, 2 app
candidate-plan tests, Clippy with warnings denied for `prodex-mojo-core`,
`prodex-runtime-proxy`, and `prodex-app`, the Mojo authority
and no-fallback guards, and `git diff --check`.

The canonical broad report at this checkpoint counts **75,486 reachable Mojo
LOC** and **201,807 Rust production LOC**, totaling **277,293 LOC**:
**27.22246865229198% Mojo**. The release floor and ownership non-regression
checks pass; the 75% broad project target remains in progress.

## Route-affinity and chain log rendering hard replacement

Runtime route-affinity recompute/result/prefix logs, compact-followup owner logs,
and previous-response chain retry/dead-upstream logs now assemble in
`log_semantics.mojo`. Mojo owns HTTP-vs-WebSocket prefixes, presence
flags, event/field ordering, compact owner message selection and ordering,
optional dash defaults, compact owner source text, and complete output
templates. A single owner-log ABI call returns the ordered optional messages.
Rust retains typed DTO preparation, `u128` delay decimal materialization,
existing Rust Debug materialization for optional affinity state fragments, ABI
buffer allocation, and UTF-8 output materialization. Final runtime logging
remains a Rust host effect.

The former Rust prefix helper, five route-affinity format templates, chain
session/default helper, and both chain format templates were deleted. The
no-fallback guard requires all three Mojo log ABI renderers, rejects restoring
the old Rust templates, and self-tests those rejection paths.

Focused validation passes the 2 direct real-Mojo ABI contract tests, all 3
route-affinity caller regressions, both chain-log caller regressions, Clippy
with warnings denied for `prodex-mojo-core` and `prodex-runtime-proxy`,
`cargo fmt --all`, the Mojo authority guard, the no-fallback guard and its
self-test, and `git diff --check`. An initial filter ran 0 tests; `--list`
confirmed the actual names, then the focused binaries ran 2, 3, and 2 tests.
The guard self-test also exposed an outdated quota-label fixture, which now
includes the required Mojo calls. Earlier Mojo development caught and fixed an
initial Mojo 1.1 `Array[StringSlice]` indexing incompatibility.

The canonical broad report at this checkpoint counts **76,076 reachable Mojo
LOC** and **202,246 Rust production LOC**, totaling **278,322 LOC**:
**27.333807604141963% Mojo**. The release floor and ownership non-regression
checks pass; the 75% broad project target remains in progress.

## Governance inspection policy hard replacement

Governance inspection labels and deterministic validation now extend the
existing `governance_inspection.mojo` owner. Mojo owns data-classification
and coverage labels, conservative coverage merge, content-location path
validation, detector/tag/reason token validation, and configured inspection
limit validation, alongside the already-Mojo-owned finding classification
policy. Rust retains the public domain DTOs, Serde/debug behavior, byte-range
integer conversion, deterministic sorting/deduplication, and typed error
materialization.

The previous Rust classification/coverage string tables, coverage merge branch,
content-location byte predicate, token character predicate, and inspection-limit
bounds branch were deleted. `prodex-domain` delegates those decisions through
the checked `prodex-mojo-core` adapter; the no-fallback guard rejects
restoration of the removed Rust semantics.

Focused validation passes the direct real-Mojo governance policy contract test,
all 5 `prodex-domain` governance-inspection tests, Clippy with warnings
denied for `prodex-mojo-core` and `prodex-domain`, the Mojo
authority and no-fallback guards, and staged diff hygiene. A governance-only
canonical measurement was taken in a detached worktree from the current
checkpoint base so an unrelated in-progress runtime-log wave did not contaminate
the accounting.

The canonical broad report for this checkpoint counts **75,662 reachable Mojo
LOC** and **201,958 Rust production LOC**, totaling **277,620 LOC**:
**27.253800158490023% Mojo**. The release floor and ownership non-regression
checks pass; the 75% broad project target remains in progress.

## Smart Context, response metadata, doctor, and quota hard replacements

Smart Context duplicate-text planning now runs through the existing Mojo owner.
Rewrite mode uses digest buckets and confirms exact text equality before
replacing a duplicate, so a digest collision cannot discard distinct content.
Probe mode retains its bounded candidate limit. Rust keeps Serde traversal,
SHA-256 acquisition, and applying the returned index plan.

Runtime response metadata extraction now uses a checked Mojo JSON-tree plan for
response IDs, turn-state headers, event type, and token-usage fields. Rust keeps
Serde tree acquisition, number-text preservation, and typed DTO materialization.
The adapter and tests live in a private `json` submodule to stay below the
production size guard's cohesion threshold.

Runtime Doctor marker semantics now include a versioned ABI v2 failure-class
result. Mojo owns the marker-to-class mapping; Rust validates and materializes
the returned class. Quota auto-rotate eligibility now delegates to the existing
runtime-proxy Mojo planner instead of a second Rust status match.

Focused real-Mojo and caller validation passes: 116 `prodex-mojo-core` tests,
8 Smart Context planner cases, 1 Smart Context app caller case, 358
`prodex-runtime-proxy` tests, 28 `prodex-runtime-quota` tests, 39
`prodex-runtime-doctor` tests, and 1 response-metadata ABI test. The focused
Clippy run with warnings denied, Mojo authority and no-fallback guards,
`cargo fmt --check`, and `git diff --check` pass. The canonical production-share
check passes at **76,652 reachable Mojo LOC** and **202,361 Rust production
LOC**, or **27.47% Mojo**; the 75% project target remains unmet.

The size guard still reports existing files from the checkpoint base:
`prodex-mojo-core/src/log.rs` (989 lines),
`prodex-mojo-core/src/provider_constraints.rs` (869 lines),
`prodex-runtime-proxy/tests/src/error_policy.rs` (864 lines), and 33 near-limit
production files against the budget of 32. This checkpoint does not relax that
guard.

## Kiro streaming final-event selection hard replacement

The final Kiro Responses event type now comes from the existing Kiro Mojo
kernel. Rust retains response serialization and SSE framing, then sends the
Mojo-selected event unchanged. The old Rust status match was removed.

Validation passes: the strict real-Mojo final-event ABI test, the Kiro app
stream status caller test, all 367 `prodex-provider-core` tests, workspace
Clippy with warnings denied, `cargo fmt --check`, and the Mojo authority and
no-fallback guards. The broad 75% campaign objective remains in progress.

## Session report planning hard replacement

Session report updates, timestamp precedence and parsing, and stable report
ordering now use a checked Mojo ABI. Rust retains JSON parsing, DTO updates,
and local time formatting; the former Rust timestamp planner was removed.

Validation passes: 3 direct real-Mojo tests, all 55 `prodex-session-store`
tests, the affected Clippy run with warnings denied, formatting, documentation,
and Mojo authority/no-fallback guards. The project target is still measured by
the canonical production-share check.

## DeepSeek schema, history, and stream hard replacements

Strict function-schema acceptance now runs in Mojo before the existing schema
normalizer, and the Rust validator was removed. Mojo ABI v3 also selects the
first eligible history call ID and checks stored tool-call history without a
Rust scan. Empty DeepSeek text, refusal, and reasoning deltas are filtered in
the existing Mojo stream transform; Rust no longer repeats that decision.

Validation passes: direct real-Mojo schema and stream tests, all 368
`prodex-provider-core` tests, 103 app request-translation tests, the app SSE
regression, workspace Clippy, formatting, Mojo ownership and no-fallback
guards, and `git diff --check`. The 75% project target remains unmet.

## Gemini response and tool-order hard replacements

Gemini buffered-response status and completed stream tool-call item selection
now run in the existing Mojo response kernel. Rust keeps JSON acquisition,
argument normalization, SSE framing, and thought-signature handling. Gemini
function-response ordering now uses a stable Mojo permutation through the
provider-constraints owner, while Rust validates and applies the returned
indices.

Validation passes: direct real-Mojo response and ordering tests, all 374
`prodex-provider-core` tests, 51 Gemini SSE app tests, workspace Clippy,
formatting, and the Mojo authority/no-fallback guards. The 75% target remains
in progress.

## Sub-agent overlay and dry-run rendering hard replacement

The sub-agent overlay instructions and enabled/disabled dry-run reports now
render through a versioned v1 `sub_agent_policy` Mojo ABI. Rust retains model
and session-ID redaction, Markdown-safe display preparation, platform shell
quoting, and filesystem writes. The Rust text templates are removed, and
renderer errors propagate without a text fallback.

Validation passes: the direct real-Mojo ABI output and edge test, 17 app tests
matching `overlay_`, 11 app tests matching `dry_run_`, workspace Clippy with
warnings denied, `cargo fmt --all -- --check`, `npm run docs`, and the Mojo
no-fallback guard self-test and check. The canonical production-share check
reports **78,012 reachable Mojo LOC** and **202,546 Rust production LOC**, or
**27.81% Mojo**. The 7% release floor and ownership non-regression pass; the
75% project target remains unmet.

`npm run test:changed` passes churn hygiene, then stops at the repository-wide
size guard before changed-test execution. It reports seven existing violations,
including `prodex-mojo-core/src/log.rs` (989 lines against 850),
`prodex-mojo-core/src/provider_constraints.rs` (870 against 850),
`prodex-runtime-proxy/tests/src/error_policy.rs` (864 against 860), and 34
near-limit files against a budget of 32. No size guard was weakened.

## Profile health half-open timing hard replacement

The application no longer owns a duplicate half-open circuit backoff formula.
It delegates timing to the existing runtime-proxy Mojo adapter; Rust retains
route-key lookup and circuit state updates. Boundary coverage checks score
thresholds, saturation, and the Mojo ABI cap.

Validation passes: the direct real-Mojo profile-health ABI test (3 tests), 365
runtime-proxy app tests, `cargo fmt --all -- --check`, Mojo no-fallback guard
self-test and check, and `git diff --check`. The production-share check reports
**78,012 reachable Mojo LOC** and **202,533 Rust production LOC**, or **27.81%
Mojo**. The 7% release floor and ownership non-regression pass; the 75% project
target remains unmet.

## Sub-agent provider URL policy

CLI validation now calls the existing v1 `sub_agent_policy` operation for the
local-provider/URL-presence decision. Rust retains provider identity and URL
acquisition, URL syntax validation, and the existing CLI error text. The
no-fallback guard requires this Mojo call and rejects the removed Rust
predicates. Existing caller coverage checks both violation directions, keeps
top-level `--url` separate, and accepts a valid local sub-agent URL.

Validation passes: the focused CLI boundary test, the direct Mojo contract
test with all features, ownership and production-share checks, and the
no-fallback guard self-test and check. The canonical production-share report
shows 27.8052% Mojo; the 7% release floor and ownership non-regression pass,
while the 75% project target remains unmet. Formatting and diff hygiene are
checked for this checkpoint. `npm run test:changed` passes churn hygiene, then
stops at the existing size guard with seven violations and 34 near-limit files
against a budget of 32; changed-test selection does not run. No guard was
weakened.

## Runtime doctor marker-summary wave

`runtime_doctor_marker_summary.mojo` now owns bounded batch reduction of log
marker counts into the four selection totals and six failure-class totals. The
production consumer is `runtime_doctor_finalize_log_summary` in
`crates/prodex-runtime-doctor/src/diagnosis/final_summary/log_summary.rs`.
Rust retains log parsing and quota-floor reason-facet extraction. Selection
totals are reduced before the synthesized quota-floor marker is added; failure
totals are reduced after it is added, preserving the existing summary contract.

Validation passed:

- `cargo test --locked -q -p prodex-mojo-core --features mojo-rich --test runtime_doctor_markers marker_summary_counts_abi_tags_fixed_selection_and_failure_totals` (1 test).
- `cargo test --locked -q -p prodex-runtime-doctor --lib runtime_doctor_marker_summary_reducer_preserves_caps_synthesis_order_and_unicode` (1 test).
- `cargo clippy --locked -p prodex-mojo-core -p prodex-runtime-doctor --all-targets --all-features -- -D warnings`.
- `cargo fmt --check`, `git diff --check`, and `node scripts/ci/churn-hygiene.mjs --worktree --check`.
- `node scripts/ci/mojo-no-fallback-guard.mjs`, `node scripts/ci/mojo-ownership.mjs --check`, and `node scripts/ci/mojo-authority-guard.mjs`.
- `node scripts/ci/mojo-production-share.mjs --check`: 27.81% Mojo (78,073 Mojo LOC, 202,617 Rust LOC); the 7% release floor passes, and the 75% project target remains unmet.

## CLI default-run discovery

The `prodex` CLI now uses launch-arguments Mojo operation 13 to decide whether
an unrecognized first command defaults to `run`. Rust still projects the two
relevant `OsString` arguments into borrowed UTF-8 views and leaves Clap help,
version, command parsing, and error behavior unchanged. Operation 13 reuses the
versioned launch-arguments ABI and its existing production build path; the
ownership ledger and no-fallback guard now cover the new consumer and tests.

Validation passed:

- `PRODEX_MOJO_REQUIRED=1 cargo test --locked -p prodex-mojo-core --all-features --test launch_args default_cli_invocation_policy_uses_real_mojo_classification -- --exact` — 1 passed; `--list` confirmed all 6 launch-argument tests are present.
- `PRODEX_MOJO_REQUIRED=1 cargo test --locked -p prodex-cli --lib bare_invocation_defaults_to_run_and_clap_keeps_help_and_version` — 1 passed through the production caller. A temporary incorrect Mojo default made this same test fail with Clap's missing-command help; the Mojo source was restored and the test passed again.
- `cargo fmt --all -- --check`, workspace Clippy with warnings denied, `npm run docs`, the Mojo no-fallback self-test and check, Mojo ownership and authority checks, and `git diff --check` — passed.
- `node scripts/ci/mojo-production-share.mjs --check` — 27.83% Mojo; the 7% release floor passes and the 75% project target remains unmet.
- `npm run test:changed` stopped at the size guard before test selection: it reported 7 violations and 34 near-limit Rust files against the budget of 32. No tests ran under that command.

## Runtime-doctor compact-exit aggregation wave

`runtime_doctor_compact_exit_counts` in `runtime_doctor_marker_summary.mojo`
now owns the eleven compact-exit marker families, their legacy/current aliases,
and fixed output slots. Rust batches raw summary counts and projects the Mojo
slots into the existing diagnosis formatter. Filesystem and log parsing stay
in their host owners.

Validation passed:

- `PRODEX_MOJO_REQUIRED=1 cargo test --locked -p prodex-mojo-core --features mojo-rich --test runtime_doctor_markers -- --list` listed 6 tests; `PRODEX_MOJO_REQUIRED=1 cargo test --locked -p prodex-mojo-core --features mojo-rich --test runtime_doctor_markers -- --test-threads=1` passed 6/6, including all eleven compact-exit buckets, alias pairs, and the 257-marker batch boundary.
- The first caller filter used `--exact` with an unqualified name and ran 0 tests, so it was discarded. `PRODEX_MOJO_REQUIRED=1 cargo test --locked -q -p prodex-runtime-doctor --lib -- --list` listed the fully qualified production caller test and 39 other tests; `PRODEX_MOJO_REQUIRED=1 cargo test --locked -q -p prodex-runtime-doctor --lib -- --test-threads=1` then passed all 40.
- Sensitivity check: temporarily redirected the `committed` aliases to the adjacent output slot. The direct ABI test failed with the expected shifted totals; after restoring Mojo, it passed again.
- `cargo fmt --check`, `mojo format mojo/prodex_core/runtime_doctor_marker_summary.mojo`, `git diff --check`, and `node scripts/ci/churn-hygiene.mjs --worktree` passed; 543 changed lines, maximum changed file 133 lines.
- The Mojo no-fallback self-test and check, `npm run mojo:ownership`, and `npm run mojo:authority` passed.
- `cargo clippy --locked -p prodex-mojo-core -p prodex-runtime-doctor --all-targets --all-features -- -D warnings` passed.
- `npm run docs` passed. `npm run test:changed` stopped at the existing repository-wide size guard before test selection: seven violations and 34 near-limit files against a budget of 32. No guard was weakened.
- `node scripts/ci/mojo-production-share.mjs --check`: 27.85% Mojo (78,228 Mojo LOC, 202,626 Rust LOC), up about 0.02 percentage points from the previous checkpoint. The 7% release floor passes; the 75% project target remains unmet.
- Checkpoint commit `fd6767e6c` was pushed to `origin/main`.

## Profile-name and secret-reference policy hard replacements

Profile-name fallback and suffix candidate formatting now use the existing
`profile_identity.mojo` planner. API-key and Claude login share the same
profile-identity adapter; Rust retains profile-state and filesystem availability
checks through the caller callback. The new `secret_policy.mojo` kernel validates
`SecretRef` provider, name, and optional version metadata as nonempty, printable
ASCII components of at most 128 bytes. Secret storage and resolution remain in
Rust, and neither path recomputes a Rust policy result after a Mojo error.

Validation passed: the domain suite (26 tests), secret-store suite (56 tests),
Mojo-core all-features suite (207 tests), app profile-name caller test,
workspace and focused app Clippy with warnings denied, formatting, size and
churn guards, Mojo ownership/authority/no-fallback guards, and diff hygiene.
The canonical production-share check
reports **79,571 Mojo LOC** and **203,726 Rust LOC**, or **28.09% Mojo**. The 7%
release floor and ownership non-regression pass; the 75% campaign target remains
in progress.

## Runtime-doctor request-timeline detail wave

Request-timeline detail formatting now uses operation 22 in the existing
runtime-doctor Mojo renderer. Mojo owns the fixed field order, five-field cap,
48-Unicode-scalar truncation, and `key=value` joining; Rust projects the parsed
field map into bounded ABI slots and keeps event assembly.

Direct real-Mojo and runtime-doctor caller tests pass, including the 48/49
Unicode-scalar boundary, omitted fields, field order, and the five-field cap.
The existing parser regression for timeline caps and Unicode also passes.
Workspace Clippy, Rust formatting, Mojo no-fallback/authority/ownership guards,
targeted Markdown and runtime-policy checks, diff hygiene, and staged churn
hygiene pass. The production-share check reports **79,641 Mojo LOC** and
**203,712 Rust LOC**, or **28.11% Mojo**. The 7% floor and Mojo ownership
non-regression pass; the 75% campaign target remains in progress.

## Runtime-doctor last-marker line truncation wave

Operation 23 in `runtime_doctor_render.mojo` now owns the 160-Unicode-scalar
cap and ellipsis for `last_marker_line`. The shared bounded-text helper also
preserves operation 22's request-timeline behavior. Rust keeps log redaction
and trimming, then passes the borrowed text through the existing versioned
renderer ABI; the Rust truncation loop is gone.

Direct Mojo ABI coverage and the production caller test pass at the 160/161
Unicode-scalar boundary. The existing request-timeline and marker-summary
regressions pass after sharing the bounded-text helper. Rust formatting,
workspace Clippy, Mojo no-fallback/authority/ownership guards, targeted
Markdown/runtime-policy checks, and diff hygiene pass. The canonical report
shows **79,661 Mojo LOC** and **203,712 Rust LOC**, or **28.11% Mojo**; the 7%
floor and ownership non-regression pass, while the 75% target remains unmet.

An initial attempt to run the marker-summary regression as `--test parsing`
failed because the crate has no integration-test target by that name. The test
is mounted as a library unit test; the corrected `--lib` command passed.

## Runtime-doctor plan input-validation wave

The main plan, summary plan, state plan, and route plan ABIs now own their
fixed-layout input validation in Mojo. The summary validator checks every one
of its 128 marker-count slots before reduction. Rust no longer repeats those
range decisions; it keeps input record construction, output validation, and
typed ABI error mapping.

Validation passed:

- `PRODEX_MOJO_REQUIRED=1 cargo test --locked -q -p prodex-mojo-core --all-features input_contracts_are_validated_by_mojo` — 1 passed across all four ABIs.
- `PRODEX_MOJO_REQUIRED=1 cargo test --locked -q -p prodex-mojo-core --all-features` — 132 unit tests and all integration targets passed.
- `PRODEX_MOJO_REQUIRED=1 cargo test --locked -q -p prodex-runtime-doctor --lib` — 42 passed.
- `cargo fmt --all -- --check` and `PRODEX_MOJO_REQUIRED=1 cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` — passed.
- `node scripts/ci/mojo-no-fallback-guard.mjs --self-test`, `node scripts/ci/mojo-no-fallback-guard.mjs`, `node scripts/ci/mojo-authority-guard.mjs`, and `node scripts/ci/mojo-ownership.mjs --check` — passed; ownership reports 144 authoritative operations.
- `node scripts/docs/lint-markdown.mjs migration/mojo-75-campaign.md`, `node scripts/docs/runtime-policy.mjs --self-test`, `node scripts/docs/runtime-policy.mjs --check`, and `git diff --check` — passed.
- `node scripts/ci/mojo-production-share.mjs --check` — 79,664 Mojo LOC and 203,557 Rust LOC, or **28.13% Mojo**. The 7% floor and ownership non-regression pass; the 75% target remains unmet.

The adapter source was added after the frozen ownership baseline, so this wave
claims no frozen semantic LOC credit. The canonical broad-source count records
the reduction in current Rust production LOC directly.

## Governance inspection collection ordering migration

Finding ordering and tag/reason-code sorting and deduplication now run through
the versioned ABI in the existing `governance_inspection.mojo` kernel. Mojo
returns validated index plans and enforces input caps of 256 findings and 32
tags or reason codes before sorting. Rust projects typed keys and applies the
returned indices; the Rust sort/dedup implementation was removed. Caller tests
verify the exact finding key order and metadata deduplication. The direct ABI
test covers each cap and rejects cap-plus-one inputs, including duplicate
metadata.

Validation passed:

- `mojo format mojo/prodex_core/governance_inspection.mojo` — unchanged; `cargo fmt --all -- --check` passed.
- `PRODEX_MOJO_REQUIRED=1 cargo test --locked -q -p prodex-mojo-core --all-features` — 133 unit tests and all integration targets passed.
- `PRODEX_MOJO_REQUIRED=1 cargo test --locked -q -p prodex-domain` and `PRODEX_MOJO_REQUIRED=1 cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` — passed.
- The Mojo no-fallback guard and self-test, authority guard, and ownership check passed; ownership reports 145 authoritative operations.
- `node scripts/ci/mojo-production-share.mjs --check` — 79,961 Mojo LOC and 203,719 Rust LOC, or **28.19% Mojo**. The 7% release floor and ownership non-regression pass; the 75% project target remains unmet.
- `git diff --check` passed for the wave.

## Secret rotation bounds migration

`SecretRotationPolicy::validate` now delegates its maximum-age and overlap
decision to the existing `secret_policy.mojo` kernel. Mojo preserves zero-age
error precedence and compares the full `u64` range. Rust keeps the public
domain error enum and maps the validated decision tag; no Rust bounds check
remains.

Validation passed:

- `mojo format mojo/prodex_core/secret_policy.mojo` and `cargo fmt --all -- --check`.
- `PRODEX_MOJO_REQUIRED=1 cargo test --locked -q -p prodex-domain` and `PRODEX_MOJO_REQUIRED=1 cargo test --locked -q -p prodex-mojo-core --all-features`.
- `PRODEX_MOJO_REQUIRED=1 cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`.
- The no-fallback guard and self-test, authority guard, and ownership check pass; ownership reports 146 authoritative operations.
- `node scripts/ci/mojo-production-share.mjs --check` — 79,996 Mojo LOC and 203,764 Rust LOC, or **28.19% Mojo**. The 7% release floor and ownership non-regression pass; the 75% target remains unmet.

## Gemini internal-instruction leak detection migration

Mojo now owns Gemini internal-instruction leak classification and sanitation,
system-instruction corpus normalization, and bounded eight-word echo detection
behind ABI v1. Rust retains JSON traversal, text ownership, and ABI error
mapping. The original prefix and marker catalog remains exact in the Mojo
catalog module; source comparison confirmed all 160 prefixes and 60 original
markers in order, plus eight distinct helper markers and three safe-status
markers. The Rust classifier and echo-window implementation are gone.

Validation passed:

- `mojo format mojo/prodex_core/gemini_internal_instruction.mojo mojo/prodex_core/gemini_internal_instruction_catalog.mojo` and `cargo fmt --all -- --check`.
- `PRODEX_MOJO_REQUIRED=1 cargo test --locked -q -p prodex-provider-core gemini_bridge::leaks::tests -- --test-threads=1` — 4 passed; the complete `prodex-provider-core` library suite passed 371 tests.
- `PRODEX_MOJO_REQUIRED=1 cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` passed.
- The Mojo no-fallback guard and self-test, authority guard, and ownership check pass; ownership reports 147 authoritative operations.
- `node scripts/ci/mojo-production-share.mjs --check` — 80,663 Mojo LOC and 203,573 Rust LOC, or **28.38% Mojo**. The 7% release floor and ownership non-regression pass; the 75% project target remains unmet.
- Targeted Markdown lint and `git diff --check` pass.

## Live log-throughput state policy expansion

The existing `log_throughput_policy.mojo` owner now decides accepted throughput
observations, duplicate live/disk replay classification, candidate ordering,
strict sample freshness, bounded-state insertion thresholds, and finish-rate
precedence. Completed and streaming rate arithmetic remain Mojo-owned. Rust
keeps event and path projection, `Instant` timestamps, `VecDeque` samples,
profile maps, and TUI state. The state ABI adapter and its direct tests live in
private `log_throughput_policy/state.rs`; the parent module re-exports the
existing public call paths.

The bounded-state kernel decides when an eviction is required and owns the 64
stream/256 observation limits. Rust still selects `BTreeMap::keys().next()` as
the victim because stream keys contain native `PathBuf` values whose ordering
is platform-specific; Rust preserves that ordered-map behavior and applies the
eviction.

Validation passed:

- `mojo format mojo/prodex_core/log_throughput_policy.mojo`, `cargo fmt --all -- --check`, and `git diff --check`.
- `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0 cargo test --locked -p prodex-mojo-core --features mojo-core --lib log_throughput_policy::state::tests::throughput_state_policy_preserves_replay_freshness_order_and_bounds -- --exact` — 1 passed on restored Mojo source.
- The same strict test failed at `assertion failed: !sample_expired(2_000_000_000).unwrap()` after a temporary mutation changed the strict `>` freshness cutoff to `>=`; the original source bytes were restored exactly. SHA-256 before and after restoration: `32044bc181b2e0c54eae8a73cfb1f90253ab68e32508023ac908fc6bd1872bef`.
- `cargo test --locked -q -p prodex-app --lib log_throughput -- --test-threads=1` — 17 passed. The separately moved freshness-boundary test passed 1/1 in the app test module.
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` passed.
- Size guard, Mojo no-fallback self-test/check, authority guard, ownership check, and production-share check passed; ownership reports 152 authoritative operations.
- `node scripts/ci/mojo-production-share.mjs --check` — 80,944 Mojo LOC and 203,776 Rust LOC (28.43%). The 7% release floor and Mojo non-regression pass; the 75% project target remains unmet.

## Kiro chat-message prompt construction migration

Kiro prompt construction now runs as one complete transform in operation 53 of
the existing Kiro kernel. Mojo owns recursive string/array/object extraction,
`text`/`content`/`output` precedence, Unicode whitespace trimming, role labels,
tool-call formatting, and nonempty section joining, including the historical
`User:\n` empty result. The existing legacy function-tool and tool-choice
operations (IDs 6 and 7) now receive canonical JSON and own their validation,
Unicode trimming, defaults, and output shapes. Rust retains JSON
serialization/materialization and the bounded ABI call; the prompt parser,
formatter, and legacy conversion logic were removed. The live
`local_rewrite_kiro` path still calls the Mojo-backed provider wrapper. The
kernel operation was added without changing the Kiro ABI record layout.

Direct ABI and provider caller assertions cover nested content precedence,
Unicode trimming, blank-text suppression, unknown/default roles, malformed
tool-call fields, empty sections, legacy name/description trimming, and invalid
legacy shapes. A sensitivity proof changed the empty fallback predicate from
`sections == 0` to `sections > 0`; the strict ABI test then returned an empty
string instead of `User:\n` and failed. Restoring the original bytes produced
the same SHA-256 before and after: `d912707ad0c47077ca1781e9364c668d5e793872ccefbc4522f8ab6709636e4e`.

Validation passed:

- `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0 cargo test --locked -q -p prodex-mojo-core --features mojo-rich --test kiro_prompt` — 2 passed, including after restoring the mutation.
- `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0 cargo test --locked -q -p prodex-provider-core kiro_provider_core_maps_chat_messages_and_legacy_functions` — 1 passed.
- `npm run test:changed` — Mojo core tests passed; provider-core passed 371 unit tests and 7 integration tests.
- `PRODEX_MOJO_REQUIRED=1 cargo test --locked -q -p prodex-app --lib local_rewrite_tests::kiro_chat_completions_route_reuses_responses_translation_surface -- --test-threads=1` — 1 passed through the live local Kiro route.
- `cargo clippy --locked -q -p prodex-provider-core -p prodex-mojo-core --all-targets --all-features -- -D warnings`, `cargo fmt --all -- --check`, `npm run docs`, and `git diff --check` passed.
- Mojo no-fallback self-test and guard, authority guard, and ownership check passed; combined ownership reports 153 authoritative operations.
- `node scripts/ci/mojo-production-share.mjs --check` — 81,306 Mojo LOC and 203,673 Rust LOC, or **28.53% Mojo**. The 7% release floor and ownership non-regression pass; the 75% project target remains unmet.

## Provider usage SSE observation merge migration

The existing `provider_usage.mojo` kernel now owns a versioned latest-present
merge for input, output, and total token observations. Each field updates when
the new parsed event summary supplies a value and retains its prior value when
the field is absent. Rust continues UTF-8 validation, SSE framing and
`[DONE]` handling, malformed-frame rejection, `serde_json` event parsing, and
typed DTO materialization. The provider-core dependency already enables the
`mojo-provider-constraints` build feature that selects this Mojo source; strict
tests confirm the new export links through that production graph.

Direct Mojo adapter and provider caller tests cover partial updates across
events, malformed and non-usage frames, `[DONE]`, and empty streams. A
sensitivity proof changed the operation to keep prior present values even
when later values were present; the direct strict Mojo test failed with the
expected input/output/total mismatch. Restoring the original source bytes
produced the same SHA-256 before and after:
`6f25c8d0ed2ada88b425991c21c2b47191e15934abf4dd2d56b6979e27bd3236`.

Validation passed:

- `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0 cargo test --locked -q -p prodex-mojo-core --features mojo-provider-constraints --lib provider_usage::tests -- --test-threads=1` — 2 passed; the focused latest-present test passed again after mutation restoration.
- `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0 cargo test --locked -q -p prodex-provider-core` — 373 unit tests and 7 integration tests passed.
- `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0 cargo clippy --locked -q -p prodex-provider-core -p prodex-mojo-core --all-targets --all-features -- -D warnings` passed.
- `npm run test:changed`, `npm run docs`, `cargo fmt --all -- --check`, `git diff --check`, and `node scripts/ci/size-guard.mjs` passed.
- Mojo no-fallback self-test/guard, authority guard, and ownership check pass; ownership reports 154 authoritative operations.
- `node scripts/ci/mojo-production-share.mjs --check` — 81,354 Mojo LOC and 203,725 Rust LOC (28.54% Mojo); the 7% release floor and Mojo non-regression pass. The 75% project target remains unmet.

## Quota reset-epoch precedence migration

`mojo/prodex_core/quota.mojo` now owns JSON reset-epoch precedence through the
typed `prodex_quota_reset_epoch_v1` ABI. Rust retains Serde parsing, duplicate
key resolution, typed candidate extraction, case-insensitive header lookup,
and signed numeric/string conversion. Mojo selects the first valid top-level
`resets_at`, `reset_at`, `error.resets_at`, or `error.reset_at`; absent those,
it honors primary and secondary used-percent gates, including a missing gated
reset, then falls back from primary to secondary. The existing textual reset
parser stays unchanged.

Direct real-Mojo tests and `prodex-quota` caller tests cover candidate order,
missing gated values, 100% thresholds, negative epochs, numeric strings,
malformed and non-object JSON, case-variant header names, and Serde's
last-duplicate-key behavior.

Validation passed:

- `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0 cargo test --locked -q -p prodex-mojo-core --features mojo-quota --test quota_reset_epoch` — 2 direct real-Mojo tests passed.
- `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0 cargo test --locked -q -p prodex-quota quota_reset` — 7 focused caller tests passed; the full changed-test run passed all 87 `prodex-quota` tests.
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --all -- --check`, `npm run docs`, and `npm run test:changed` passed. The changed-test run also passed 371 provider unit tests and 7 provider integration tests.
- `node scripts/ci/mojo-no-fallback-guard.mjs --self-test`, `node scripts/ci/mojo-authority-guard.mjs`, `node scripts/ci/mojo-ownership.mjs --check`, `node scripts/ci/size-guard.mjs`, and `git diff --check` passed; ownership reports 154 authoritative operations.
- The primary gate was mutated from `>= 100` to `> 100`; the direct test failed on the 100% case as expected. The original Mojo source was restored byte-for-byte, with SHA-256 `f50493c4ff9f58e5260533260c1cf9511e77d791ce20fa633b591fb09b8d4127` before and after.
- `node scripts/ci/mojo-production-share.mjs --check` — 81,355 Mojo LOC and 203,718 Rust LOC, or **28.54% Mojo**. The 7% release floor and ownership non-regression pass; the 75% project target remains unmet.

## Live runtime-log record bounding migration

The focused `live_log_record.mojo` kernel now owns the live-record byte limit,
nested JSON string clip boundaries, compact metadata fallback selection, and
plain-text truncation prefix and tail. It preserves the 128 KiB record limit,
8 KiB nested string limit, selected `timestamp`/`pid`/`event` fields, and exact
truncation markers. Mojo writes the plain-text tail into a caller-bounded output
buffer and reports insufficient capacity.

Rust keeps `serde_json` parsing, recursive value traversal, string ownership,
JSON serialization/materialization, and live-store mutex/state. Rust applies
each checked Mojo string boundary and uses the Mojo JSON plan to materialize the
selected fallback fields. A Mojo error propagates from the live-store append;
the disk logger still attempts to write the original line, with no Rust
truncation policy fallback.

Validation passed:

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/live_log_record_test.mojo`.
- `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0 cargo test --locked -q -p prodex-runtime-log --lib` — 23 passed.
- `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0 cargo test --locked -q -p prodex-mojo-core --features mojo-runtime --lib live_log_record::tests -- --test-threads=1` — 2 passed.
- A mutation changed the Mojo record limit from 128 KiB to 127 KiB. The direct test failed at the exact-boundary assertion. Restoration matched byte-for-byte; SHA-256 before and after was `a324b159574d094734903dc4b6b3c46a0f6e9a4856c7934110bd7ea762130831`.
- `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0 cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` and `cargo fmt --all -- --check` passed.
- `npm run docs` and `npm run test:changed` passed. `node scripts/ci/mojo-no-fallback-guard.mjs --self-test`, the no-fallback guard, `node scripts/ci/mojo-authority-guard.mjs`, `node scripts/ci/mojo-ownership.mjs --check`, and `node scripts/ci/size-guard.mjs` passed; ownership reports 157 authoritative operations.
- `node scripts/ci/mojo-production-share.mjs --check` — 81,456 Mojo LOC and 203,809 Rust LOC, or **28.55% Mojo**. The 7% release floor and Mojo non-regression pass; the 75% project target remains unmet.

## Codex model-provider source selection migration

The `prodex-config` crate contains governance data types and defaults, with no
config parser or standalone decision kernel to migrate. `prodex-codex-config`
now routes model-provider source selection through the versioned
`prodex_codex_config_model_provider_plan_v1` ABI in the existing
`codex_config.mojo` owner. Mojo directs profile-v2 and base-config reads,
selects the normalized CLI override or parsed config candidate, and returns its
source. Rust keeps file access, TOML parsing, profile-path construction,
plan-directed reads, typed source mapping, and provider string ownership.
Provider-value normalization and OpenAI identity continue through their
existing Mojo operations.

The config consumer regression covers CLI-over-profile precedence while a
malformed base config remains unparsed when the profile config supplies a
provider. Direct required-Mojo tests cover read ordering, all three sources,
normalization, and the no-provider case. A mutation inverted CLI-over-profile
precedence; both the direct ABI test and consumer test failed as expected. The
Mojo source was restored byte-for-byte; SHA-256 before and after was
`d7833c5d05523c0e7768e6bd29b6ea859d066a3cc731ec30480374399dbbcf89`.

Validation passed:

- `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0 cargo test --locked -q -p prodex-config -p prodex-codex-config` — 20 Codex config tests and 3 governance config tests passed.
- `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0 cargo test --locked -q -p prodex-mojo-core --features mojo-runtime --lib codex_config::tests -- --test-threads=1` — 2 direct ABI tests passed.
- `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0 cargo clippy --locked -q -p prodex-config -p prodex-codex-config -p prodex-mojo-core --all-targets -- -D warnings` and `cargo fmt --all -- --check` passed.
- `npm run docs`, `npm run test:changed -- --base HEAD`, and `git diff --check` passed. Plain `npm run test:changed` stopped before tests because its inherited `origin/main..HEAD + worktree` range contains 1,892 lines from earlier committed campaign changes; this worktree measures 410 changed lines, with a 132-line largest file.
- `node scripts/ci/mojo-no-fallback-guard.mjs --self-test && node scripts/ci/mojo-no-fallback-guard.mjs`, `node scripts/ci/mojo-authority-guard.mjs`, `node scripts/ci/mojo-ownership.mjs --check`, `node scripts/ci/size-guard.mjs`, and `node scripts/ci/mojo-production-share.mjs --check` passed. Ownership reports 160 authoritative operations; the source report remains at **28.58% Mojo**, above the 7% release floor.

## Update-notice release-version migration

`update_notice_policy.mojo` now owns fetched-release validation and both release
comparison contracts. It trims the same Unicode whitespace as Rust `str::trim`,
accepts one optional lowercase `v`, validates strict SemVer identifiers and
`u64` core components, and compares prerelease/build identifiers. Notice
comparison preserves `semver::Version` total ordering, including build metadata;
update decisions use SemVer precedence and ignore build metadata. Rust retains
GitHub redirect/tag acquisition, version strings, cache/network IO, and typed
result mapping. Invalid UTF-8 is rejected at the ABI boundary, and Mojo errors
propagate without Rust recomputation. The existing ABI caps each input at
1 MiB; larger version strings are rejected as invalid input.

The required-Mojo ABI and `prodex-update-notice` caller tests cover equality,
numeric and prerelease ordering, build metadata, leading zeros, overflow,
missing components, extra `v`, Unicode whitespace, malformed text, and invalid
UTF-8. They also verify the existing 1 MiB input boundary. The ownership ledger
records the new release-version operation; the no-fallback guard requires the
production Mojo calls and propagating error path.

Validation passes:

- The required-Mojo `prodex-mojo-core` ABI tests passed (4); all `prodex-update-notice` caller tests passed (11).
- A mutation reversed minor-version ordering. The ABI test failed with `Less` instead of `Greater`; restoring the Mojo source returned SHA-256 `0c1995227cbd29426bc19b5e236cb530683dd830f7a389eafa0a8165ce78879c`.
- `npm run test:changed -- --base origin/main`, workspace Clippy with all targets/features, `cargo fmt --all -- --check`, `npm run docs`, and `git diff --check` passed.
- The no-fallback guard and self-test, authority guard, ownership check, and size guard passed. Ownership reports 161 authoritative operations.
- `node scripts/ci/mojo-production-share.mjs --check` reports 82,068 Mojo LOC and 204,312 Rust LOC (**28.66% Mojo**); the 7% release floor and ownership non-regression pass. The 75% project target remains in progress.
## Session usage-limit marker classification migration

The resume monitor's deterministic usage-limit marker classifier now calls
operation 15 of the existing rich runtime-error kernel in
`rich_fallback.mojo`. Mojo owns exact legacy-marker matching, event/error
context rules, structured quota-code and message classification, ignored
conversation-object filtering, and the bounded 2,048-value scan. Rust retains
Serde JSON parsing and canonicalization, session file reads, session identity,
workflow evidence, and retry orchestration. The rich ABI rejects inputs above
the existing 64 MiB session-record ceiling and returns errors directly; no Rust
classification fallback remains.

The removed Rust block contains 156 non-empty semantic lines in the active
monitor module. That module was introduced after the frozen ownership baseline,
so this cleanup earns no frozen-volume credit. The existing `runtime_error_policy` export carries the
new operation ID without changing the ABI layout. Direct required-Mojo tests
cover exact and structured markers, ignored conversation text, and both sides
of the 2,048-value boundary. App caller regressions cover marker recognition
and post-child profile rotation.
- `PRODEX_MOJO_REQUIRED=1 cargo clippy --locked -p prodex-mojo-core -p prodex-provider-core --all-targets --all-features -- -D warnings`, `cargo fmt --all -- --check`, and `git diff --check` passed.
- Mojo no-fallback self-test and guard, authority guard, and ownership check passed; ownership reports 148 authoritative operations.
- `node scripts/ci/mojo-production-share.mjs --check` — 81,025 Mojo LOC and 203,351 Rust LOC, or **28.49% Mojo**. The 7% release floor and ownership non-regression pass; the 75% project target remains unmet.

## Smart Context memory-capsule ordering migration

Mojo now owns required-first ordering and optional relevance-descending,
token-cost-ascending, ID-ascending ordering through
`prodex_mojo_smart_context_capsule_order_v1`. The stable merge sort compares
borrowed UTF-8 ID byte spans and typed relevance, token-cost, and required
keys. NaN relevance compares equal before token-cost and ID tie-breakers;
equal keys retain input order. The v1 ABI bounds inputs to 65,537 capsules and
passes each ID as a borrowed byte pointer with a checked length, then returns a
permutation. The ABI copies no ID bytes and imposes no aggregate ID-byte limit.

Rust retains every `String`, marshals count-bounded ID pointers, checked lengths,
and typed keys, validates the full permutation, and moves capsules into that
order. The existing Mojo admission planner still receives 65,536-item batches
with the remaining token budget. Rust contains no capsule comparator or
ordering fallback. Rust supplies permutation and scratch buffers, so the Mojo
ordering kernel performs no heap allocation.

Validation passed:

- `mojo format mojo/prodex_core/smart_context_capsule_order.mojo mojo/tests/smart_context_capsule_order_test.mojo` and `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/smart_context_capsule_order_test.mojo` passed.
- The mutation proof inverted required-first ordering; the direct Mojo test failed at the expected permutation assertion. Restoring saved source bytes was byte-exact: SHA-256 before and after was `0567239cb15ec10ff2055660f963323612988c1db3f893fbeb2b7bf50b5ee426`.
- `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0 cargo test --locked -q -p prodex-runtime-proxy --lib capsule_ -- --test-threads=1` — 7 passed, including NaN tie-breaks, 65,537-item batching, total ID bytes above 4 MiB, and per-ID span validation.
- `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0 cargo clippy --locked -q -p prodex-mojo-core -p prodex-runtime-proxy --all-targets --all-features -- -D warnings` and `cargo fmt --all -- --check` passed.
- `npm run docs`, `npm run test:changed`, and `git diff --check` passed.
- `node scripts/ci/mojo-no-fallback-guard.mjs --self-test`, `node scripts/ci/mojo-authority-guard.mjs`, `node scripts/ci/mojo-ownership.mjs --check`, and `node scripts/ci/size-guard.mjs` passed. Ownership reports 154 authoritative operations; the new Mojo module is 198 lines.
- `node scripts/ci/mojo-production-share.mjs --check` — 81,486 Mojo LOC and 203,748 Rust LOC, or **28.57% Mojo**. The 7% release floor and ownership non-regression pass; the 75% project target remains unmet.
- No commit or push was made.
## Smart Context source-symbol migration, stage 1

Stage 1 moves only declaration recognition, Rust/Python/JavaScript classification, labels, names, fallbacks, `impl` naming, and Rust test-attribute labeling into versioned ABI `prodex_smart_context_symbol_classify_v1`, linked through `prodex-mojo-core` from `smart_context_symbol_classification.mojo` and `smart_context_symbol_parser.mojo`. Rust retains prefix attachment, range planning, excerpts, deduplication, capacity, and DTO construction; no Rust classifier or feature-off fallback remains.

Direct required-Mojo tests cover declaration families, names, labels, styles, and attributes; the artifact-store caller test checks persisted ranges, text, and hashes. Mutating the `fn ` recognizer failed the direct test. Byte-exact restoration matched SHA-256 `b3eb46d111a8c3f0e418013ff4315813e28b1e97cc374e2ec0f39caec176071c`. Direct tests pass 2/2 and app caller passes 1/1. Clippy, fmt, docs, changed-tests, authority, ownership, no-fallback, size, share, churn, and diff checks pass.

## Smart Context full source-symbol indexing migration

The stage-1 source-symbol classifier is now superseded by the full versioned
`prodex_smart_context_symbol_index_v1` planner. Mojo owns declaration
recognition and naming plus prefix attachment, Rust/Python/JavaScript range
boundaries, duplicate suppression, excerpt bounds, capacity completeness, and
bounded symbol-index planning. Rust keeps borrowed line-span preparation,
validated ABI output materialization, hashes, and artifact DTO construction.
The old Rust `semantic_index/symbols.rs` parser and the stage-1 Mojo classifier
and parser modules are deleted; no Rust or duplicate Mojo fallback remains.

Validation passed:

- `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0 cargo test --locked -p prodex-mojo-core --features mojo-runtime --test smart_context_symbols -- --list` listed 4 tests; all 4 passed.
- `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0 cargo test --locked -p prodex-app runtime_smart_context_artifact_symbol_index_uses_mojo_ranges -- --list` listed 1 production caller test; it passed.
- Mojo no-fallback, authority, and ownership guards passed; ownership reports 167 authoritative operations and 95.10% Mojo in the frozen semantic ledger.
- `node scripts/ci/mojo-production-share.mjs --check` reports 83,922 reachable Mojo LOC and 204,650 Rust production LOC, or **29.0818% Mojo**.
## Kiro model-catalog normalization

Kiro profile catalogs, ACP model lists, quota lookup, import validation, and
dynamic provider configuration share one normalization path. The existing
Kiro Mojo domain now selects root and nested model arrays, enforces the input
entry cap, applies identifier/name alias precedence, trims strings with Rust
Unicode whitespace rules, and selects optional description and positive
context-window metadata. The versioned v1 ABI returns checked string spans and
typed issue tags. Rust retains Serde tree acquisition and JSON materialization;
the existing provider catalog service still owns canonical-ID merge and total
catalog limit enforcement. Mojo failures return errors without Rust policy
recomputation.

The required-Mojo ABI test covers primary aliases, Unicode trim, fallback names,
source order, missing arrays, empty catalogs, and limits. The provider-core
caller test covers output shape, metadata, alias precedence, and dynamic
deduplication. The app parser test covers all supported root and nested shapes.
A mutation changed primary identifier precedence; both the direct ABI test and
provider-core caller test failed. Restored Mojo source matched its pre-mutation
SHA-256 exactly (`516ed644fd8f926849a98de95cd28d779216e08c57a10fa13f0525d25c78f2af`).

Focused validation passed:

- `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0 cargo test --locked -q -p prodex-mojo-core --features mojo-rich --test kiro_catalog -- --test-threads=1` — 2 passed after restoration.
- `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0 cargo test --locked -q -p prodex-provider-core kiro_model_catalog_uses_mojo_precedence_and_preserves_model_metadata -- --test-threads=1` — 1 passed after restoration.
- `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0 cargo test --locked -q -p prodex-app --lib kiro_model_catalog -- --test-threads=1` — 4 passed, including supported root/nested shapes and the hard limit.
- Formatting, docs, no-fallback, authority, ownership, and size checks pass; ownership reports 161 authoritative operations.
- `node scripts/ci/mojo-production-share.mjs --check` — 81,976 reachable Mojo LOC and 204,388 Rust LOC (286,364 total), or **28.63% Mojo**. The 7% release floor and non-regression pass; the 75% project target remains unmet.

The checkpoint-wide gates also pass: workspace Clippy with warnings denied,
`cargo fmt --all -- --check`, `npm run docs`, `npm run test:changed -- --base HEAD`
(16 changed paths, 341 tracked changed lines; 1,124 including new files), size guard, Mojo no-fallback
self-test/check, authority and ownership checks, and `git diff --check`.

## Runtime doctor Smart Context fallback-decision migration

`runtime_doctor_marker.mojo` now owns Smart Context autopilot fallback-decision
classification through `prodex_mojo_runtime_doctor_smart_context_decision_is_fallback_v1`.
Only `rewritten` and `pass_through` are non-fallback decisions; every other decision label
is classified as fallback. The former Rust `!matches!(decision, "rewritten" | "pass_through")`
production classifier was deleted. Rust retains log/JSON parsing, aggregation, reason counting,
and report ownership; Mojo errors do not trigger Rust semantic recomputation.

Verification includes a direct required-Mojo ABI integration test and the production
runtime-doctor summary regression. Both targeted `-- --list` invocations report one test.
A sensitivity mutation replaced the `pass_through` exemption with
`self_check_passthrough`; the production caller regression failed with `fallback_count`
`left: 1` versus `right: 2`. Restoring the Mojo source returned the exact pre-mutation
SHA-256 `5be6e9fd8c8c4ccfa80ca24964fb1d990a80372f98ab797a8db18720aa641429`, and the
caller regression passed again.

## Runtime operational histogram observation migration

`operational_metrics.mojo` now owns per-observation histogram accumulation through
`prodex_mojo_operational_histogram_observe_v1`. Mojo performs cumulative bucket
comparison, saturating bucket increments, saturating sample count, and saturating
sum updates. Rust retains the mutex-protected registry, metric-key construction,
bucket/count storage, and the typed ABI call. The former Rust loop over bucket
bounds plus Rust `saturating_add` updates for histogram count/sum were deleted; no
Rust production fallback or recomputation remains.

Verification passed:

- `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0 cargo test --locked -q -p prodex-mojo-core --features mojo-observability --test operational_metrics -- --list` listed 4 tests; all 4 passed.
- `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0 cargo test --locked -q -p prodex-app --lib runtime_histogram_bucket_plan_is_observable_at_the_registry_boundary -- --list` listed 1 production caller test; it passed.
- No-fallback self-test/check, authority guard, ownership check, size guard, formatting, and `git diff --check` passed; ownership reports 176 authoritative operations.
- Sensitivity proof changed the authoritative cumulative comparison from `observation <= bounds[index]` to `<`. The direct required-Mojo test failed with bucket counts `[0, 1, 2]` instead of `[1, 1, 2]`. Restoring the source was byte-exact: SHA-256 before and after was `ded151abd8475c683e90b5452e03fd95de153a1942bd459fba7a8d483e87fdc6`; the targeted test passed again.
- `node scripts/ci/mojo-production-share.mjs --check` reports 84,270 reachable Mojo LOC and 204,893 Rust production LOC, or **29.1427% Mojo**. The 7% release floor and Mojo non-regression pass; the 75% project target remains in progress.

## Runtime doctor log-value and fallback-reason migration

`runtime_doctor_marker.mojo` now owns two additional diagnostic decisions.
`prodex_mojo_runtime_doctor_log_value_is_ignored_v1` classifies only the empty
string and exact `-` sentinel as absent.
`prodex_mojo_runtime_doctor_smart_context_fallback_reason_source_v1` selects the
fallback-reason source with explicit event reasons first, then `self_check` for
`self_check_passthrough`, then the decision label. Rust retains log/JSON parsing,
DTO and BTreeMap ownership, iteration over the Mojo-selected source, and counter
mutation. The prior Rust sentinel predicate and fallback-reason precedence chain
were deleted; Mojo failures do not trigger Rust semantic recomputation.

Verification passed:

- `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0 cargo test --locked -q -p prodex-mojo-core --features mojo-rich --test runtime_doctor_markers -- --list` listed 9 tests; all 9 passed.
- `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0 cargo test --locked -q -p prodex-runtime-doctor runtime_doctor_smart_context_fallback_reason_source_is_mojo_owned_at_summary_boundary -- --list` listed 1 production caller test; it passed.
- No-fallback self-test/check, authority guard, ownership check, and size guard passed; ownership reports 178 authoritative operations.
- Sensitivity proof changed `reason_count > 0` to `reason_count > 1`. The production caller regression failed because the single explicit `previous_response` reason was no longer selected. Restoring the source was byte-exact with SHA-256 `96284d3a981fecf3b1a8a1f1162597d4d9ccc81e1edf97bcd0e332585f7e6493`; the caller regression passed again.
- `node scripts/ci/mojo-production-share.mjs --check` reports 84,335 reachable Mojo LOC and 204,973 Rust production LOC, or **29.1506% Mojo**. The 7% release floor and Mojo non-regression pass; the 75% project target remains in progress.

## Runtime doctor log-value and fallback-reason policy migration

`runtime_doctor_marker.mojo` now owns two additional deterministic diagnostic decisions. The versioned `prodex_mojo_runtime_doctor_log_value_is_ignored_v1` ABI classifies exact empty and `-` sentinel values, while `prodex_mojo_runtime_doctor_smart_context_fallback_reason_source_v1` selects explicit event reasons first, then `self_check` for `self_check_passthrough`, then the decision label. The Rust copies in `log_fields.rs` and `smart_context.rs` were deleted; Rust retains parsing, DTO ownership, counters, and application of the Mojo-selected source without semantic recomputation.

Verification passed with `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0`: `runtime_doctor_markers -- --list` enumerated 9 tests and all 9 passed; the production summary regression `runtime_doctor_smart_context_fallback_reason_source_is_mojo_owned_at_summary_boundary -- --list` enumerated 1 test and passed; the full `prodex-runtime-doctor` suite passed 43 unit tests plus 1 integration test. Clippy for `prodex-runtime-doctor` and `prodex-mojo-core` with all targets/features and warnings denied, formatting, Mojo no-fallback self-test/check, authority guard, ownership check, and `git diff --check` all passed. Mutation proof changed Mojo reason precedence from `reason_count > 0` to `reason_count > 1`; the production summary regression failed with exit 101 because the expected explicit reason disappeared. Source restoration was byte-exact with SHA-256 `96284d3a981fecf3b1a8a1f1162597d4d9ccc81e1edf97bcd0e332585f7e6493`. Canonical production share is **29.1506% Mojo** (84,335 Mojo LOC, 204,973 Rust LOC, 289,308 total); the 75% project target remains in progress.

## Profile-export requested-profile selection migration

`profile_export_policy.mojo` now owns requested-profile selection through `prodex_profile_export_selection_v1`: empty-pool rejection, default-all selection, first-missing-request detection, request-order preservation, and duplicate suppression. `resolve_requested_profile_names` no longer contains the Rust membership/dedup algorithm; Rust retains BTreeSet acquisition/order, borrowed view construction, validated index materialization, String ownership, and the existing user-facing errors. Mojo errors propagate without semantic recomputation.

Required-Mojo validation passed: the direct `profile_export_selection_plan_is_mojo_owned -- --list` target enumerated 1 test and passed; the production `requested_profile_names_ -- --list` filter enumerated 3 tests and all passed; the full `prodex-profile-export` suite passed 58 tests. Clippy for `prodex-profile-export` + `prodex-mojo-core` with all targets/features and warnings denied, formatting, no-fallback self-test/check, authority guard, ownership check, and `git diff --check` passed. Mutation proof inverted the Mojo duplicate-insertion gate; `requested_profile_names_deduplicate_and_preserve_request_order` failed with `left: []` and `right: ["second", "main"]` (exit 101). The Mojo source was restored byte-for-byte to SHA-256 `d974225adfe6828d813cc1fe0c509a20d35a528e660ca2574b116a0eb5e9ce83`. Canonical production share is **29.1607% Mojo** (84,412 Mojo LOC, 205,060 Rust LOC); the 75% target remains in progress.

## Profile export/import active-profile resolution migration

`profile_export_policy.mojo` now owns two additional active-profile decisions. `prodex_profile_export_active_profile_selected_v1` decides whether the current active profile belongs to the selected export set, while `prodex_profile_import_active_profile_plan_v1` preserves an existing active profile first and otherwise resolves the source active profile through the imported source-to-target name mapping. Rust retains BTreeMap/string ownership, borrowed ABI views, output-index validation, and payload materialization; the former Rust membership scan and `existing.or_else(source-map lookup)` policy are deleted with no fallback recomputation.

Focused required-Mojo verification listed and passed one direct adapter test plus one caller test for each export/import decision. Mutation proof inverted the existing-profile precedence (`existing.len > 0` to `existing.len == 0`); `imported_active_profile_uses_existing_active_profile_first` failed with `Some("target")` instead of `Some("current")`. Restoring the Mojo source was byte-exact with SHA-256 `5138875cc228decf163e9b40999e308648ab4b45df89366fa49bacf11227e152` before and after.

## Copilot JSONC normalization migration

Copilot config `//` line-comment stripping now runs through `prodex_profile_export_copilot_strip_json_line_comments_v1` in `profile_export_policy.mojo`. Mojo owns string/escape state, comment detection outside JSON strings, byte-preserving copy, and newline preservation. Rust retains the initial strict Serde parse, caller-owned output allocation, UTF-8 materialization, zeroizing storage, and the final Serde parse; the three former Rust parser helpers were deleted and there is no fallback recomputation.

Required-Mojo direct adapter coverage and the production `copilot_config_parser_accepts_copilot_jsonc_comments` caller test each list one test and pass. A mutation changed the second `/` detector to `.`, and the caller failed with `expected value at line 1 column 1`; restoring the Mojo source was byte-exact at SHA-256 `695e5913f11f95294f413193291389b37e705448955e4778ca19a354bb312776`.

## Copilot version and platform metadata migration

`profile_export_policy.mojo` now owns Copilot CLI version-triplet parsing and OS/architecture platform-label mapping through operation IDs 1 and 2 of `prodex_profile_export_copilot_metadata_v1`. Version parsing preserves the first three dot-separated components, consumes only each component's leading ASCII digits, maps missing/invalid/u64-overflow components to zero, and ignores suffixes after the leading digits. Platform mapping preserves exact linux/macos/windows x86_64/aarch64 labels and the historical linux-x64 default. Rust retains host OS/architecture acquisition, borrowed ABI view construction, tuple/static-label materialization, and all network/Serde work; the former Rust split/parser closure and platform `match` were deleted with no fallback.

Required-Mojo direct coverage and the production `copilot_url_helpers_match_import_expectations` caller each list one test and pass. A mutation changed linux/x86_64 from tag 0 to tag 1; the caller failed with `left: "linux-arm64"` versus `right: "linux-x64"`. Restoring the Mojo source was byte-exact at SHA-256 `3c99a33e6f3f53e46b9b2a089729d1972c3412d35ecd60b2ef336d30c8950ccc`.

## Copilot host/API URL planning migration

`profile_export_policy.mojo` now owns Copilot user-API origin and default models-API URL planning through operations 1 and 2 of `prodex_profile_export_copilot_url_v1`. The Mojo plan preserves Unicode host trimming, trailing-slash removal, exact `://` scheme parsing, authority/path/query/fragment boundaries, bracketed IPv6 and numeric-port handling, localhost/API-prefix decisions, case-insensitive GitHub aliases, case-sensitive lowercase HTTP(S) prefix stripping, exact lowercase `.ghe.com` routing, and the historical `https://` trailing-slash edge case. Rust retains borrowed host views, caller-owned output allocation, UTF-8 String materialization, anyhow error text, Serde, and network work; the former Rust URL parser and `authority_host_and_port` helper are deleted without fallback recomputation.

Required-Mojo direct adapter coverage and the production `copilot_url_helpers_match_import_expectations` caller each list one test and pass. Boundary coverage includes default HTTPS, explicit port, `api.` hosts, bracketed IPv6 loopback, invalid empty-scheme input, uppercase GitHub aliases, case-sensitive GHE fallback, and the legacy `https://` normalization. Mutation proof changed the GHE output prefix from `https://copilot-api.` to `https://api.`; the caller failed with `left: "https://api.enterprise.ghe.com"` versus `right: "https://copilot-api.enterprise.ghe.com"`. Restoring the Mojo source was byte-exact at SHA-256 `651889bc17b4b4764b3a7330105da5f432e442fc0d35ffb07aad9545b9f87333`.

## Copilot import-state planning migration

`profile_export_policy.mojo` now owns Copilot profile import-state decisions through `prodex_profile_export_copilot_import_state_v1`: auto-activation when no active profile exists, explicit activation override, update-existing versus add-new/default, requested-name mismatch against an already imported account, and requested-name duplicate rejection. Rust retains the profile-name existence callback, lazy default-name generation, String/error materialization, and application of validated action tags; the former Rust branch tree and activation formula are deleted with no fallback recomputation.

Required-Mojo direct coverage and the production `copilot_state_plan_updates_existing_or_adds_new_profile` caller each list one test and pass. Mutation proof changed the authoritative activation formula from `has_active_profile == 0 or activate_requested != 0` to `and`; the caller failed with `AddNew { profile_name: "copilot-octo", activate: false }` instead of `activate: true`. Restoring the Mojo source was byte-exact at SHA-256 `dfd5fea45de80dc81d756cfdcdc24f40826fd78e9a453bb2f99d407f61764760`.
