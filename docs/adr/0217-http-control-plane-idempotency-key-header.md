# 0217: HTTP Control-Plane Idempotency Key Header

## Status

Accepted

## Context

Mutating control-plane operations require idempotency so retries do not apply
admin mutations twice. The application boundary can enforce idempotency once it
has an `IdempotencyKey`, but HTTP adapters also need a shared way to read and
validate the key from request metadata.

If each HTTP composition root parses the header independently, invalid keys can
produce inconsistent errors or raw idempotency values can leak in responses.

## Decision

Use the `Idempotency-Key` HTTP header for mutating control-plane requests.

Add `idempotency_key_from_headers` to `prodex-gateway-http`. The helper treats
the header as optional, validates present values with the domain
`IdempotencyKey` parser, and maps parse failures to a stable redacted HTTP
error plan.

Add `plan_application_control_plane_idempotency_from_http` to
`prodex-application`. It reads the key from `GatewayHttpRequestMeta` and then
reuses `plan_application_control_plane_idempotency`, so mutating operations
still reject missing keys and read-only operations remain key-optional.

## Consequences

- HTTP adapters get one shared header contract for control-plane idempotency.
- Invalid idempotency key values use the same redacted error envelope as domain
  key validation.
- Mutating admin request idempotency remains enforced at the application
  boundary.
- The helper is side-effect-free and does not depend on an HTTP framework.
