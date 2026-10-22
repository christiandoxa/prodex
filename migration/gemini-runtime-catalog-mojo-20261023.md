# Gemini runtime catalog plans and schemas compiled in Mojo (2026-10-23)

Gemini catalog candidate order, trim/deduplication, metadata lookup/defaults,
priority, hard-limit diagnostics and exact context/compact budgets now use the
existing compiled runtime planner. Its model-entry schema, five fixed reasoning
levels/descriptions, capabilities, modalities and instructions use the existing
compiled renderer. The Rust assembly loop, metadata helper, JSON schema and
duplicate 430-byte instruction constant are deleted without a fallback. Gemini
and DeepSeek reuse the same existing compiled instruction text.

The existing typed provider-core input/result contracts and version-1 parsed-tree
ABIs are reused; both Mojo owners already belong to the production build graph.
No wrapper layer, dependency or replacement decision engine is added. Rust keeps
Serde acquisition/materialization, the catalog envelope, CLI/config preparation
and filesystem/secret-store writes. Existing launch configuration/defaults,
numeric override validation and consumed-argument handling are unchanged.

Gemini metadata lookup remains casefolded catalog-ID matching without aliases.
Its launch candidate retains first position after trimming/deduplication; an empty
candidate is omitted as before. All candidates retain the exact caller budgets,
including zero, one, u64::MAX and compact values above context. Gemini accepts no
dynamic models. Generic external-provider alias/context policy and DeepSeek's
seven legacy entries retain their distinct behavior. Unsupported providers and
malformed tree/flags/versions remain rejected; short buffers preserve output and
metadata. The existing Gemini hard-limit message keeps its original wording.

Ten complete catalogs were captured from the original Rust implementation before
production changes. The established DeepSeek whole-catalog SHA256 fixture pattern
is reused: every serialized catalog field/order contributes to the frozen digest;
counts and first-row budgets are also explicit. Complete original objects remain
in validation evidence and all ten hashes were independently verified against
them. Cases cover defaults, known/unknown/casefolded/padded models, Unicode and
escaping, empty/Unicode-blank input, unsigned endpoints, oversized compact values
and 4,097-byte queries/padding. Seven original caller tests pass before adoption,
then all ten complete-catalog digests match through the compiled production path.

Strict Cargo uses `PRODEX_MOJO_REQUIRED=1` and `PRODEX_MOJO_VERSION=1.1.0`:

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/runtime_model_catalog_test.mojo`
- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/runtime_catalog_plan_test.mojo`
- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/deepseek_runtime_catalog_plan_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib json::runtime_catalog`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-runtime --lib json::runtime_catalog`
- `cargo test --locked -q -p prodex-provider-core --lib catalog::runtime:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib runtime_gemini_config:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib runtime_external_provider_config:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib runtime_deepseek_config:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib runtime_local_provider_config:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-provider-core -- --test-threads=1`

The fixed-budget direct fixture retains all DeepSeek assertions and now verifies
Gemini's identical budget behavior plus ID-only lookup and limit diagnostics.
Strict workspace Clippy/fmt, guards, offline replay, default CI/changed tests,
docs, churn and whitespace follow the
[validation matrix](external-model-schema-mojo-20261022.md). Default CI/changed
tests omit strict/archive overrides. All three direct tests, eight bridge tests
in each core/runtime mode, eight provider-boundary cases, seven Gemini callers,
20 external, six DeepSeek and five Local callers pass. All ten original Gemini
catalog digests match. The complete provider suite passes 555 tests. Strict
workspace Clippy/fmt, guards and offline replay pass. Full default CI passes
1,446 Rust tests in 476.88 seconds. Default changed tests pass 555 Rust tests in
84.56 seconds; their no-feature app/core invocations select zero tests, supplied
instead by the explicit feature/caller commands above.

Canonical share is 36.738793987602925% (139,344 Mojo / 239,939 Rust production LOC),
removing 97 net Rust production LOC. No counting rule or guard limit changes.
The owned core target was invalidated before adopted ABI checks; original-source,
complete-object, test and prototype evidence remains owned by this checkpoint.
Cookie and info-panel WIP remain protected. The 75% goal stays open. Native
non-Linux, live providers, exhaustive serialized suites and benchmarks are outside
coverage.
