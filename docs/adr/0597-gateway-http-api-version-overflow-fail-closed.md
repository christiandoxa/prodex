# ADR 0597: Gateway HTTP API Version Overflow Fail Closed

## Status

Accepted.

## Context

Gateway HTTP API-version planning extracts explicit `/vN/...` path prefixes
into domain `ApiVersion` values. A numeric prefix larger than `u16` could not
fit the domain type and previously fell back to the default legacy version.

## Decision

Overflowing explicit API-version prefixes now stay explicit and map to the
sentinel unsupported version `u16::MAX.0`, which is rejected by the existing
domain API-version policy.

## Consequences

Oversized explicit versions fail closed with the same stable redacted
`api_version_unsupported` envelope. Legacy unversioned paths still use the
configured default version.
