# ADR 0451: Redis tenant display errors are redacted

## Status

Accepted.

## Context

Redis plans are used for distributed rate limiting, short-lived cache, and
coordination primitives. Redis tenant mismatch response planning already returns
a stable redacted error, but local `Display` text included tenant identifiers.

## Decision

`RedisPlanError::TenantMismatch` now uses a generic display message. Structured
enum fields retain tenant IDs for trusted diagnostics and tests.

## Consequences

Accidental display paths no longer expose tenant IDs from Redis coordination or
rate-limit planning failures.
