# ADR 0850: Redact gateway HTTP plan display header metadata

Status: Accepted

## Context

`GatewayHttpPlanError` distinguishes duplicate authorization, account, session,
Codex metadata, and trace-context headers. Its `Display` output named those
headers directly, so generic local error formatting could expose request
metadata topology.

## Decision

Keep the typed variants for routing and response planning, but render missing
or duplicate request metadata with generic `Display` messages.

## Consequences

Gateway HTTP response planning remains unchanged, while stringified local plan
errors no longer expose header names.
