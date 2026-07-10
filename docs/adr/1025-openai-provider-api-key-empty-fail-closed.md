# ADR 1025: OpenAI provider API-key values fail closed

## Status

Accepted.

## Context

The OpenAI-compatible gateway provider and Mem0 memory gateway can read upstream
credentials from `--api-key`, `OPENAI_API_KEYS`, `OPENAI_API_KEY`, or selected
profile `auth.json`. Direct runtime resolution previously trimmed these inputs
and dropped empty values, allowing an explicitly empty credential source to
disappear or fall through to a different source. Singular key values could also
be whitespace-normalized into a different credential than operators supplied.

## Decision

OpenAI-compatible provider credential resolution now fails closed when
`--api-key`, `OPENAI_API_KEYS`, or `OPENAI_API_KEY` is present but resolves to
no non-empty key. Singular `--api-key` and `OPENAI_API_KEY` values must also be
whitespace-free. Mem0 memory gateway selected-profile `auth.json`
`OPENAI_API_KEY` values use the same exact singular-key boundary.
`OPENAI_API_KEYS` remains a comma-, semicolon-, or newline-separated list. Unset
values still mean "no upstream key configured", which is allowed until gateway
data-plane auth is enabled and a separate upstream key is required.

## Consequences

Malformed provider credential wiring is visible at startup. Deployments that
intentionally omit OpenAI-compatible upstream credentials should leave the
credential inputs unset rather than setting empty environment variables.
