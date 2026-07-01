# 0625. Capability Route Name Guard

## Status

Accepted

## Context

Provider/model capability negotiation chooses a route before provider
invocation. Provider and model names are internal routing metadata that may flow
into diagnostics, telemetry, and adapter selection.

The domain negotiation previously accepted candidates with empty provider/model
names or names containing whitespace, control characters, or non-ASCII text.

## Decision

`ModelRouteCandidate` now exposes `is_well_formed()`. `negotiate_capability`
skips malformed candidates before checking required capabilities.

Route names are considered well-formed only when both provider and model are
non-empty ASCII graphic strings with a 128-byte maximum.

## Consequences

- Malformed provider/model routes cannot be selected for data-plane invocation.
- Existing route candidate construction remains source-compatible.
- Public capability errors stay stable and redacted.
