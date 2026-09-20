# Documentation Index

This index lists the maintained documentation for the current Prodex source.
Historical implementation snapshots and completed refactor plans are deliberately
not retained here; Git history is the record for obsolete material.

## Product and Runtime

| Document | Scope |
| --- | --- |
| [README](../README.md) | Product overview and installation |
| [Quickstart](../QUICKSTART.md) | First successful launch |
| [Local models](../LOCAL.md) | Self-hosted OpenAI-compatible model setup |
| [Architecture](architecture.md) | Workspace, dependency, and runtime boundaries |
| [Runtime policy](runtime-policy.md) | Runtime policy keys and enforcement behavior |
| [State model](state-model.md) | Affinity and persistence |
| [Optional tools](optional-tools.md) | Discovery, validation, and activation |
| [Smart Context](smart-context.md) | Safety, migration, and runtime verification |
| [Provider conformance](provider-conformance.md) | Adapter contract |
| [Harness modes](harness-modes.md) | Model-facing request policy |
| [Super sub-agents](sub-agents.md) | Staged CLI contract, session boundaries, and local-process design |

## Security, Operations, and Release

| Document | Scope |
| --- | --- |
| [Threat model](threat-model.md) | Trust boundaries, controls, and negative tests |
| [Security test matrix](security-test-matrix.md) | Canonical control-to-test evidence |
| [Testing](testing.md) | Test commands and evidence levels |
| [Supply chain](supply-chain.md) | Pins, provenance, and release gates |
| [Deployment](deployment.md) | Supported deployment patterns |
| [Backup and restore](backup-restore.md) | Recovery contract |

## Enterprise Governance

The maintained enterprise contract is split by responsibility:

- [Provider registry and routing](enterprise-governance/06-provider-registry-and-routing.md)
- [Storage, HA, backup, and DR](enterprise-governance/09-storage-ha-backup-and-dr.md)
- [Classification and enforcement](enterprise-governance/15-classification-contract-and-enforcement.md)
- [Response-stream enforcement](enterprise-governance/16-response-stream-enforcement.md)
- [Policy authority and revision store](enterprise-governance/17-policy-authority-and-revision-store.md)
- [Audit, SIEM, and evidence](enterprise-governance/18-audit-siem-and-evidence.md)
- [Unified gateway and identity](enterprise-governance/19-unified-gateway-and-identity.md)
- [Operations, SLOs, and alerts](enterprise-governance/20-operations-slos-and-alerts.md)
- [Testing, performance, and evidence](enterprise-governance/21-testing-performance-and-evidence.md)
- [Rollout, rollback, and deprecation](enterprise-governance/22-rollout-rollback-and-deprecation.md)
- [Implementation ledger](enterprise-governance/implementation-ledger.md)
- [Machine-readable test matrix](enterprise-governance/test-matrix.json)
- [Architecture decision records](enterprise-governance/adrs/)
- [Synthetic policy samples](enterprise-governance/samples/)

## Generated Evidence

Generated documents are scoped evidence, not substitutes for runtime verification:

- [Provider capabilities](provider-capabilities.md) via
  `node scripts/catalog/provider-capability-matrix.mjs --write`.

Broken local links and duplicate canonical numeric prefixes are CI failures. Smart Context
correctness is verified by its production guard and focused Mojo/runtime tests rather than the
retired offline replay-report CLI. The checked-in OpenAPI document describes the gateway's
documented routes, not complete upstream route or semantic coverage. Enterprise audit,
break-glass evidence, immutability, backup/DR, and OpenAPI documents describe their own deployment
or gateway boundaries; they are not automatically inherited by local audit logs, Super, or
sub-agent processes.
