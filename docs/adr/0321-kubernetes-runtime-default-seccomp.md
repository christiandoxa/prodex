# ADR 0321: Kubernetes RuntimeDefault Seccomp Guard

## Status

Accepted.

## Context

The Kubernetes baseline already uses restricted Pod Security Admission and
workload-level hardening such as non-root users, read-only root filesystems,
dropped Linux capabilities, and disabled privilege escalation. Seccomp is part
of that same runtime isolation boundary, but it should be guarded directly so a
future manifest edit cannot silently weaken pods to an unconfined syscall
profile while leaving the broader PSA labels intact.

## Decision

Keep gateway, migration, and control-plane workload security contexts on
`seccompProfile.type: RuntimeDefault`.

The deployment security guard now treats both the explicit `seccompProfile`
field and `RuntimeDefault` value as required Kubernetes markers, and its
self-test rejects missing or weakened seccomp settings.

## Consequences

- The checked-in deployment baseline stays aligned with the restricted Pod
  Security runtime profile.
- Operators must make any workload-specific seccomp exception explicitly rather
  than inheriting an unconfined default by accident.
- The guard remains lightweight and string-based, so it proves the checked-in
  artifact intent without becoming a Kubernetes schema validator.
