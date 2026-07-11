# ADR 1020: Guardrail model scopes fail closed

## Status

Accepted.

## Context

Gateway guardrails can define a global `allowed_models` list. Invalid model
strings were silently filtered out during launch configuration. Because an
empty guardrail `allowed_models` list means no global model allowlist, malformed
configured entries could disable the intended model restriction.

## Decision

Guardrail `allowed_models` entries now fail closed. If a model scope is present,
it must be a non-empty string without whitespace. Invalid values reject gateway
launch configuration instead of being dropped.

## Consequences

Malformed global model scopes can no longer widen model authorization by
removing the guardrail allowlist. Deployments must remove absent model
restrictions or provide exact model names. Existing valid model scopes continue
to work unchanged. Regression coverage lives in
`crates/prodex-app/tests/src/app_commands/runtime_launch.rs`.
