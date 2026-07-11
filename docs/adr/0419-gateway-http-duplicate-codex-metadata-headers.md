# 0419: Gateway HTTP duplicate Codex metadata headers

## Status

Accepted

## Context

`x-openai-subagent`, `x-codex-turn-metadata`, and `x-codex-beta-features` are
preserved upstream because Codex uses them to describe turn behavior. Duplicate
values are ambiguous because HTTP stacks may choose the first value, choose the
last value, or merge them before forwarding.

## Decision

`prodex-gateway-http` rejects duplicate Codex metadata headers during request
planning. The failure uses a generic redacted
`codex_metadata_header_invalid` envelope.

## Consequences

- Metadata semantics do not depend on framework header folding behavior.
- Ambiguous turn metadata fails before routing or upstream commit.
- Raw metadata values remain out of client-visible error messages.
