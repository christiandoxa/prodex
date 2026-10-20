# Runtime Log Scanner Filename Classification Moved to Mojo (2026-10-20)

## Production ownership

The existing compiled log throughput and retention policy owner now
classifies scanned runtime-log basenames through operation 8 of the
versioned prodex_log_retention_policy_v1 ABI. The operation validates
the byte lengths and pointers, matches the caller-supplied UTF-8 prefix
exactly, prioritizes the .lock suffix over .log, and returns a typed
ignore/lock/log tag. The typed Rust adapter rejects unknown tags.

The previous Rust filename prefix/suffix selection was deleted from the
live disk scanner. Rust remains responsible for reading the directory,
managing locks, enforcing symlink and regular-file safety checks, reading
mtime, and deleting files according to already compiled expiry and
retention plans. This migration does not alter filesystem safety checks
or introduce a Rust policy fallback.

## Verification

- Direct compiled Mojo test exercises Unicode paths, prefix mismatches,
  .lock priority, .log recognition, and ABI version handling.
- Two direct Rust core ABI tests pass, including byte-overflow rejection.
- The real scanner caller test covers accepted .log and .lock entries,
  irrelevant case-sensitive names, ignored directories and symlinks.
- All 29 prodex-runtime-log crate tests pass.
- Mojo no-fallback self-test rejects restored Rust filename predicates.
- Cargo format, diff checks, and the official production-share check pass.

The official broad share rises from 35.76207093143992% to
35.76609372332399% (129,886 Mojo / 233,268 Rust production lines).
The 75% project objective remains unmet. Measurement rules and release
floor have not been weakened. No remote CI green prerequisite.
