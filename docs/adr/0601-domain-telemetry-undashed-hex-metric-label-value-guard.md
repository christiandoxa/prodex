# ADR 0601: Domain Telemetry Undashed Hex Metric Label Value Guard

## Status

Accepted.

## Context

The domain telemetry boundary rejects canonical UUID-shaped metric label values.
Raw identifiers can also appear as 32-character hexadecimal values, including
undashed UUIDs and W3C trace IDs.

## Decision

Metric label validation now rejects 32-character hexadecimal values in addition
to canonical dashed UUID values.

## Consequences

Raw request, call, tenant, principal, audit, and trace identifiers remain
trace-only. Closed categorical metric values remain accepted.
