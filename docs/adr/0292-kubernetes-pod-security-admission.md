# ADR 0292: Kubernetes Pod Security Admission

## Status

Accepted.

## Context

The Kubernetes baseline hardens the known gateway, migration, and control-plane
workloads with non-root users, dropped capabilities, read-only root filesystems,
and disabled service-account token automounting. Those controls are workload
local. A future pod added to the `prodex` namespace without the same hardening
could bypass the baseline unless the namespace also enforces Pod Security
Admission.

## Decision

Label the `prodex` namespace with restricted Pod Security Admission for
`enforce`, `audit`, and `warn`, each pinned to `latest`.

The deployment security guard now rejects missing namespace PSA labels or
weakened values such as `baseline`.

## Consequences

- New pods in the namespace must satisfy the restricted Pod Security profile.
- Audit and warning labels surface rollout issues before enforcement failures
  reach production.
- Operators that need privileged sidecars must make an explicit namespace or
  workload exception outside the default Prodex namespace.
