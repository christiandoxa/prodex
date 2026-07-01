# ADR 0450: Storage tenant display errors are redacted

## Status

Accepted.

## Context

Tenant-scoped storage planners reject cross-tenant commands before adapter SQL or
cache operations run. Their response planners already expose stable redacted
messages, but local `Display` text still included tenant identifiers for many
tenant mismatch variants.

## Decision

Storage tenant mismatch `Display` messages now report only the failing storage
boundary class. Structured enum fields retain the tenant IDs for trusted
diagnostics and tests.

## Consequences

Accidental display paths no longer expose tenant IDs across billing, audit,
lifecycle, secret-reference, and idempotency storage planners.
