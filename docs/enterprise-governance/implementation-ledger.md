# Enterprise Governance Implementation Ledger

This ledger describes controls implemented by the current Prodex source. It is
not a claim that a particular customer environment has met its SLO, legal,
pager, PKI, IdP, SIEM, disaster-recovery, or network-connectivity acceptance
criteria. Those are deployment acceptance responsibilities.

Status values are:

- `implemented`: the production path contains the control;
- `verified`: focused automated evidence covers its positive and negative path.

## Data Plane

| Control | Status | Source evidence |
| --- | --- | --- |
| Bounded schema-aware request inspection | verified | Application inspection pipeline, Unicode/nested-value/match-flood regressions, and Presidio adapter bounds |
| Monotonic four-level classification | verified | Domain classification properties and session monotonicity tests |
| Deterministic PDP and obligation merge | verified | Side-effect-free application/domain evaluators and explicit-deny/conflict tests |
| Immutable request governance context | verified | Application boundary guard and redacted governance types |
| Hard-eligible deterministic provider routing | verified | Governed routing planner, permutation properties, and eligible-set fallback tests |
| Continuation affinity and pre-commit-only rotation | verified | Response/turn/session binding regressions and post-commit no-retry tests |
| Reservation-based usage accounting | verified | Atomic PostgreSQL/Redis accounting and reconciliation tests |
| Response inspection before commitment | verified | Bounded full inspection for enforcing text/SSE and randomized incremental chunk tests |
| WebSocket enforcement boundary | verified | Off/observe transparency plus explicit HTTP `426` fallback when full inspection cannot be guaranteed |
| Bounded admission and backpressure | verified | Global, lane, profile, queue, and request-size limit tests |

## Policy Authority and Control Plane

| Control | Status | Source evidence |
| --- | --- | --- |
| Tenant-scoped immutable revisions | verified | SQLite and PostgreSQL governance lifecycle tests |
| Maker-checker approval and CAS activation | verified | Approval quorum/replay/self-approval and concurrent activation tests |
| Active/LKG hydration and refresh | verified | Atomic snapshot refresh and invalid-candidate preservation tests |
| Artifact authenticity | verified | Ed25519 signatures bind tenant, kind, revision, and SHA-256 artifact checksum; create/activate/startup/refresh/LKG verification tests cover tampering and unknown keys |
| Signing-key rotation | verified | Multiple bounded verifier keys are accepted; enforcing modes require a matching retained key |
| Execution approval | verified | Content-free digest binding, scope, expiry, quorum, and one-use tests |
| Break-glass isolation | verified | Separate credential scope, reason, expiry, and mandatory-audit tests |
| Versioned OpenAPI administration | verified | Strict schemas, canonical error envelopes, pagination/idempotency, and lifecycle HTTP tests |

## Identity, Storage, and Audit

| Control | Status | Source evidence |
| --- | --- | --- |
| OIDC/workload JWT/mTLS authentication | verified | Signature, issuer, audience, scope, assurance, certificate, and negative-path tests |
| Tenant authorization and RLS | verified | Cross-tenant denial, PostgreSQL RLS, and context-reset tests |
| Bound sessions and revocation | verified | Fixation/replay/expiry/concurrency tests and cross-replica revocation epoch invalidation |
| Mandatory tamper-evident audit | verified | Bounded durable commit-ack queue, hash-chain verification, and fail-closed request tests |
| Durable SIEM outbox | verified | Idempotent delivery, retry, lease, dead-letter, and recovery tests |
| External secret references | verified | Purpose-bound `SecretRef`, projected rotation, stale-version, and bank-mode configuration tests |
| Versioned storage migrations | verified | Repeatable SQLite/PostgreSQL migrations and compatibility tests |
| Backup and isolated restore | verified | Synthetic backup/restore drill with tenant, RLS, governance, audit, and outbox integrity checks |

## Operations and Supply Chain

| Control | Status | Source evidence |
| --- | --- | --- |
| Live low-cardinality telemetry | verified | Authn/authz/policy/secret/API/provider/inspection/audit/persistence counters, bounded gauges, and classic histograms |
| Operational alerting artifacts | verified | Checked-in Prometheus rules and Grafana panels for latency, failures, SIEM, audit, and queue saturation |
| Runtime diagnostics without TUI output | verified | Runtime log markers, doctor summaries, and stdout/stderr isolation tests |
| Hardened deployment artifacts | verified | Non-root/read-only manifests, least privilege, probes, disruption budget, and deny-default/allow-listed network policy guard |
| Supply-chain release gates | verified | Locked dependencies, audit/deny checks, SBOM/provenance, signed checksum, and vulnerability scanning workflow |
| Reproducible quality gates | implemented | Formatting, Clippy, full tests, docs/architecture guards, load/stress, storage proof, backup/restore, benchmarks, and release workflow are versioned commands |

## Canonical Evidence

- [`test-matrix.json`](test-matrix.json) maps threats to focused automated
  evidence.
- [`21-testing-performance-and-evidence.md`](21-testing-performance-and-evidence.md)
  defines evidence quality and benchmark rules.
- [`20-operations-slos-and-alerts.md`](20-operations-slos-and-alerts.md) separates
  checked-in telemetry from environment-specific SLO acceptance.
- [`22-rollout-rollback-and-deprecation.md`](22-rollout-rollback-and-deprecation.md)
  defines safe rollout and rollback.
- [`../security-test-matrix.md`](../security-test-matrix.md) is the concise
  source-level security matrix.

The source release is complete only when its mandatory release gates pass. A
deployment is accepted only after its owners separately verify external
identity, PKI, pager, SIEM, network-policy, failover, capacity, and recovery
objectives in the target environment.
