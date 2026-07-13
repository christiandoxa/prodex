# ADR 0010: Bank Profile Fail-Closed Semantics

- Status: Accepted
- Scope: `bank_enforce`

## Context

Regulated deployments need a named, testable configuration contract. A label is
dangerous if individual controls can silently remain optional or fail open.

## Decision

Define `bank_enforce` as a typed deployment profile with startup and runtime
invariants. Startup rejects public exposure, anonymous compatibility, missing
approved identity/tenant binding, invalid mandatory policy/registry snapshots,
raw or unavailable required secrets, non-durable mandatory audit/outbox,
unresolved classification requirements, insecure storage isolation or
unrestricted provider egress. Runtime denies when mandatory identity,
inspection, policy, obligation, approval, secret, provider eligibility,
accounting or audit state is absent/invalid. No generic break-glass may disable
these invariants. Upstream errors and streaming commitment semantics remain
transport-transparent.

## Consequences

Bank mode trades availability for the declared controls. Every dependency has
an explicit fail-closed test, alert and runbook. The name provides technical
controls and evidence, not automatic legal or regulatory certification.

## Implementation status

The typed profile, startup validator, mandatory stage checks, private-listener
guard, identity/audit/secret-reference requirements and runtime fail-closed
inspection/obligation behavior are implemented. Evidence includes
`bank_governance_deployment_matrix_fails_closed`,
`bank_runtime_listener_guard_rejects_public_or_wildcard_addresses`,
`bank_snapshot_denies_unsupported_inspection`, and the deployment-security
guard. Bank mode must not be advertised as release-ready until PostgreSQL/SIEM,
full-channel, chaos and release gates pass.
