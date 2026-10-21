# Shared Super expose UTF-8 boundaries in Mojo (2026-10-22)

## Production ownership

Super expose now uses the compiled UTF-8 floor-cut kernel for redacted output,
run event text, aggregate output, exec output and exec audit previews. The four
Rust byte-boundary scanners and two duplicate local helpers are deleted.
Rust keeps lossy decoding, secret redaction, the existing byte budgets, strings,
buffers, process effects and validation of the returned boundary.

The already-reachable launch instruction implementation supplies the pure
algorithm, moved into `rich_text.mojo`. Its existing versioned C export and
Rust adapter retain their contract and call the shared kernel. Super expose
adds operation 8 to its existing v1 exec record ABI, reusing input acquisition,
the rich ABI check, status handling and caller-owned two-word output. Existing
operations keep their numbers and fields; unknown operation 9 is still rejected.
There is no new C export, dependency or configuration surface.

The new operation accepts a full-width unsigned byte budget, validates UTF-8
and output alignment before writes, and returns the largest scalar boundary
within the cap. Empty strings, embedded NUL, all scalar widths and maximum
budgets remain valid. Rust validates bounds, the reserved word and scalar
alignment without searching for another boundary. ABI failures return empty
bounded text or preserve the aggregate buffer; no Rust clipping fallback runs.
Redaction remains before clipping, and stdout events continue after the
aggregate output cap.

## Verification

Direct assertions exercise each byte position across 1-, 2-, 3- and 4-byte
scalars through both the existing Copilot export and Super operation 8. They
also cover empty input, maximum budgets, bad versions/opcodes, invalid UTF-8
and atomic rejection of malformed output alignment. Compiled bridge tests
validate the same edges. Production caller regressions cover lossy decoding,
redaction, run text/aggregate caps, later events and exec output boundaries.
The existing Copilot file-reader tests preserve its post-lossy clipping.

Strict Cargo checks use Mojo 1.1.0 with `PRODEX_MOJO_REQUIRED=1` and
`PRODEX_MOJO_VERSION=1.1.0`; default CI and changed-test selection omit them.

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/super_expose_text_cut_test.mojo`
- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/launch_copilot_instruction_policy_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib rich::super_expose::exec::`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib launch_config::instructions::`
- `cargo test --locked -q -p prodex-app --lib super_expose:: -- --test-threads=1`
- `cargo test --locked -q -p prodex-app --lib compiled_copilot_instruction_ -- --test-threads=1`
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

An initial scratch assertion used an inferred `Int` against `Int64` ABI output;
the explicit test cast fixes that compiler error. The first caller test build
also exposed missing private fixture imports; explicit test imports fix it.
Neither failure required a production algorithm change. Native non-Linux,
live-provider and full-workspace test execution remain outside this offline
unit. The 75% goal remains open.

Both direct Mojo programs, all eight Super exec bridge tests, all three launch
instruction bridge tests, all 111 Super expose tests and both compiled Copilot
caller regressions pass. Authority, ownership, no-fallback self-test, size,
crate-boundary self-test/check and secret-boundary checks pass. Canonical share
is 36.31346167151882% (136,458 Mojo / 239,320 Rust production LOC).

Strict workspace Clippy, final documentation/whitespace checks, the full local
CI gate and changed-test runner pass. CI includes 685 runtime-launch tests,
43 auto-rotate tests and 282 passing Node tests; the Windows installer test is
skipped on Linux. Feature-off zero-test core harnesses in changed-test selection
are not counted as compiled ABI coverage.
