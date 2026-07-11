# ADR 0244: Kubernetes explicit service accounts

## Status

Accepted

## Context

The Kubernetes baseline already disables service-account token automounting on
pods. Without explicit service accounts, however, gateway, migration, and
control-plane pods still target the namespace default service account. That
creates avoidable ambiguity when operators add RBAC, admission policies, or
workload identity bindings.

Enterprise deployments should make workload identity intentional even when the
current pods do not need Kubernetes API credentials.

## Decision

The production-shaped Kubernetes manifest now defines dedicated
`prodex-gateway` and `prodex-control-plane` `ServiceAccount` objects with
`automountServiceAccountToken: false`.

Gateway pods and the safe-failing migration Job use `serviceAccountName:
prodex-gateway`. The placeholder control-plane deployment uses
`serviceAccountName: prodex-control-plane`. The deployment security guard now
requires these service accounts and workload bindings.

## Consequences

Operators can attach future RBAC or workload identity to the correct Prodex
surface without relying on namespace defaults. Pod token automounting remains
disabled unless a future adapter has a deliberate, reviewed reason to enable it.
