# Local runtime model schema compiled in Mojo (2026-10-23)

Local's model-entry schema now extends the existing compiled runtime-catalog
owner. Its fixed description, priority, reasoning defaults/descriptions,
capabilities, text modality and explicit empty instruction source move out of
Rust. The Rust JSON schema function is deleted without a fallback. The existing
version-1 parsed-tree ABI, measured writer and validation remain unchanged.

A typed provider port projects only the observed slug and two unsigned budgets.
Both renderer ports reuse their existing private Serde/tree acquisition. No
placeholder metadata, new export, dependency or duplicate decision engine is
added. Rust retains the catalog envelope, Serde serialization, secure filesystem
writes and launch/config acquisition. Local launch defaults, trim behavior,
argument preparation and compact-limit clamp are outside this checkpoint.

Local's raw slug is preserved and supplies the display name. Priority remains
one, default reasoning high, and the fourth effort description remains exactly
`Max reasoning effort`. Reasoning summaries remain disabled while search stays
enabled. Persisted sessions retain an explicitly empty base instruction source.
The exact supplied context and compact budgets survive, including zero,
u64::MAX and compact values above context. Other providers retain their existing
schemas and capabilities.

Seven complete serialized files were captured through the original Rust writer
before production changes. Their frozen SHA256 and byte-length fixture follows
the established whole-catalog baseline pattern. Complete original files remain
in validation evidence; an independent check verifies every length and digest.
Cases cover ordinary and empty slugs, Unicode/whitespace/JSON escaping, zero,
unsigned endpoints, compact above context and a 4,097-byte slug. The frozen caller
passes on the original implementation and on the compiled production path.

Strict Cargo uses `PRODEX_MOJO_REQUIRED=1` and `PRODEX_MOJO_VERSION=1.1.0`:

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/local_runtime_model_catalog_test.mojo`
- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/runtime_model_catalog_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib json::runtime_catalog`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-runtime --lib json::runtime_catalog`
- `cargo test --locked -q -p prodex-provider-core --lib catalog::runtime:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib runtime_local_provider_config:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib runtime_gemini_config:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib runtime_external_provider_config:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib runtime_deepseek_config:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-provider-core -- --test-threads=1`

Strict workspace Clippy/fmt, guards, offline replay, default CI/changed tests,
docs, churn and whitespace use the
[validation matrix](external-model-schema-mojo-20261022.md). The included caller
file also uses `rustfmt --check --edition 2024 crates/prodex-app/src/runtime_local_provider_config/catalog_tests.rs`.
Default CI/changed tests omit strict/archive overrides. The owned core target
was invalidated before adopted ABI validation. Cookie and info-panel WIP remain
protected. No counting rule or guard cap changes. The 75% target remains open.
Native non-Linux, live providers, exhaustive serialized suites and benchmarks
are outside coverage.

Both direct Mojo tests, nine bridge tests in each core/runtime mode, nine
provider-boundary cases and six Local callers pass. All seven original file
lengths/digests match. Regressions pass seven Gemini, 20 external and six
DeepSeek callers; the complete provider suite passes 556 tests.
Canonical share is 36.74501510892666% (139,354 Mojo / 239,892 Rust production LOC),
removing 47 net Rust production LOC.

Strict workspace Clippy/fmt, guards, boundary self-tests and offline replay pass.
Full default CI passes 1,446 Rust tests in 522.33 seconds. Default changed tests
pass 556 Rust tests in 127.47 seconds; their no-feature app/core invocations select
zero tests, covered by the explicit caller/feature commands above.
