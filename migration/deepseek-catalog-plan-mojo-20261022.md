# DeepSeek runtime catalog plans compiled in Mojo (2026-10-22)

The existing compiled runtime planner now owns DeepSeek candidate order, trimmed
ASCII-folded deduplication, priority and fallback metadata. The former Rust
assembly and metadata-default helper are deleted. The application maps typed
plans into the unchanged compiled schema renderer and retains secure file effects.

Rust acquires the existing Mojo static catalog through its public enumeration
ABI and serializes metadata. Its seven runtime entries, labels and descriptions
remain distinct: `auto`, `pro` and `flash` keep their individual entries. Every
DeepSeek row retains the configured context and compact values exactly, including
zero and `u64::MAX`. Existing public static enumeration/lookup APIs remain compatible.
The version-1 plan ABI and scratch layout are unchanged. Dynamic DeepSeek records
are rejected; its production caller supplies an empty slice.

Eight complete catalogs were captured from the original production caller and
frozen as canonical SHA256 values. They cover aliases, whitespace, empty and long
queries, Unicode, zero and unsigned endpoints. The original frozen caller passes.
Strict Cargo uses `PRODEX_MOJO_REQUIRED=1` and `PRODEX_MOJO_VERSION=1.1.0`:

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/deepseek_runtime_catalog_plan_test.mojo`
- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/runtime_catalog_plan_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib json::runtime_catalog`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-runtime --lib json::runtime_catalog`
- `cargo test --locked -q -p prodex-app --lib runtime_deepseek_config:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib runtime_external_provider_config:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-provider-core -- --test-threads=1`

Clippy, formatting, guards, offline replay, docs and changed-test commands follow
the [validation matrix](external-model-schema-mojo-20261022.md). Direct Mojo,
six bridge tests per mode, six DeepSeek callers, 20 external callers and 554
provider tests pass. Strict workspace Clippy, formatting and guards pass.
Default changed tests pass 554 Rust tests in 89.00s, without strict/archive overrides.
Canonical share is 36.67728728192322% (138,942 Mojo / 239,881 Rust production LOC).
The checked acquisition adapter adds one net Rust production line while semantic
ownership reuses the existing Mojo planner; no duplicate table is introduced.
The 75% target remains open.
Full default CI passed 1,443 Rust tests at parent `204077056`; it is not repeated
for this focused follow-up. Native non-Linux execution, live providers, exhaustive
serialized suites and performance benchmarks remain outside coverage.
