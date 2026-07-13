# Policy Model and Obligations

This document defines the target policy contract for governed requests. It is
an implementation specification, not a statement that every control is live.
The current implementation has policy snapshot, last-known-good, RBAC, request
inspection, guardrail, and admission primitives. A single production
`PolicyInput -> PolicyDecision` path and the obligation model below remain
gaps. Detailed design is in
[`04-classification-and-obligations.md`](04-classification-and-obligations.md)
and [`07-policy-approval-and-store.md`](07-policy-approval-and-store.md).

## Immutable input

`PolicyInput` is assembled once from authenticated and validated sources:

- tenant, principal, roles, credential and session assurance;
- operation, channel, request identity, tool and continuation context;
- data classification, inspection coverage, detector revision and finding
  kinds, never raw content;
- requested capability, model, region, retention and residency;
- provider-registry snapshot, policy snapshot and configuration revisions;
- approval or break-glass references after independent validation; and
- bounded runtime health/capacity facts used only where policy permits them.

Unknown mandatory attributes are not silently defaulted. Observe mode records
the would-deny result. Enterprise and bank enforcement deny when the missing
attribute can weaken a mandatory control.

## Decision

The target `PolicyDecision` contains a stable decision ID, input fingerprint,
policy revision, effect, reason codes, obligations, and expiry. Effects are:

1. `deny`;
2. `require_approval`;
3. `allow_with_obligations`; and
4. `allow`.

The most restrictive effect wins. A deny cannot be cancelled by a later allow.
Conflicting obligations fail closed instead of choosing an arbitrary value.

## Obligations

Obligations are typed and bounded. The initial required set is:

| Obligation | Enforcement point |
| --- | --- |
| Provider allow-set and trust tier | Governed router before any upstream commitment |
| Region/residency and local-only | Registry hard filter |
| Retention ceiling and no-training | Registry hard filter and provider request mapping |
| Tool allow/deny-set | Request enforcement before dispatch |
| Request masking/inspection | Inspection and request PEP |
| Response inspection/redaction | Streaming response PEP before local commit |
| Approval requirement | Approval PEP before reservation and dispatch |
| Mandatory durable audit | Mutation/dispatch transaction boundary |
| Cost/token/concurrency limits | Existing reservation and admission boundaries |
| Continuation pinning and expiry | Session/affinity PEP |

An obligation is effective only when a named enforcement point acknowledges it.
Unsupported mandatory obligations produce deny; they are never logged as
enforced.

## Evaluation and caching

The PDP is pure, local, deterministic and side-effect free. It evaluates an
immutable, checksum-verified snapshot. Compilation, database reads, Vault,
SIEM, DNS, provider probes and external PDP calls are forbidden on the request
hot path. Cache keys include every decision-relevant revision and attribute;
revocation or snapshot invalidation takes precedence over cache or LKG.

## Mode behavior

| Mode | Policy authority |
| --- | --- |
| `personal` | Existing compatible behavior; governance may be off |
| `enterprise_observe` | Compute and audit shadow decisions without changing the legacy routing result |
| `enterprise_enforce` | Enforce identity, policy and supported mandatory obligations |
| `bank_enforce` | Fail closed for missing/invalid identity, policy, inspection, registry, audit or secret prerequisites |

## Acceptance evidence

- golden, property and fuzz tests for stable effects and reason codes;
- conflict and missing-attribute tests for every obligation;
- proof that every supported channel reaches the same PDP/PEP boundary;
- snapshot activation, invalidation, LKG and rollback tests; and
- p99 PDP latency at or below 1 ms on the declared reference machine.
