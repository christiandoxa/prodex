# Blocked quota messages in Mojo

`prodex-quota::collect_blocked_limits` now delegates selection and complete
message composition to `quota_blocked_windows.mojo`. Rust borrows provider
observations, formats local reset times with Chrono, allocates output, and
materializes `BlockedLimit` values. The former Rust message builder and
superseded scalar Mojo selection export are deleted. There is no Rust fallback.

The versioned ABI uses fixed-width observation rows and borrowed UTF-8 views.
Mojo validates flags, row tags, view presence and UTF-8 before writing output.
The existing measured native-string adapter allocates the complete result
without introducing a message-count or 64 MiB text limit. Main quota rows,
additional provider order, empty-name precedence, code-review inclusion and
unavailable-data behavior remain unchanged.

The window-label export and blocked-message renderer share one internal label
policy. Native integration tests exercise optimized production objects; direct
ABI tests cover invalid input, capacity failures and untouched output on
validation failure. Signed duration extremes, Unicode, quotes, newlines, NULs
and 2,049 observations are covered at the Rust caller boundary.

The ownership manifest also follows the already-promoted optional-tool content
policy. Its old tree-compatibility entry referenced a deleted export; it now
points to the live release-identity and digest policy and its actual callers.

The 75% campaign remains open. The release-floor check is a separate gate and
does not prove that the project target has been reached.

Validation at this checkpoint:

- PASS: `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.2.0 cargo test --locked -q -p prodex-mojo-core --no-default-features --features mojo-quota` (45 tests).
- PASS: `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.2.0 cargo test --locked -q -p prodex-quota` (111 tests).
- PASS: `mojo run -I mojo/prodex_core mojo/tests/quota_blocked_windows_test.mojo`.
- PASS: `PRODEX_MOJO_REQUIRED=1 PRODEX_MOJO_VERSION=1.2.0 cargo clippy --locked --workspace --all-targets --all-features -- -D warnings` (vendored dependency warnings remain).
- PASS: `cargo fmt --check`, `npm run docs`, and `git diff --check`.
- PASS: ownership, authority, no-fallback and canonical production-share checks.
- The first `test:changed` invocation set `PRODEX_MOJO_REQUIRED=1` globally and
  failed at its intentionally featureless core build. The normal selector is
  rerun without that environment flag; strict feature-enabled tests pass above.

Canonical share after this slice is 36.97708562739238%: 141,038 reachable Mojo
production LOC and 240,382 Rust production LOC. Measurement rules are unchanged.
Native macOS and Windows execution and remote CI are not claimed here.
