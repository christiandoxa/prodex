# ADR 0702: Observability Call ID Header Rejects Whitespace Padding

## Status

Accepted.

## Context

`gateway.observability.call_id_header` configures the response header used to
propagate gateway call/request IDs. Policy validation and runtime resolution
trimmed the configured header name before using it.

## Decision

The observability call-id header name must be an exact non-empty value without
whitespace. Policy validation rejects whitespace-bearing header names, and
runtime config resolution fails closed instead of trim-normalizing them.

## Consequences

The default `x-prodex-call-id` header remains unchanged. Padded configured
header names no longer silently change propagation behavior.
