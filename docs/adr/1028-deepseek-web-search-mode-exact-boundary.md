# ADR 1028: DeepSeek web search mode exact boundary

## Status

Accepted.

## Context

DeepSeek provider mode can select web-search compatibility behavior through
profile config `deepseek.web_search_mode` or
`PRODEX_DEEPSEEK_WEB_SEARCH_MODE`. Runtime resolution previously trimmed the
value and silently used the default mode when a configured value was empty or
unsupported.

## Decision

DeepSeek web-search mode resolution now treats configured values as exact
feature-mode selectors. Empty, whitespace-bearing, non-string config, or
unsupported values fail closed during launch. Unset configuration still uses
the default mode, and the existing accepted aliases remain supported.

## Consequences

Operators see malformed DeepSeek web-search mode overrides at startup instead
of silently running with a different compatibility mode. Deployments that want
the default mode should leave the override unset.
