# 0644: Policy Integrity Metadata Length Boundary

## Status

Accepted

## Context

Policy snapshots carry digest and signature metadata before activation. The
domain validation rejected empty and non-printable metadata, but did not bound
metadata size. A malformed control-plane or gateway adapter could pass oversized
metadata through policy activation checks and into diagnostics or storage.

## Decision

`validate_policy_snapshot` now treats policy digest and signature metadata over
512 bytes as invalid. Empty metadata is distinct from whitespace-containing
metadata because integrity metadata must be exact printable bytes.

## Consequences

- Policy activation remains fail-closed for malformed integrity metadata.
- Stable policy error responses continue to redact rejected metadata values and
  lengths.
- Existing constructors stay unchanged for compatibility.
