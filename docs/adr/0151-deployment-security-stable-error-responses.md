# ADR 0151: Deployment security stable error responses

## Status

Accepted

## Context

The domain deployment validator reports concrete container and Kubernetes
manifest violations such as root execution, writable filesystems, mutable image
references, missing ExternalSecret references, missing NetworkPolicy, or missing
migration jobs. Those details are useful in CI logs and trusted operator
diagnostics, but client-visible readiness, startup, or control-plane validation
APIs should expose a stable machine-readable failure without leaking internal
manifest topology or policy details.

## Decision

Add `plan_deployment_security_error_response` to `prodex-domain`. It maps any
non-compliant `DeploymentSecurityReport` into one stable response plan:

- status: `InvalidDeployment`;
- code: `deployment_security_validation_failed`; and
- message: `deployment security validation failed`.

The detailed `DeploymentViolation` list remains available to CI and trusted
operator tooling. Public API boundaries should use the stable response plan when
reporting deployment security validation failures.

## Consequences

Deployment security validation can be surfaced through readiness/startup or
control-plane APIs without coupling clients to individual Kubernetes object names
or container-hardening checks. CI still retains precise violation evidence for
review and remediation.
