# 0418: Gateway HTTP duplicate affinity headers

## Status

Accepted

## Context

`session_id` and `x-codex-turn-state` preserve profile affinity for remote
compact and continued response chains. Duplicate values are ambiguous because
HTTP stacks may choose the first value, choose the last value, or merge them.

## Decision

`prodex-gateway-http` rejects duplicate `session_id` and `x-codex-turn-state`
headers during request planning. The failure uses a generic redacted
`affinity_header_invalid` envelope.

## Consequences

- Continuation affinity does not depend on framework header folding behavior.
- Ambiguous profile ownership fails before routing or upstream commit.
- Raw affinity values remain out of client-visible error messages.
