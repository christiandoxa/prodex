# Security Test Matrix

The authoritative machine-readable inventory is
[`test-matrix.json`](test-matrix.json). Every row names a threat, control,
mode, phase, test type, expected result and implementation status. `planned`
means required evidence is not yet present; it is not a passing result.

## Required suites

| Area | Minimum evidence |
| --- | --- |
| Tenant and identity | forged headers, confused deputy, cross-tenant/RLS, session fixation/revocation |
| Inspection | Unicode byte ranges, nested tool arguments, depth/size limits, unsupported modality, fail-open/closed modes |
| Policy | golden effects/reasons, missing attributes, obligation conflict, snapshot invalidation/LKG |
| Approval | self/stale/replayed approval, quorum races, expiry, use count, break-glass overreach |
| Routing | hard-filter before score, fixed-point determinism, pinning/revocation, eligible fallback, no postcommit retry |
| Audit/SIEM | atomic rollback, chain tamper/fork, outbox duplicate/lease/backlog, content canary |
| Secrets | raw-secret rejection, namespace authorization, cache expiry, rotation/revocation and Vault outage |
| Storage/HA | migrations, PostgreSQL RLS, restore, multi-replica activation, Redis loss and DB failover |
| Deployment | non-root/read-only, private bind, deny-by-default egress, image/provenance scanning |
| Performance | disabled overhead, stage p99 budgets, load/soak and bounded cardinality |

## Adversarial invariants

Tests must prove that no caller-controlled field becomes trusted identity; no
classification can be lowered by an untrusted input; no soft score can restore
an ineligible provider; no approval can authorize outside the active policy; no
continuation survives explicit revocation; no mandatory mutation succeeds
without audit/outbox; no secret enters logs/evidence; and no retry or rotation
occurs after response commitment.

## CI policy

Unit, golden and property suites run on every change. Architecture guards,
format, lint, full workspace tests, docs/JSON validation and secret/PII scans are
required merge gates. Integration, PostgreSQL/RLS, multi-replica, restore,
container/Kubernetes security and benchmark jobs run in their declared CI
environments. Fuzz, load, soak and chaos jobs retain seeds and raw artifacts and
run on scheduled/release gates according to cost.

Exceptions are time-bounded, owner-approved and auditable. They identify the
exact missing control/test, affected modes, compensating control and expiry;
they never convert a planned row to passed.
