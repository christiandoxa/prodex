# Enterprise Governance Document Map

Verified: Prodex 0.346.0, 2026-07-25.

The former duplicate numeric prefixes described distinct contracts rather than
duplicate copies. They are now uniquely numbered without changing their
requirements:

| Former document | Canonical document | Scope decision |
| --- | --- | --- |
| `04-classification-and-obligations.md` | `15-classification-contract-and-enforcement.md` | Detailed classifier/enforcement contract; separate from policy evaluation |
| `05-response-stream-enforcement.md` | `16-response-stream-enforcement.md` | Commit-aware response enforcement; separate from approval workflow |
| `07-policy-approval-and-store.md` | `17-policy-authority-and-revision-store.md` | Durable policy authority and revision lifecycle; complements concise policy and approval contracts |
| `08-audit-siem-and-evidence.md` | `18-audit-siem-and-evidence.md` | Detailed evidence and outbox contract; separate from the combined audit/secret overview |
| `10-unified-gateway-and-identity.md` | `19-unified-gateway-and-identity.md` | Gateway identity boundary; separate from rollout compatibility |
| `11-operations-slos-and-alerts.md` | `20-operations-slos-and-alerts.md` | Operational signals and SLOs; separate from the security test matrix |
| `12-testing-performance-and-evidence.md` | `21-testing-performance-and-evidence.md` | Evidence bundle and test program; complements the concise benchmark register |
| `13-rollout-rollback-and-deprecation.md` | `22-rollout-rollback-and-deprecation.md` | Detailed rollout lifecycle; separate from operator incident runbooks |

No content was discarded. The shorter numbered documents remain required
release artifacts; these detailed contracts retain their independent scope.
