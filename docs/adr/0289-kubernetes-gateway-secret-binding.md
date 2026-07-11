# ADR 0289: Kubernetes Gateway Secret Binding

## Status

Accepted.

## Context

The Kubernetes baseline separates gateway data-plane secrets from control-plane
storage and coordination secrets. Gateway pods need the data-plane bearer token
and provider credentials for inference traffic, while control-plane pods must
not inherit those credentials.

## Decision

Bind `Deployment/prodex-gateway` to `prodex-gateway-secrets` and reject gateway
workload mounts of `prodex-control-plane-secrets` in the deployment security
guard.

The control-plane placeholder remains bound to `prodex-control-plane-secrets`,
which contains only PostgreSQL and Redis references.

## Consequences

- Gateway pods receive the data-plane and provider credentials they require.
- Control-plane storage credentials are not accidentally used as the gateway
  runtime secret source.
- The guard self-test now proves that missing gateway secret bindings and
  gateway mounts of control-plane secrets fail closed.
