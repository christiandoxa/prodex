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

## Runtime health Rust-oracle deletion wave

The runtime-proxy health scorer, latency policy, inflight limits, and bump/recovery
decisions now use their existing `runtime_health.mojo` owners unconditionally. The
feature-off Rust implementations were removed; the exact pre-migration Rust formulas
remain under `#[cfg(test)]` as direct Mojo differential oracles.

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
reading and parsing, metadata lookup, context limits, model JSON construction,
and a feature-off Rust oracle. Dynamic duplicates remain available to first-match
metadata lookup, while the Mojo index plan keeps the first model in output order.

The merge ABI accepts IDs across Rust's representable string range, including
values longer than 65,536 bytes. Its candidate count remains capped at 65,536;
external model catalogs stay within their existing 512-entry dynamic limit plus
the bounded static provider list.

Validation passes: strict Mojo core tests (84); the long-ID Rust differential;
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
parsing, `[DONE]` handling, result mapping, and the feature-off Rust oracle.
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
