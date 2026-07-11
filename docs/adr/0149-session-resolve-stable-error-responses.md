# ADR 0149: Session resolve stable error responses

## Status

Accepted

## Context

`prodex-session-store` resolves Codex session metadata for resume flows and
runtime affinity diagnostics. Raw `SessionResolveError` values include the
selector supplied by the user and, for ambiguous prefixes, the matching session
IDs. These identifiers are useful in trusted CLI diagnostics but should not be
copied into generic API or operator-facing response envelopes.

## Decision

Add `plan_session_resolve_error_response` to `prodex-session-store`. It maps
missing and ambiguous session resolution failures to stable, redacted response
plans:

- `session_not_found`
- `session_selector_ambiguous`

## Consequences

Future resume/control-plane adapters can report session resolution failures
without leaking session selectors, session IDs, paths, or affinity metadata.
