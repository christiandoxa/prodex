# 0635: Restore Checksum Character Guard

## Status

Accepted

## Context

Restore validation rejected missing, blank, and mismatched checksums. A
non-blank checksum containing whitespace, control characters, or non-ASCII text
could still be treated as integrity metadata.

## Decision

`RestorePlan::validate` now requires actual and expected checksum values to be
non-empty printable ASCII tokens.

## Consequences

- Restore plans cannot treat malformed checksum material as an integrity proof.
- Raw checksum values remain available only to trusted diagnostics.
- Client-visible restore errors stay redacted through the existing response
  planner.
