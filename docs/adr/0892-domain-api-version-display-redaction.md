# ADR 0892: Domain API version display redaction

## Status

Accepted.

## Context

API version response planning already uses stable public messages for
unsupported and no-longer-available versions. Raw `Display` output still used
the internal `sunset` lifecycle term for expired versions.

## Decision

Render sunset API-version failures as `API version is no longer available`,
matching the response planner. Keep typed variants and response codes unchanged
so callers can still distinguish unsupported from gone versions.

Regression coverage pins the exact display strings and rejects requested version,
sunset timestamp, and `sunset` wording from raw display output.

## Consequences

API version boundaries can safely fall back to raw display strings without
exposing lifecycle terminology or rejected version metadata. Trusted diagnostics
should match typed variants for exact classification.
