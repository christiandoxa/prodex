# ADR 1029: DeepSeek strict tools exact boundary

## Status

Accepted.

## Context

DeepSeek provider mode can enable strict tool handling through profile config
`deepseek.strict_tools` or `PRODEX_DEEPSEEK_STRICT_TOOLS`. Runtime resolution
previously treated unknown string values as `false`, so malformed explicit
configuration could silently disable stricter tool handling.

## Decision

DeepSeek strict-tools resolution now treats configured values as exact boolean
selectors. Native TOML booleans remain supported, common string booleans remain
accepted, and unset configuration still defaults to `false`. Empty,
whitespace-bearing, unsupported, or non-boolean configured values fail closed
during launch.

## Consequences

Operators see malformed DeepSeek strict-tools overrides at startup instead of
silently running with strict tool handling disabled. Deployments that want the
default non-strict mode should leave the override unset or set an explicit
supported false value.
