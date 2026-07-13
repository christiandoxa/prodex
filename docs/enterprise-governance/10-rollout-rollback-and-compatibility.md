# Rollout, Rollback, and Compatibility

Governance ships as independently observable capabilities behind typed,
versioned modes. Existing behavior remains the compatibility reference until
shadow parity and phase exit gates pass. Detailed sequencing is in
[`13-rollout-rollback-and-deprecation.md`](13-rollout-rollback-and-deprecation.md).

## Modes

| Mode | Authority |
| --- | --- |
| `personal` | Current local-compatible path |
| `enterprise_observe` | New identity/governance inputs are real, decisions are shadow-only |
| `enterprise_enforce` | Governed decisions and supported obligations are authoritative |
| `bank_enforce` | Strict startup/runtime fail-closed profile |

Capabilities also use `off`, `observe`, and `enforce` states. Configuration
validation rejects combinations that could label a control enforced when its
PEP or dependency is absent.

## Promotion sequence

1. Build immutable snapshots and content-free telemetry.
2. Run shadow decisions on synthetic and opted-in tenants.
3. Compare stable effect/reason/eligible-set parity; investigate every unsafe
   divergence.
4. Canary enforcement for low-risk tenants and one capability at a time.
5. Expand by tenant/cohort only after security, reliability and performance
   budgets pass for the soak window.
6. Practice application, policy, registry, secret and database rollback.
7. Enable bank mode only after its full fail-closed scenario passes.
8. Remove a legacy authority only after usage is zero and the supported
   rollback window no longer depends on it.

## Compatibility rules

- Preserve upstream status/body/stream semantics unless the local proxy fails
  before an upstream response exists.
- Preserve continuation affinity and never rotate after stream commit.
- Add database fields/tables before readers depend on them; retain dual-read
  compatibility through the oldest supported rollback image.
- Version APIs, event schemas, snapshots and configuration. Readers reject
  unknown mandatory semantics instead of guessing.
- Dual-write is allowed only with one declared authority and divergence
  telemetry; two independent authorities are forbidden.

## Rollback

Rollback selects a previous compatible binary plus immutable approved policy,
registry and configuration snapshots. Invalidated or revoked revisions remain
invalid. Ephemeral caches are discarded and rebuilt from durable authority.
Database rollback follows expand/migrate/contract or a tested forward fix; a
destructive schema reversal is not the default incident response.

Immediate rollback triggers include policy bypass, cross-tenant access,
unredacted sensitive output, mandatory audit loss, routing outside the approved
eligible set, post-commit retry/rotation, bank fail-open behavior, or a sustained
SLO/budget breach.

## Exit evidence

- mode/config validation and shadow-divergence evidence;
- canary and soak dashboards with bounded labels;
- five-sample before/after performance evidence;
- prior-image and prior-snapshot rollback drills;
- database forward/back compatibility and export/import tests; and
- a removal checklist proving no supported client or rollback target depends on
  the retired path.
