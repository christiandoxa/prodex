# Transcript shell field policy in Mojo (2026-10-22)

## Production ownership

Shell calls and results now use the existing compiled transcript projection
ABI with two typed operations: 8 (`ShellCall`) and 9 (`ShellOutput`). Mojo
chooses `command`/`arguments`/`action`, `output`/`aggregated_output`/`stdout`,
name/default precedence, container eligibility and the stable source label.
The Rust shell-call/output builders and scalar/JSON fallback helpers are
deleted. The shared operation renderer and generic Serde acquisition are
reused; no bridge, parser, dependency or fallback implementation is added.

Rust retains file/clock effects, Serde number/container spelling, final event
materialization and redaction. Root members are acquired once. Strings use
the existing compiled operation-span policy; shell-call names use the
compiled tool-name sanitizer and shell results use the compiled visible-tool
projection. Protocol/status presentations keep their previous path. The host
only supplies these native/library inputs and applies the compiled record.

The existing v1 ABI retains borrowed validated trees, two presentation views
per node, measured capacity, checked UTF-8/topology and no writes on rejected
input or insufficient capacity. Modes 0–7 stay compatible. Both new modes
are dispatched by the real production classifier and caller.

## Compatibility

Presence blocks fallback before type checking. Null/blank/rejected command
or output fields never select a later alias. Name fallback instead follows
string type: absent/non-string names use `shell`, while a present empty name
retains the sanitizer's `tool` result. Scalar false/number values and complete
Serde arrays/objects retain their spelling and ordering.

Existing string rejection of control characters and the 192-character scalar
bound remain unchanged. Existing tool-output mojibake heuristics also stay
intact. Containers retain their full JSON presentation until
the final 65,536-byte safe-text limit. Final redaction and blank-event filtering
remain with their existing owners. No prompts, credentials or live provider
traffic are used as fixtures.

## Verification

Strict Cargo checks use Mojo 1.1.0 with `PRODEX_MOJO_REQUIRED=1` and
`PRODEX_MOJO_VERSION=1.1.0`. Default changed-test selection omits them.

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/log_transcript_shell_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib json::log_transcript_context::`
- `cargo test --locked -q -p prodex-app --lib app_commands::log::shell_tests:: -- --test-threads=1`
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
- `node scripts/ci/crate-boundary-guard.mjs --self-test`
- `node scripts/ci/crate-boundary-guard.mjs`
- `npm run docs`
- `npm run test:changed`
- `git diff --check`

All listed checks pass: direct Mojo assertions, seven strict projection bridge
tests, three new caller tests, all 85 log tests, all 120 Super expose tests,
workspace Clippy and formatting. Default `npm run test:changed` passes (4.06
seconds); its feature-off zero-test harnesses do not replace strict positive
ABI/caller counts.

Initial caller fixtures expected visible repeated
non-ASCII tool output and a retained Bearer prefix inside an authorization
JSON field; source inspection confirmed existing mojibake rejection and whole
field redaction. Fixed those expectations without changing production. Clippy
also required the negative ABI test to acquire its view pointer after mutation;
no lint or guard was suppressed. Canonical share is 36.35691843125163% (136,774 Mojo
/ 239,424 Rust production LOC). Counting rules, baselines and thresholds stay
unchanged; the 75% objective remains open. Native non-Linux, live-provider,
full default CI and exhaustive workspace tests are outside this focused unit.
