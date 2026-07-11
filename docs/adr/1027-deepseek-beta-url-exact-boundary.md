# ADR 1027: DeepSeek beta base URL exact boundary

## Status

Accepted.

## Context

DeepSeek provider mode can override its beta endpoint through profile config
`deepseek.beta_base_url` or `PRODEX_DEEPSEEK_BETA_BASE_URL`. Runtime resolution
previously trimmed the value and fell back to the default beta endpoint when the
configured value became empty, hiding malformed upstream routing configuration.

## Decision

DeepSeek beta endpoint resolution now treats configured values as exact
upstream routing targets. Empty, whitespace-bearing, non-HTTP(S), or hostless
values fail closed during launch. Existing trailing-slash normalization remains
for valid URLs.

## Consequences

Operators see malformed DeepSeek endpoint overrides at startup instead of
silently routing to a different endpoint. Deployments that want the default
DeepSeek beta endpoint should leave the override unset.
