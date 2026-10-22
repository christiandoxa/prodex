# Noncompact compiled-policy adapter cleanup (2026-10-22)

The noncompact loop already delegates its decisions to Mojo. This checkpoint
removes repeated Rust fact marshalling around that existing authority; it does
not claim a new semantic transfer. Six full input initializers become one local
record, copied into candidate admission and refreshed at the original stages.
All six compiled calls remain, with unchanged public types and ABI.

Fresh selection updates candidate presence and the cold-start wait observation.
Recovery resets candidate presence before waiting. A failed cold-start claim
restores the same stage/wait flag and clears transient recovery. The transient
stage samples the wait flag after the same host wait. Admission refreshes its
affinity, saturation and wait facts. Filesystem, locks, selection, waits, logs
and response effects retain their original owners and order.

A path-specific source comparison checks all twelve fields at each of the six
calls, the ordered sequence of 38 host/effect-related calls and every literal
message. This source proof supplements runtime regression coverage; it is not
an independent proof of every execution. A direct Mojo test exercises preferred
affinity, saturation, candidate availability, recovery precedence, invalid
flags/version/pointers and unchanged result sentinels on rejection. No production
Mojo algorithm, dependency, fallback or counting rule changes.

Strict Cargo uses `PRODEX_MOJO_REQUIRED=1` and `PRODEX_MOJO_VERSION=1.1.0`.
Default CI and changed tests omit both variables and any archive override.
Validation commands:

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/runtime_noncompact_loop_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib runtime::noncompact_loop::`
- `cargo test --locked -q -p prodex-app --lib main_internal_tests::runtime_proxy_ -- --test-threads=1`
- `node scripts/ci/test-serial.mjs --suite runtime-smoke`
- `node scripts/ci/runtime-test-manifest-guard.mjs`
- `cargo fmt --check`
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `node scripts/ci/runtime-hotpath-guard.mjs --self-test`
- `node scripts/ci/runtime-hotpath-guard.mjs`
- `node scripts/compat/check-upstream-baseline.mjs`
- `node scripts/compat/capture-replay-fixture-tests.mjs`
- `node scripts/ci/mojo-no-fallback-guard.mjs --self-test`
- `node scripts/ci/mojo-no-fallback-guard.mjs`
- `node scripts/ci/mojo-authority-guard.mjs`
- `node scripts/ci/mojo-ownership.mjs --check --json`
- `node scripts/ci/mojo-production-share.mjs --check --json`
- `node scripts/ci/size-guard.mjs`
- `node scripts/ci/secret-boundary-guard.mjs`
- `node scripts/ci/churn-hygiene.mjs --staged --check`
- `npm run ci -- --jobs 1`
- `npm run test:changed`
- `npm run docs`
- `git diff --cached --check`
- `git diff --check`

The unchanged 459 runtime tests pass before and after the adapter change, with
four bridge and 19 serialized smoke tests. Direct Mojo, strict workspace Clippy,
formatting, manifest, hotpath, compatibility, ownership and secret guards pass.
Full default CI passes in 544.73 seconds with 1,443 Rust tests. Changed tests
pass in 3.21 seconds, with zero selected Rust tests; the strict positive runs
above supply runtime/ABI coverage. Documentation, staged churn and diff checks pass.

Canonical share is 36.465731369527106% (137,707 Mojo / 239,927 Rust production
LOC), a reduction of 55 Rust production LOC. The 75% target remains open.
Native non-Linux execution, live providers and exhaustive workspace/all-serialized
suites are outside local coverage. There are no admission, transport, persistence
or latency-policy changes requiring a new load experiment.
