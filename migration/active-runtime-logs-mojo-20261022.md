# Active runtime-log selection in Mojo (2026-10-22)

Info-report log selection now delegates PID parsing, runtime-process membership,
last-encountered retention and ascending PID order to Mojo. Rust's prefix/split /
integer parser and set/map selection algorithm are removed without a fallback.
Native Path/OsStr basename and UTF-8 acquisition, original path ownership and
filesystem effects stay in Rust. The existing core-file decimal parser and
shared index heapsort are reused by one focused active-log owner.

The version-1 scalar ABI preserves exact prefix-plus-hyphen matching, optional
ASCII plus signs, leading zeros, the full u32 range and an arbitrary trailing
segment after the next hyphen. The version-1 batch ABI accepts numeric process
facts and optional PID observations in caller-owned arenas. It validates byte
and count bounds, alignment, versions, flags and late malformed rows before
writing scratch or output. It adds no fixed row cap. Mojo chooses ascending PID
order with the last input index for duplicate active PIDs. Rust validates unique
in-range indices and applies the resulting plan.

An empty-path query preserves the original return before consuming the supplied
path iterator when no runtime process exists. Both query and selection consume
the same compiled policy. Invalid native filenames retain host-side exclusion.
No new dependency, Rust fallback or counting-rule change is introduced. The
necessary checked ABI adapter increases Rust source while replacing the original
Rust decisions; raw source share is reported without adjusting the metric.

Strict Cargo sets `PRODEX_MOJO_REQUIRED=1` and `PRODEX_MOJO_VERSION=1.1.0`.
Default CI/changed tests omit both and any archive override. Commands:

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/runtime_active_logs_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib log::active_paths::tests::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib core_file_policy::`
- `cargo test --locked -q -p prodex-app --lib reports::info::processes::active_paths_tests:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib reports::info:: -- --test-threads=1`
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --check`
- `node scripts/ci/mojo-no-fallback-guard.mjs --self-test`
- `node scripts/ci/mojo-no-fallback-guard.mjs`
- `node scripts/ci/mojo-authority-guard.mjs`
- `node scripts/ci/mojo-ownership.mjs --check --json`
- `node scripts/ci/mojo-production-share.mjs --check --json`
- `node scripts/ci/size-guard.mjs`
- `node scripts/ci/secret-boundary-guard.mjs`
- `node scripts/ci/super-wildcard-guard.mjs`
- `node scripts/ci/churn-hygiene.mjs --staged --check`
- `npm run ci -- --jobs 1`
- `npm run test:changed`
- `npm run docs`
- `git diff --cached --check`
- `git diff --check`

Four caller regressions pass before/after migration, with direct Mojo, three
active-log bridge, three core-file and 27 info-report tests. Strict workspace
Clippy, formatting and ownership/authority/no-fallback/size/secret guards pass.
Full default CI passes in 732.74 seconds with 1,443 Rust tests. Changed tests
pass in 13.12 seconds with zero selected Rust tests; the strict positive ABI
and caller runs above supply coverage. Documentation, staged churn and diff
checks pass.

An optional `cargo test --locked -q -p prodex-mojo-core --features mojo-rich,mojo-routing --lib log::active_paths::tests::`
experiment fails to link existing runtime/session symbols without `mojo-runtime`.
The missing-symbol callers and all pre-existing build-selection gates match the
published parent. This is a source-level comparison, not a separate baseline
build. The failure is retained; feature dependencies and guards were unchanged
at that checkpoint.
The complete `mojo-core` configuration and real application callers pass.
A prototype comparator needed explicit Int64 conversion before its direct test
passed; that compiler failure is also retained. The first CI run stopped at
the existing wildcard-import cap. The new caller tests now use explicit imports;
the unchanged cap, caller regressions, strict Clippy and formatting pass again.

Canonical share is 36.481147777398505% (137,867 Mojo / 240,046 Rust production
LOC). The 75% target remains open. Native non-Linux execution, live providers,
exhaustive workspace/all-serialized suites and performance benchmarks are outside
local coverage. The optional feature failure above is subsequently repaired by
the Cargo dependency change documented in
`migration/rich-runtime-feature-contract-20261022.md`, with 968 passing tests.
