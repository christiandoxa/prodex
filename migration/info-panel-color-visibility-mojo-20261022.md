# Main info-panel color and visibility in Mojo (2026-10-22)

## Production ownership

The main `prodex info` panel now uses the existing compiled info-render ABI
for value color and five-hour field visibility. The two new operations are
35 (color) and 36 (visibility) on `prodex_terminal_info_render_v1`. The pending
migration was integrated against current main in an isolated checkout; the
original unfinished work remains preserved.

Mojo reuses the existing doctor ASCII substring comparator. It owns warning
precedence, positive status words, the exact quota-label unavailable-zero
exception, exact active-profile label, runtime/Codex label coloring and the
two exact five-hour field names. Existing substring behavior remains intact,
including `unavailable=01` and `nothing` matching `thin`. Labels retain their
case sensitivity, while values use ASCII case folding. Unicode and embedded
NUL are preserved. Every positive observation count shows five-hour fields.

Rust's lowercase allocation, keyword branches, quota exception and field-name
predicate are deleted. Rust keeps quota acquisition, terminal effects, text,
layout and Ratatui colors. The bridge validates compact ASCII results into
`InfoPanelValueColor` or bool; the caller exhaustively materializes the color.
No Rust semantic fallback or duplicate test oracle remains.

The v1 ABI checks mode-specific text/count shapes before output mutation.
Existing operations and the legacy invalid-version status stay unchanged.
No dependency, counting-rule, baseline, threshold or release changes occur.

## Verification

Strict Cargo checks use Mojo 1.1.0 with `PRODEX_MOJO_REQUIRED=1` and
`PRODEX_MOJO_VERSION=1.1.0`. Direct assertions cover colors, visibility,
invalid mode/shape/version and unchanged output after rejection. Bridge
fixtures cover warning precedence, exact labels, substring edge cases,
Unicode/NUL and the full host count range. Caller tests reach the real panel
text, color and field suppression helpers.

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/info_panel_policy_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib info_render::`
- `cargo test --locked -q -p prodex-app --lib app_commands::info_handler::tests:: -- --test-threads=1`
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --check`
- `node scripts/ci/mojo-no-fallback-guard.mjs --self-test`
- `node scripts/ci/mojo-no-fallback-guard.mjs`
- `node scripts/ci/mojo-authority-guard.mjs`
- `node scripts/ci/mojo-ownership.mjs --check --json`
- `node scripts/ci/mojo-production-share.mjs --check --json`
- `node scripts/ci/size-guard.mjs`
- `node scripts/ci/super-wildcard-guard.mjs`
- `node scripts/ci/crate-boundary-guard.mjs --self-test`
- `node scripts/ci/crate-boundary-guard.mjs`
- `node scripts/ci/secret-boundary-guard.mjs`
- `npm run docs`
- `npm run test:changed`
- `git diff --check`

All listed checks pass. The strict bridge filter runs 23 tests, including the
two new panel regressions; all five production caller tests pass. Workspace
Clippy and formatting pass. Default `npm run test:changed` passes (2.58
seconds); its feature-off zero-test harnesses do not replace the positive
strict bridge/caller runs. Canonical share is 36.348485291498655% (136,747 Mojo
/ 239,464 Rust production LOC); the 75% target remains open. The first added
negative fixture assumed status 4 for a bad version; source inspection showed
the existing renderer returns 1, and the assertion was corrected.

Full default CI passed for the immediately preceding session-JSON checkpoint;
it is not repeated for this small panel policy unit. Native non-Linux, live
providers and exhaustive full-workspace tests remain outside this offline unit.
