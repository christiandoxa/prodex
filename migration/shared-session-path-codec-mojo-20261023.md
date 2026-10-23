# Windows session path codec in Mojo (2026-10-23)

The shared attachment policy now owns shortest-backslash-run detection, escaped
run decoding with ceiling division, and Windows session-path rendering. The
original Rust codec is deleted. Rust retains native platform selection, path
display/interpretation, attachment eligibility, secure file opening, copying,
locking and persistence. Non-Windows backslashes retain their literal meaning.

The additive version-1 ABI borrows UTF-8 input and template bytes, validates
versions, operations, phases, lengths, output capacity and metadata alignment,
and uses the existing measured native-text allocator. Invalid arguments and
short capacity leave output and metadata untouched. Measurement counts expanded
runs arithmetically; it never loops over an unallocated expanded output. No Mojo
heap objects or cross-ABI memory ownership are introduced. On Windows the bridge
materializes measured output even when text is unchanged; attachment maintenance
is outside the runtime proxy hot path.

Both existing audited semantic ranges and the new codec range are recorded in
the ownership inventory. The production build already selects this Mojo owner.
The no-fallback guard requires both native codec calls and rejects the restored
Rust escape-width helper. No new dependency or counting-rule change is used.

With `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.2.0`, these commands pass:

- `rtk cargo test --locked -q -p prodex-mojo-core --features mojo-rich --lib shared_attachment_policy` (9 tests, including the new codec parity and invalid-ABI checks).
- `rtk cargo test --locked -q -p prodex-shared-codex-fs --lib image_attachments` (25 tests).
- `rtk cargo test --locked -q -p prodex-shared-codex-fs` (full owning crate).
- `rtk cargo clippy --locked --workspace --all-targets --all-features -- -D warnings`.

The frozen test-only original codec covers Unicode, embedded NUL, UNC paths,
mixed and trailing run lengths, no escaping, and long input. A private caller
test verifies the platform branch without exposing a test-only public API.
Direct production object compilation passes for all six release triples and
their release CPU choices. Windows native execution remains a CI responsibility;
Linux executes the same compiled codec through the Rust bridge.

Additional checks pass: formatting and whitespace, crate-boundary guard and
self-test, ownership, authority, no-fallback/self-test, and production-share guard.
The cookie checkpoint's 459 serialized runtime-proxy cases, runtime-smoke suite
and changed-path checks also pass. The omitted benchmark pin assertion was fixed
in `92ecefc8b`; 52 related Node cases pass, with one native Windows installer test
skipped on Linux. The full default CI run that observed the old assertion is
still finishing its independent Rust shards; its Node failure is not reported
as a green full run.

Canonical share is 36.868677577130605% (140,282 Mojo / 240,209 Rust production LOC).
The 75% target is not met. No release, tag, live-provider traffic or remote-CI wait
is part of this checkpoint. The previous Docker scaffold failure remains
documented in the stable-toolchain checkpoint.
