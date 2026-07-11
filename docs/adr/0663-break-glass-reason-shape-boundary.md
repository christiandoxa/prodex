# 0663. Break-Glass Reason Shape Boundary

## Status

Accepted

## Context

Break-glass authorization requires an explicit human reason and expiry before
emergency control-plane actions can run. The existing check rejected missing
reasons, but it accepted unbounded text and control characters.

Regulated audit evidence needs the reason to be present without allowing
multiline or oversized payloads into control-plane decision records.

## Decision

`decide_break_glass_action` now rejects empty, whitespace-only,
control-character-containing, or over-512-byte reasons before authorization.

Client-visible failures continue to use the redacted
`break_glass_not_authorized` response.

## Consequences

- Break-glass emergency access remains explicit, short-lived, and auditable.
- Control-plane adapters cannot persist malformed break-glass reason text.
- Stable authorization responses do not reveal rejected reason content.
