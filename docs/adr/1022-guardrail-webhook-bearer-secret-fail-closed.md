# ADR 1022: Guardrail webhook bearer secret refs fail closed

## Status

Accepted.

## Context

Gateway guardrail webhooks can use `webhook_bearer_token_env` as a secret
reference for outbound webhook authentication. Invalid, missing, or empty env
references were silently treated as no bearer token. A configured authenticated
webhook could therefore be called without authentication, or fail open when the
webhook rejected unauthenticated requests.

## Decision

Configured guardrail webhook bearer token env references now fail closed. The
env var name must be an exact non-empty string without whitespace, the env var
must exist, and its value must be non-empty after trimming. Invalid references
reject gateway launch configuration.

## Consequences

Webhook authentication is no longer silently disabled by malformed or missing
secret references. Deployments that want unauthenticated webhooks must omit
`webhook_bearer_token_env`; deployments that configure it must provide a valid
secret. Regression coverage lives in
`crates/prodex-app/tests/src/app_commands/runtime_launch.rs`.
