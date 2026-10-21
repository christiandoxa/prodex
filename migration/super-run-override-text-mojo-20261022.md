# Super run override text reuses Mojo validation (2026-10-22)

## Production ownership

The four run override fields now reuse the existing compiled bounded-text
validator. Mojo decides nonempty byte limits, embedded NUL and Unicode control
rejection. Rust's duplicate content predicates are deleted. Serde acquisition,
missing/null handling, field-specific budgets, owned strings and applying
configuration stay in Rust. Ordinary error text and validation order remain
unchanged; ABI errors propagate without a Rust validation fallback.

Operation 9 extends the existing v1 exec ABI and calls its existing pure
path-text validator. It shares the current rich ABI check, input views,
two-word result and status handling; no new C export or parallel content
algorithm is introduced. UTF-8 and output alignment are checked before writes.
The Rust adapter validates both returned words. Full-width budgets, exact
UTF-8 byte caps and zero budgets retain their original meaning.

The UTF-8 clipping regressions use unknown opcode 99 so the deliberate
addition of operation 9 preserves their invalid-opcode contract. Existing
clipping, exec validators and their opcode assignments remain intact.

## Verification

Direct Mojo checks cover empty input, exact multibyte caps, maximum budgets,
NUL, Unicode C1 controls and failure atomicity. Compiled bridge checks also
preserve non-control Unicode whitespace and format characters. Production
`apply_overrides` regressions keep null handling and type errors, unchanged
values after invalid model text, a 256-byte Unicode model and untrimmed text.
The whole Super expose namespace exercises the previous run/event and text
clipping behavior with the expanded ABI.

Strict Cargo checks use Mojo 1.1.0 with `PRODEX_MOJO_REQUIRED=1` and
`PRODEX_MOJO_VERSION=1.1.0`; default CI and changed-test selection omit them.

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/super_expose_bounded_text_test.mojo`
- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/super_expose_text_cut_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib rich::super_expose::exec::`
- `cargo test --locked -q -p prodex-app --lib super_expose::run::config::tests:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib super_expose:: -- --test-threads=1`
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

The first no-fallback self-check exposed overlapping fixtures for the existing
override-removal rule. The new fixture includes that rule's production marker;
function-scoped matching keeps both checks active without changing a threshold.
Native non-Linux, live-provider and full-workspace tests remain outside this
offline unit. The 75% project target remains open.

Both direct Mojo programs, all ten strict Super exec bridge tests, both override
caller regressions and all 113 Super expose tests pass. Authority, ownership,
no-fallback self-test, size, crate-boundary self-test/check and secret-boundary
checks pass. Canonical share is 36.31324608695189% (136,463 Mojo / 239,331 Rust
production LOC). Checked bridge growth slightly lowers raw share; counting
rules and guard thresholds remain unchanged.

Strict workspace Clippy, final documentation/whitespace checks, the full local
CI gate and changed-test runner pass. CI includes 685 runtime-launch tests,
43 auto-rotate tests and 282 passing Node tests; the Windows installer test is
skipped on Linux. Feature-off zero-test core harnesses in changed-test selection
are not counted as compiled ABI coverage.
