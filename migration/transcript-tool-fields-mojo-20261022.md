# Transcript function/custom tool fields in Mojo (2026-10-22)

## Production ownership

Function/custom calls and their results now use the same compiled projection
as shell records. Typed operations 10 (`FunctionCall`), 11 (`CustomCall`) and
12 (`ToolOutput`) extend the existing v1 ABI without changing its shape. The
shared tool renderer now owns name/type/default selection, stable source
prefixes, arguments/input choice and string-only result eligibility.
Rust's three field-building branches are removed; there is no new bridge,
parser, dependency or Rust fallback.

Rust retains Serde acquisition and the existing compiled tool-name and
visible-output normalizer invocations. Raw argument/input strings remain in
the borrowed tree, so Mojo does not accidentally substitute their bounded
scalar presentations. Function/custom results use the full compiled visible
output presentation; shell results retain their distinct scalar-first path.
Final redaction, safe-text limits and event materialization keep their owners.

The compiled helper is shared across shell/function/custom records. Old modes
0–9 remain compatible. Tree/view validation, measured capacity and rejection
before output mutation remain on the same exported ABI. Unknown-mode fixtures
now use the signed maximum while the guard pins the supported upper bound.

## Compatibility

Function calls read only string `arguments`; custom calls read only string
`input`. Missing/non-string bodies produce blank candidates, which the existing
caller filters. Names use the existing sanitizer when string-typed, including
its empty-name `tool` result. Missing/non-string names use `tool` for functions
and `custom-tool` for custom calls. The source prefix remains `tool-call:`.

Both function/custom output families require a string `output`; they never
use shell aliases, booleans or containers. Their full visible text, ANSI
normalization, control behavior, mojibake rejection and final 65,536-byte bound
remain intact. Raw call payloads keep whitespace/newlines and reach final
safe-text normalization without the shell scalar's 192-character limit.

## Verification

Strict Cargo checks use Mojo 1.1.0 with `PRODEX_MOJO_REQUIRED=1` and
`PRODEX_MOJO_VERSION=1.1.0`; default changed-test selection omits them.

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/log_transcript_shell_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib json::log_transcript_context::`
- `cargo test --locked -q -p prodex-app --lib app_commands::log::tool_tests:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib app_commands::log:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib super_expose:: -- --test-threads=1`
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --check`
- `node scripts/ci/mojo-no-fallback-guard.mjs --self-test`
- `node scripts/ci/mojo-no-fallback-guard.mjs`
- `node scripts/ci/mojo-authority-guard.mjs`
- `node scripts/ci/mojo-ownership.mjs --check --json`
- `node scripts/ci/mojo-production-share.mjs --check --json`
- `node scripts/ci/size-guard.mjs`
- `node scripts/ci/super-wildcard-guard.mjs`
- `node scripts/ci/secret-boundary-guard.mjs`
- `npm run docs`
- `npm run test:changed`
- `git diff --check`

All listed checks pass: direct Mojo assertions, ten strict projection bridge
tests, three new caller tests, all 88 log tests, all 120 Super expose tests,
workspace Clippy and formatting. Default `npm run test:changed` passes (8.83
seconds); its feature-off zero-test harnesses do not replace strict positive
ABI/caller counts. Canonical share is 36.36187225536168% (136,789 Mojo
/ 239,399 Rust production LOC). Counting rules, baselines and guard thresholds
are unchanged. The 75% goal remains open. Native non-Linux, live-provider,
full default CI and exhaustive workspace tests are outside this focused unit.
