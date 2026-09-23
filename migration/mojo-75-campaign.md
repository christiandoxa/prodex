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
