# ADR 0701: Guardrail Keyword Values Are Preserved Exactly

## Status

Accepted.

## Context

`gateway.guardrails.blocked_keywords` and
`gateway.guardrails.blocked_output_keywords` are literal text patterns used by
gateway guardrails. Runtime config resolution trimmed these values before
building the active guardrail config, changing the configured pattern text.

## Decision

Guardrail keyword values are preserved exactly. Blank-only keyword entries are
rejected, and non-blank values are no longer trim-normalized before runtime
guardrail matching.

## Consequences

Existing keyword patterns without leading or trailing whitespace are unchanged.
Policies that deliberately include whitespace in a keyword now keep that exact
matching behavior.
