# ADR 0805: Redact model route candidate debug output

Status: Accepted

## Context

`ModelRouteCandidate` carries provider and model route names used during
capability negotiation. Its derived `Debug` formatter exposed raw route topology
to diagnostics and containing debug output.

## Decision

Use a custom `Debug` implementation for `ModelRouteCandidate` that redacts
provider and model names while preserving the capability set.

## Consequences

Diagnostics can still explain capability compatibility, but raw provider/model
route names no longer appear through model route candidate debug output.
