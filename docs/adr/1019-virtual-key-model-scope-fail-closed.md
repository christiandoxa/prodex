# ADR 1019: Virtual key model scopes fail closed

## Status

Accepted.

## Context

Configured gateway virtual keys can restrict inference to `allowed_models`.
Invalid model strings were silently filtered out during launch configuration.
Because an empty `allowed_models` list means unrestricted model access for a
virtual key, malformed configured entries could widen data-plane authorization.

## Decision

Virtual key `allowed_models` entries now fail closed. If a model scope is
present, it must be a non-empty string without whitespace. Invalid values
reject gateway launch configuration instead of being dropped.

## Consequences

Malformed model scopes can no longer widen a virtual key to all models.
Deployments must remove absent model restrictions or provide exact model names.
Existing valid model scopes continue to work unchanged. Regression coverage
lives in `crates/prodex-app/tests/src/app_commands/runtime_launch.rs`.
