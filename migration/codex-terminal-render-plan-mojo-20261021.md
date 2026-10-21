# Codex terminal token rendering in compiled Mojo (2026-10-21)

## Production ownership

`TerminalInfo::user_agent_token` in
`crates/prodex-app/src/quota_support/codex_openai_auth.rs` delegates the complete
terminal token projection to `prodex_codex_auth_terminal_token_v1` in the existing
compiled `mojo/prodex_core/codex_auth_headers.mojo` owner.

Mojo selects the original program, original TERM, or detected terminal label;
chooses which labels include versions; joins the version; and applies the
Unicode-scalar terminal alphabet. An explicitly empty program still overrides
TERM, while an empty TERM permits the detected label. Unknown terminal names,
combining characters, embedded NUL and non-ASCII scalars preserve prior behavior.

Rust retains environment/process reads, original metadata, HTTP header validity,
allocation and checked ABI decoding. Its label/version match,
`format_terminal_version` and terminal sanitizer implementation are deleted.
The batch renderer reuses the shared Mojo scalar sanitizer introduced by the
preceding checkpoint. There is no Rust rendering or sanitizing fallback.

## ABI and regression coverage

The v1 ABI borrows exactly three fixed-layout address/length records for program,
version and TERM. Each UTF-8 field is bounded to 1 MiB. The ABI validates version,
kind, presence tag, record/metadata alignment, byte views and capacity before
writing output. The host checks the returned length and UTF-8. Stable explicit
terminal discriminants match the existing classification ABI.

Direct Mojo and Rust bridge tests cover all 14 terminal kinds, every version
rule, original field precedence, absent/empty values, allowed punctuation,
Unicode scalars and combining marks. Raw ABI tests cover invalid versions/tags,
malformed UTF-8, null/misaligned records, negative/short capacities, unchanged
output on rejection, empty output and oversized host input. The production
caller test checks metadata precedence and the actual terminal token.

## Validation

The checkpoint runs these offline commands with the pinned Mojo 1.1.0 compiler
and `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0` for direct Cargo checks.
The changed-test runner retains its default feature selection:

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/codex_terminal_token_test.mojo`
- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/codex_auth_terminal_policy_test.mojo`
- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/codex_auth_headers_policy_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib codex_auth_headers::`
- `cargo test --locked -q -p prodex-app --lib compiled_auth_headers_tests -- --test-threads=1`
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --check`
- `node scripts/ci/mojo-no-fallback-guard.mjs`
- `node scripts/ci/mojo-authority-guard.mjs`
- `node scripts/ci/mojo-ownership.mjs --check --json`
- `node scripts/ci/mojo-production-share.mjs --check --json`
- `node scripts/ci/size-guard.mjs`
- `npm run docs`
- `npm run test:changed`
- `git diff --check`

The 75% project goal remains open. Live provider traffic and non-Linux execution
are outside this offline checkpoint.

The first changed-test run inherited strict Mojo mode while selecting a
feature-off bridge build and correctly rejected that combination. The runner
is repeated in its normal environment; compiled bridge/caller checks remain
strict and separate.

After rebasing onto the preceding sanitizer/nonblank checkpoint, all listed
checks pass: nine compiled bridge tests, five application caller tests, four
direct Mojo programs (including the preceding scalar sanitizer test), strict
workspace Clippy, guards, documentation and the default changed-test runner.
Canonical production share is 36.261584893565875% (135,767 Mojo / 238,643 Rust
production LOC). The 75% project target remains unmet.
