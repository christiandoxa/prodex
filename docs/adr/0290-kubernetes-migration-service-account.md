# ADR 0290: Kubernetes Migration Service Account

## Status

Accepted.

## Context

The Kubernetes migration Job is intentionally separate from request-serving
gateway pods. It uses a migration-only secret and egress-only NetworkPolicy, but
it still shared the gateway ServiceAccount. Even with service-account token
automount disabled, sharing the workload identity makes later RBAC changes more
dangerous because migration permissions could accidentally follow gateway pods
or gateway permissions could follow the migration Job.

## Decision

Add a dedicated `prodex-gateway-migration` ServiceAccount and bind the
`prodex-gateway-migration` Job to it.

The deployment security guard now rejects migration Jobs that omit the
dedicated ServiceAccount or fall back to `prodex-gateway`.

## Consequences

- Gateway, migration, and control-plane workloads each have distinct Kubernetes
  identities.
- Future RBAC for external migrations can be scoped without affecting
  request-serving gateway pods.
- The manifest continues to disable service-account token automounting for all
  workloads.
