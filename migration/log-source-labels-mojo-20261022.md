# Human log source labels in Mojo (2026-10-22)

## Production ownership

Human log headers now call `prodex_mojo_log_event_label_v1`. Rust's complete
source match and unknown-source formatting are removed without a fallback.
The compiled renderer reuses the existing canonical source catalog for common
labels, applies UI role/transport aliases and preserves the tool-call prefix
and unknown-source rules. JSON/persisted source names keep their existing
contracts; this operation supplies human display text only.

The canonical source table is extracted into one pure view helper shared by
its unchanged numeric C facade and the new display renderer. A shared label
copy primitive preserves the numeric facade's length/capacity behavior. The
new text ABI validates UTF-8, version, phase, alignment and 64 MiB input/output
bounds before writes, and capacity failures preserve its output canaries.

Rust's caller-owned allocation/status/UTF-8 checks are shared between detail
rendering and label rendering. This private transport helper adds no display
policy and rejects inconsistent lengths instead of recomputing text. No new
dependency, counting-rule change, source-table copy or runtime fallback exists.

## Compatibility

Common labels remain uppercase. `ws`, turn/session context, prompt engineering,
tool output and user/assistant/reasoning roles retain their display aliases.
`tool-call:` matches even with an empty suffix. Matching stays case-sensitive;
unknown source text, Unicode, whitespace and controls keep the original
`stream ` prefix behavior without trimming or truncation.

The older numeric label facade still returns length `-1` for no source and
reports required length on short capacity. The new measured display API keeps
its distinct atomic-error contract. Boundary tests cover both behaviors.

## Verification

Strict Cargo checks use `PRODEX_MOJO_REQUIRED=1` and
`PRODEX_MOJO_VERSION=1.1.0`; default CI and changed tests omit both.

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/log_event_label_test.mojo`
- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/operational_log_details_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib observability::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-observability --lib observability::`
- `cargo test --locked -q -p prodex-app --lib app_commands::log::log_stream::label_tests:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib app_commands::log:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib app_commands::log_tui -- --test-threads=1`
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
- `node scripts/ci/churn-hygiene.mjs --staged --check`
- `npm run ci -- --jobs 1`
- `npm run test:changed`
- `npm run docs`
- `git diff --cached --check`
- `git diff --check`

Both direct Mojo suites, 20 bridge tests in each feature configuration, two
production-header regressions, all 104 log and 17 TUI tests pass. Header
regressions also passed against the original implementation, without changing
expectations after migration. Strict workspace Clippy and formatting pass.
Full default CI passes in 505.44 seconds; changed tests pass in 2.54 seconds,
as do documentation and final guards. The native Windows installer test is
intentionally skipped on Linux; feature-off zero-test harnesses do not replace
the strict positive Mojo bridge/caller counts.

Evidence preserves an initial test import error and a Mojo range type error;
the existing renderer re-export and explicit range index type corrected them.
Canonical share is 36.399797132774474% (137,082 Mojo / 239,519 Rust production
LOC). Guard thresholds and accounting are unchanged. The 75% target remains
open. Native non-Linux execution, live-provider traffic and exhaustive
workspace/serialized runtime suites remain outside local coverage.
