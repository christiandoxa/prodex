# ADR 0885: Config secret reference display redaction

## Status

Accepted.

## Context

Config secret publication rejects raw secret material and malformed secret
references before producing a publication plan. Response planners already use a
stable redacted message, but raw `Display` output still named `SecretRef` and
distinguished malformed reference shape from raw material.

## Decision

Render all `ConfigSecretReferenceError` variants as
`configuration secrets must use secret references`, matching the response
planner. Keep typed variants and response codes unchanged so callers can still
classify required-reference and malformed-reference failures.

Regression coverage pins the exact display string and rejects `SecretRef`
wording from raw display output.

## Consequences

Config boundaries can safely fall back to raw display output without exposing
secret-reference implementation names or validation shape. Trusted diagnostics
should match typed variants for the exact reason.
