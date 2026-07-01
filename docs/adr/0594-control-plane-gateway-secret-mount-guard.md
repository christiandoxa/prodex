# ADR 0594: Control Plane Gateway Secret Mount Guard

## Status

Accepted.

## Context

The production Kubernetes manifest keeps gateway data-plane credentials and
control-plane storage credentials in separate `ExternalSecret` targets. The
deployment security guard already rejects gateway pods that mount
control-plane secrets, but it did not reject the inverse case where a
control-plane pod mounts `prodex-gateway-secrets`.

## Decision

`scripts/ci/deployment-security-guard.mjs` now rejects
`prodex-control-plane` deployments that mount `prodex-gateway-secrets`.

## Consequences

The checked-in Kubernetes artifact must keep gateway/provider tokens out of
the control-plane workload. Shared storage credentials remain in the dedicated
control-plane secret.
