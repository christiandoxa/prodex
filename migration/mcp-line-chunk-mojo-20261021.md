# MCP physical line chunk planning in compiled Mojo (2026-10-21)

## Production ownership

The existing `mcp_stdio_policy.mojo` owner now returns a bounded copy/consume plan
through `prodex_mcp_line_chunk_v1`. Both first-line reading and continuation
header reading in `prodex-mcp-stdio` consume the same compiled plan. The old
Rust LF searches, ASCII first-byte scans, automatic header/message limit
selection, overflow decisions and last-byte completion checks are deleted.

Mojo preserves the original first non-whitespace byte across BufRead chunks,
selects the 4 KiB first-header or 64 MiB JSON-line cap, and handles fixed caller
limits. Rust ASCII whitespace excludes vertical tab; the Mojo predicate
preserves that distinction. The plan stops at the first LF without consuming
bytes belonging to a following message. Unsigned overflow is avoided before
any buffer growth.

Rust retains pipe/stdio I/O, EOF handling, allocation, copying/consumption,
completed-line UTF-8 decoding and Serde parsing. Borrowed binary chunks can end
inside a UTF-8 scalar and are never individually decoded. The same first-byte
classifier serves the existing scalar ABI and the new chunk planner; there is
no Rust scanner or classification fallback.

## ABI and regression coverage

The v1 ABI validates version/mode, raw byte views, byte-state tags, output
alignment and bounded limits before writing its five fixed-width result words.
The host bounds a borrowed fill buffer to the existing message cap plus one
byte and validates the returned range, byte state, flags and limit. A zero-byte
plan for a non-empty input is rejected to prevent a stalled read loop.

Direct Mojo tests cover LF boundaries, limit modes, unsigned overflow, vertical
tab, empty buffers, invalid ABI/mode/state and unchanged output on rejection.
Compiled bridge tests additionally cover every possible first byte, split
classification, exact/over limits, malformed raw UTF-8, invalid pointers and
maximum buffered lengths. Production caller tests read UTF-8 and framed JSON
through BufReader capacities of one, two, three and more bytes, retain the next
message, and enforce split-header limits.

## Validation commands

Use Mojo 1.1.0 and `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.1.0`
for direct compiled Cargo checks:

- `mojo run -D ASSERT=all -I mojo/prodex_core mojo/tests/mcp_line_chunk_test.mojo`
- `cargo test --locked -q -p prodex-mojo-core --features mojo-core --lib mcp_stdio_policy::`
- `cargo test --locked -q -p prodex-mcp-stdio`
- `cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --check`
- `node scripts/ci/mojo-no-fallback-guard.mjs --self-test`
- `node scripts/ci/mojo-authority-guard.mjs`
- `node scripts/ci/mojo-ownership.mjs --check --json`
- `node scripts/ci/mojo-production-share.mjs --check --json`
- `node scripts/ci/size-guard.mjs`
- `npm run docs`
- `npm run test:changed`
- `git diff --check`

The changed-test runner retains its default feature selection; strict compiled
bridge/caller tests run separately. The 75% project target remains open.
Live providers and native non-Linux execution are outside this checkpoint.

The direct Mojo program, five strict compiled bridge tests, all 19 MCP tests,
three CLI subprocess regressions, strict workspace Clippy, guard self-tests,
documentation and the default changed-test runner pass. CLI subprocess tests
isolate profile homes, Codex/Claude homes and runtime logs in a session-owned
temporary directory, removed after the children exit. The offline Codex baseline
and five replay fixture checks pass.

After rebasing onto the session-store checkpoint, five strict bridge tests,
19 MCP tests, workspace Clippy, the merged no-fallback guard self-test,
authority/size/share guards and the changed-test runner pass again. Canonical
production share is 36.278758251903874% (135,959 Mojo / 238,803 Rust production
LOC). The 75% project target remains unmet.
