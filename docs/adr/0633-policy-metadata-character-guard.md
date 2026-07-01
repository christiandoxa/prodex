# 0633: Policy Metadata Character Guard

## Status

Accepted

## Context

Policy snapshots must carry integrity and signature metadata before activation.
The domain boundary rejected empty digest or signature values, but malformed
values with whitespace, control characters, or non-ASCII material could still
reach the validation path.

## Decision

`validate_policy_snapshot` now requires exact `PolicyDigest` and
`PolicySignature` values to be non-empty printable ASCII tokens. Invalid
metadata maps to stable redacted policy activation errors.

## Consequences

- Gateways cannot activate snapshots with malformed integrity metadata.
- Raw digest and signature material stays out of client-visible responses.
- Constructors remain compatibility-oriented; activation is the strict boundary.
