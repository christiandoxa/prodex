# ADR 0677: Gateway Admin Token Roles Fail Closed

## Status

Accepted.

## Context

Configured `[[gateway.admin_tokens]]` entries are control-plane credentials.
The runtime policy validator rejects unknown roles, but direct config resolution
still defaulted a missing or unparseable role to Admin. Tests and future
callers that bypass validation could therefore turn a malformed role into write
access.

## Decision

Gateway admin token config resolution now defaults missing roles to Viewer and
rejects unknown roles. ADR 1002 removes the legacy `--auth-token`/
`PRODEX_GATEWAY_TOKEN` `default-admin` compatibility path so data-plane bearer
tokens do not become control-plane credentials.

## Consequences

Configured admin tokens need an explicit `role = "admin"` to receive write
access. Missing roles can still read admin resources, while malformed roles fail
closed during gateway launch configuration. Legacy gateway/root tokens only
protect data-plane bridge routes.
