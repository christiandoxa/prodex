# ADR 1026: External provider API-key values fail closed

## Status

Accepted.

## Context

Anthropic, Copilot, DeepSeek, and Gemini provider modes can read upstream
credentials from `--api-key` or provider-specific `*_API_KEY(S)` environment
variables. Runtime resolution previously trimmed these inputs and skipped empty
values, so an explicitly empty plural env var could fall through to a different
single-key source, an empty CLI key could be treated as absent, and padded
singular keys could be silently normalized.

## Decision

External provider API-key resolution now fails closed when a configured CLI key,
plural env var, or single-key env var is present but resolves to no non-empty
key. Singular CLI and single-key env values are exact secret values: they must
not be empty or contain whitespace. Plural `*_API_KEYS` inputs retain their
documented separator-list parsing and still trim individual list entries. Unset
credential inputs continue to mean "try provider profile/OAuth mode" where that
provider supports it.

## Consequences

Malformed provider credential wiring is visible during launch instead of being
silently ignored. Deployments that intend to use profile/OAuth auth should leave
API-key inputs unset rather than setting empty environment variables. Deployments
that use singular provider keys must provide the exact bearer value without
padding.
