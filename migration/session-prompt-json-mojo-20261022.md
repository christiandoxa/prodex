# Session prompt record policy in Mojo (2026-10-22)

## Production ownership

Rollout verification, output framing and transcript logs now pass borrowed
Serde-decoded JSON trees to the compiled session policy. Mojo owns record
classification, payload shape, metadata visibility, content fallback order
and exact LF-joined comparison. The Rust field classifiers, metadata scan
and exact-message extraction helper are removed. Output framing parses JSON
once and acquires one tree for both shape and visibility checks.

Rust retains file/clock/process effects, Serde wire compatibility, existing
raw-line limits, redaction, rendering and typed errors. ABI errors propagate
through output/verification; logs retain their existing mandatory-kernel
failure behavior. There is no Rust semantic fallback or comparison oracle.

The v1 scalar ABI bounds the tree to 1,048,576 nodes, decoded keys/text to
64 MiB and expected text to 64 MiB. It validates version, operation, canonical
unused text, topology, UTF-8 and empty wire spans, then returns a checked
boolean or a typed error tag. It borrows text without logging or persisting
it. The existing compiled session-report owner exports the new entrypoint;
its public numeric shape/visibility/exact APIs share the extracted kernels.
Ownership metadata points at the new caller and the reachable semantic file;
counting rules, baselines and thresholds are unchanged.

## Compatibility

Serde still decides malformed JSON, duplicate keys and numeric limits.
Unknown/missing/non-string record types accept any present payload, including
null. Event/response payloads require an object with a string type; metadata
and turn-context records require an object. Visibility treats absent or
non-array metadata as absent, but empty/mixed/internal kind arrays as hidden.
The log payload-only absent-metadata fast path remains distinct from record
visibility. Visible response records need no content array for framing.

Exact verification requires at least one response content part. Present
`text` wins before type checking, so non-string `text` never falls back to
`content`. Empty parts, Unicode, NUL, leading/trailing whitespace and LF
separators are preserved. The matcher compares the borrowed tree directly
against expected bytes rather than allocating the renderer's joined string;
the existing renderer join ABI remains unchanged. The 512 KiB raw-line bound
still includes its trailing newline.

## Verification

Strict Cargo commands use Mojo 1.1.0 with `PRODEX_MOJO_REQUIRED=1` and
`PRODEX_MOJO_VERSION=1.1.0`. Default CI omits those variables.

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/session_prompt_write_json_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib json::session_prompt_write::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib session_cli_policy::`
- `cargo test --locked -q -p prodex-app --lib super_expose::session_prompt_write::output::policy_tests:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib super_expose:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib app_commands::log:: -- --test-threads=1`
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --check`
- `node scripts/ci/mojo-no-fallback-guard.mjs --self-test`
- `node scripts/ci/mojo-no-fallback-guard.mjs`
- `node scripts/ci/mojo-authority-guard.mjs`
- `node scripts/ci/mojo-ownership.mjs --check --json`
- `node scripts/ci/mojo-production-share.mjs --check --json`
- `node scripts/ci/size-guard.mjs`
- `node scripts/ci/crate-boundary-guard.mjs --self-test`
- `node scripts/ci/crate-boundary-guard.mjs`
- `node scripts/ci/secret-boundary-guard.mjs`
- `npm run docs`
- `npm run test:changed`
- `npm run ci -- --jobs 1`
- `git diff --check`

All listed checks pass: direct Mojo, three strict bridge tests, 13 existing
numeric-policy tests, four new output caller tests, all 120 Super expose tests,
all 82 log tests and strict workspace Clippy. The final bridge run also covers
decoded-byte bounds before UTF-8 reads. Default `npm run ci -- --jobs 1` passes
(509.60 seconds); `npm run test:changed` passes (2.65 seconds). Its default
feature-off harnesses run zero tests; strict positive counts prove ABI use.
The first CI attempt rejected two new wildcard test imports; explicit imports
restore the existing 174-import cap, and the full rerun passes.

Canonical share is 36.33835470028661% (136,678 Mojo / 239,448 Rust production
LOC). The 75% project objective remains open.
Native non-Linux, live-provider and exhaustive full-workspace tests remain
outside this offline unit.
