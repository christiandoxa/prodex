# Live-Log Compact JSON Materialization Migrated to Mojo (2026-10-20)

## Production ownership

The existing compiled live-log-record owner now emits canonical compact
metadata JSON through the versioned prodex_live_log_compact_json_v1 ABI.
The host supplies only the original Serde-canonical JSON values, if
present, for event, pid, and timestamp. Mojo owns field omission,
canonical event/message/pid/timestamp ordering, the fixed truncation
marker, punctuation and trailing newline. Its measured first call
allocates no Mojo heap; the second call writes into exactly sized
Rust-owned storage.

Rust retains JSON parsing, earlier compiled nested-string clipping,
acquisition/serialization of three host-owned values, and application
of the compiled plan to the live-log state. The previous Rust
serialize_compact_live_log_json function, temporary Serde Map,
metadata-key selection loop, and local truncation-marker formatter were
deleted. Mojo errors propagate explicitly, not via a Rust compact
formatter fallback. The existing bounded plain-text behavior for
non-JSON input is unchanged.

## Verification

- All 30 prodex-runtime-log tests pass, including exact key order,
  Unicode/control escaping, null metadata, absent metadata, oversized
  unrelated fields, and existing nested-string and million-event tests.
- Two direct core ABI tests cover exact output and invalid version/row
  shapes plus insufficient capacity with no metadata write.
- Direct Mojo tests exercise the same measured-output, caller capacity,
  ordering and version contract.
- No-fallback guard/self-test, Cargo formatting, diff validation, and
  canonical reachable production-share non-regression checks pass.

The official broad share increases from 35.803093429485415% to
35.80922552817385% (130,342 Mojo / 233,648 Rust production lines).
The 75% project target remains unmet. No counting, baseline or release
policy was altered, and remote CI green is not a prerequisite.
