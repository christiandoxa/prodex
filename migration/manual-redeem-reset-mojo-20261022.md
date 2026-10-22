# Manual redemption reset selection in Mojo (2026-10-22)

## Production ownership

Manual redemption now asks the existing compiled confirmation-policy owner
which primary/weekly reset needs confirmation. Mojo owns saturated signed
reset-minus-now, the near-reset comparison and first minimum selection. The
Rust filtering/minimum loop and per-window eligibility function are deleted.
Rust retains Serde acquisition, the current clock, stable labels, prompts and
the consume request. ABI errors propagate before a credit-consume request;
there is no Rust reset selection fallback.

The new v1 export accepts two canonical optional timestamps and two signed
time facts. Its scalar return is bounded to none/primary/weekly or explicit
version/input errors. Presence tags and absent timestamp words are validated.
Rust validates the returned tag and that a selected timestamp exists, then
preserves the original timestamp. No allocation, text, credential or pointer
crosses this numeric decision ABI.

The existing auto-redemption signed subtraction helper moves into
`runtime_math.mojo`; both policies reuse it. Automatic redemption's original
ABI and eligibility rules remain intact. Manual behavior keeps past-reset
eligibility, negative thresholds, primary-first ties and the full signed
range, including saturated differences at both limits.

## Verification

Direct Mojo assertions cover absent windows, exact thresholds, nearest/tied
resets, past resets, signed saturation and malformed ABI tags. Four strict
confirmation/reset bridge tests cover the same contracts. All six existing
auto-redemption bridge tests pass after extraction, including 20,000 generated
scenarios. Eight production manual-redemption tests cover optional timestamp
acquisition, stable labels, confirmation defaults and a local HTTP fixture's
usage-then-consume sequence.

Strict Cargo checks use Mojo 1.1.0 with `PRODEX_MOJO_REQUIRED=1` and
`PRODEX_MOJO_VERSION=1.1.0`; default CI and changed-test selection omit them.

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/manual_redeem_reset_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib confirmation_policy::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --test auto_redeem_plan`
- `cargo test --locked -q -p prodex-app --lib app_commands::redeem:: -- --test-threads=1`
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --check`
- `node scripts/ci/mojo-no-fallback-guard.mjs --self-test`
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

Direct Mojo, four manual/confirmation bridge tests, six automatic-redemption
bridge tests, eight caller tests and strict workspace Clippy pass. Authority,
ownership, no-fallback self-test, size, crate-boundary self-test/check and
secret-boundary guards pass. Canonical share is 36.30965676420222% (136,549 Mojo
/ 239,519 Rust production LOC). The checked typed adapter grows Rust source
slightly; counting rules and guard thresholds remain unchanged. The 75%
project target remains open.

Default-environment `npm run ci -- --jobs 1` passes (541.32 seconds),
including fast integration coverage. `npm run test:changed` passes (4.12
seconds); its default feature-off Mojo harnesses run zero tests, so the
positive strict bridge/caller counts above provide the ABI evidence.
`cargo fmt --check`, docs, staged churn and `git diff --check` also pass.

Native non-Linux, live-provider and exhaustive full-workspace tests remain
outside this offline unit.
