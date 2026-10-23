# Native RTK wrapper scripts in Mojo (2026-10-23)

Compiled Mojo now owns complete POSIX shell and Windows CMD wrapper text:
quoting, literal apostrophes/percent signs, line endings, recursion and disable
guards, selective subcommand routing, optimizer-state exports and unavailable
tool diagnostics. Rust retains executable discovery, environment/platform
acquisition, native path display, borrowed ABI inputs, output validation, file
writes and executable permissions. The original production Rust templates and
quoting helpers are removed; no Rust fallback reconstructs a script.

The focused `rtk_wrapper.mojo` owner is selected by the real production build.
Its additive version-1 ABI accepts three existing wrapper roles and two existing
platforms, at most 4,096 argument views and 1 MiB total borrowed UTF-8. State paths
are absent or the three host-resolved RTK paths. Complete validation and measured
allocation precede writes. Short output capacity and malformed inputs leave
output and metadata unchanged. Escaping is shared by the wrapper roles inside
Mojo. Native string materialization reuses the existing measured-text bridge.
The new wrapper API follows the runtime feature; provider-constraints-only
consumers retain their existing RTK classification API.

Verification uses `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.2.0`:

- PASS: `rtk cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib rtk_noisy` (8 tests). Complete original Rust outputs are frozen in a test-only directory and compared for 72 platform/role/argument/path combinations. Raw ABI tests cover versions, tags, counts, UTF-8, phases and capacity.
- PASS: `rtk cargo test --locked -q -p prodex-optional-tools` (72 tests). Existing caller tests execute selective/always wrappers; the added local process test passes apostrophes, shell substitutions, percent signs and Unicode as literal arguments.
- PASS: `rtk cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`.
- PASS: `rtk proxy npm run test:node` (282 passed, one native Windows installer test skipped on Linux).
- PASS: release guard fixtures (24), version-metadata guard, crate-boundary/self-test, production boundary, runtime hotpath, ownership, authority, no-fallback/self-test, size and churn guards, formatting and whitespace, offline baseline and five replay checks.
- PASS: optimized object generation, the versioned export and absence of Modular runtime references for all six release targets and their release CPU choices.
- PASS: `rtk cargo check --locked -q -p prodex-mojo-core --features mojo-provider-constraints`; `rtk cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib` (1,112 tests); and `rtk cargo test --locked -q -p prodex-optional-tools --lib rtk::` (6 tests).
- PASS: the previously started `rtk proxy npm run ci` completed, including its default Rust shards and all 282 Node tests. The former benchmark-pin assertion failure is resolved.

The first core command used `--features mojo-rich`, which does not expose the
RTK subsystem and selected zero tests. It is not verification; the corrected
`mojo-core` command above executes the real boundary. The minimal-feature API
review also added an explicit runtime gate for the new measured wrapper bridge.

Canonical production share is 36.897274633123686% (140,448 Mojo / 240,198 Rust
production LOC); the 75% campaign goal remains open. All previous checkpoints
were pushed to main without a release or tag. Native Windows/macOS execution
remains in their CI lanes. The pre-existing Docker scaffold failure remains in
the stable-toolchain report. The previous broad CI run's stale benchmark-pin
assertion is fixed and all related Node tests now pass; the new broad run has now completed successfully.

After the previous checkpoint, the coordinator verified zero active compiler or
Cargo processes, harvested 48 validation artifacts to owned external evidence,
and ran `rtk cargo clean` (16.8 GiB removed). Scoped Trash cleanup checked an empty
user Trash and deleted no files. Current build targets are protected while the
listed checks run. Next migration candidate: request Connection-header token
parsing, reusing the existing Mojo HTTP header owner and preserving request
metadata and Unicode trimming contracts.
