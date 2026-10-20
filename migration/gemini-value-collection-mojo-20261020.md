# Gemini value and input-text traversal migrated to Mojo (2026-10-20)

## Scope and single semantic authority

The compiled `gemini_value_policy.mojo` production module now owns three
previously recursive Gemini provider operations: collecting nested array
strings, collecting Unicode-trimmed nonblank path strings, and collecting
input text from arrays and object `text`/`content` fields. Object traversal
follows only the `content` field, preserves a present `text` field's
precedence even if it is not a string, and intentionally emits both the
selected object string and the nested content string when required by the
original contract. Array order and existing collector-prefix appends remain
unchanged.

The Rust functions now acquire Serde JSON, apply the versioned typed Mojo
JSON-tree result, extend caller-owned string lists, or construct host-owned
`PathBuf`s. The previous Rust recursive collection branches were removed.
There is no silent Rust algorithm fallback. The module is compiled into the
production `mojo-rich` archive; the shared measured JSON ABI validates tree
structure, versions, capacity and UTF-8 output before publication.

## Validation and status

- `mojo build --emit object -I mojo/prodex_core -o /tmp/prodex-gemini-value-collection-20261020.o mojo/prodex_core/gemini_value_policy.mojo` passes.
- `PRODEX_MOJO_REQUIRED=1 CARGO_BUILD_JOBS=6 cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib compiled_gemini_value -- --test-threads=1`: 2 direct core tests pass.
- `PRODEX_MOJO_REQUIRED=1 CARGO_BUILD_JOBS=6 cargo test --locked -q -p prodex-provider-core --features mojo --lib -- --test-threads=1`: 510 provider-core tests pass, including three new live collector tests.
- `node scripts/ci/mojo-no-fallback-guard.mjs --self-test`: pass; source checks reject restored Rust array recursions and missing ABI/production linkage.
- `node scripts/ci/mojo-production-share.mjs --check --json`: pass, with audited non-regression satisfied.
- `cargo fmt --all -- --check` and `git diff --check`: pass.

The official production share increases from 35.72208587265256% to
35.737343386031895% (129,639 Mojo / 233,116 Rust production LOC). The
75% project target remains unmet and the measurement policy is unchanged.
Build caches are temporary and can be deleted after checkpoint consumers
finish. Remote CI is not a prerequisite for continuing migration.
