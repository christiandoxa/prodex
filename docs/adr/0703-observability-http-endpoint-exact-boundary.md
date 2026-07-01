# ADR 0703: Gateway Observability HTTP Endpoint Exact Boundary

## Status

Accepted.

## Context

`gateway.observability.http_endpoint` selects the HTTP telemetry export target.
The runtime resolver previously trimmed the configured URL before enabling the
HTTP sink, and policy validation also trimmed before URL validation. That made a
padded URL silently become a different accepted endpoint.

## Decision

Treat `gateway.observability.http_endpoint` as an exact configuration boundary.
The value must be non-empty, contain no whitespace, and parse as an HTTP(S) URL
with a host. Runtime launch preserves the accepted value instead of normalizing
it.

## Consequences

Operators must fix padded telemetry endpoint values in `policy.toml` instead of
relying on runtime cleanup. This fails closed and keeps telemetry routing
configuration auditable.
