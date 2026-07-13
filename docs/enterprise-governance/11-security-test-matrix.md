# Security Test Matrix

The authoritative machine-readable inventory is
[`test-matrix.json`](test-matrix.json). Every row names a threat, control,
mode, phase, test type, expected result and implementation status. `planned`
means required evidence is not yet present; it is not a passing result.

The matrix also uses `tested` only when it names the focused test or guard,
`implemented` when production behavior exists but a broader gate remains, and
`pending_validation` only when the required backend execution evidence is not
available.

## Candidate evidence register

| Control | Status | Named evidence |
| --- | --- | --- |
| Identity/header trust | tested | `edge_security_rejects_forwarding_spoof_and_host_origin_csrf_mismatch`; `principal_evidence_enforces_required_scope_and_anonymous_policy`; auth boundary guard |
| Bounded inspection/classification | tested | `inspection_result_is_bounded_deterministic_and_content_free`; schema walker/local detector bounds; application inspection tests |
| PDP and obligations | tested | `explicit_deny_wins_and_drops_obligations`; `policy_compilation_is_bounded_and_rejects_duplicate_ids`; governance obligation matrix |
| Provider routing | tested | governed-routing hard-filter, fixed-point score, affinity, revocation, fallback and debug suites |
| Channel boundary | tested | `provider_route_mapping_covers_forwarded_data_plane_routes`; production/application boundary guards |
| Upstream transparency/no postcommit retry | tested | generic 429/content-policy pass-through and precommit-only error-policy regressions |
| Bank startup configuration | tested | `bank_governance_deployment_matrix_fails_closed`; listener guard; deployment-security guard |
| PostgreSQL/RLS and SIEM | tested | live disposable PostgreSQL/TLS/RLS lifecycle and outbox proof passed |
| Performance | implemented | maximum-bound and disabled-path Criterion budgets, fuzz, load and stress passed; external multi-replica soak remains deployment evidence |

## Declared residuals

- One process has one attached executable provider adapter. Heterogeneous
  in-process selection/fallback is unsupported and must fail unavailable.
- Mandatory governed data-plane audit is synchronous precommit I/O.
- Policy-selected Presidio and guardrail-webhook calls are bounded request-path
  network dependencies.
- Cross-replica revocation has a documented five-second refresh bound; Redis
  restart, managed-database failover and external SIEM outage exercises remain
  deployment acceptance work.

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
