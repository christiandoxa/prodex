# ADR 0777: Redact API version error debug output

## Status

Accepted

## Context

`ApiVersionError` carries requested version identifiers and sunset timestamps
for API lifecycle decisions. Derived `Debug` exposed those details in
diagnostics.

## Decision

Implement custom `Debug` for `ApiVersionError`. Preserve the lifecycle failure
variant shape while redacting requested versions and sunset timestamps.

## Consequences

Diagnostics still show whether version negotiation failed because the version
was unsupported or sunset. Requested version values and sunset timestamps no
longer appear through API-version error debug formatting.
