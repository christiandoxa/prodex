# Operator Runbooks

These runbooks describe target governed operation. Commands and dashboards must
be replaced with deployed, tested equivalents before production use. Existing
runtime diagnosis (`prodex doctor --runtime`), logs, policy LKG and recovery
primitives remain useful; governance-specific automation is still incomplete.

## Common first response

1. Preserve the latest runtime log and content-free metrics/audit references.
2. Identify tenant cohort, mode, active policy/registry/config revisions,
   deployment image and first/last affected request IDs.
3. Stop rollout expansion; do not disable tenant isolation, audit, redaction or
   hard provider filters to restore service.
4. Determine whether failure is precommit or postcommit. Never retry/rotate a
   committed stream.
5. Prefer a previous compatible image or immutable approved/LKG snapshot.
6. Open an incident record and retain commands, timestamps, approvals and
   verification output without raw content or secrets.

## Invalid policy or registry snapshot

- Alert on checksum/schema/compile/refresh failure and freeze promotion.
- Keep a verified non-revoked LKG only where the current mode permits it.
- In bank mode, deny when no valid mandatory snapshot exists.
- Validate a corrected immutable revision; use maker-checker activation.
- Confirm gateway acknowledgement and decision/reason distributions before
  closing. Never reactivate an invalidated revision.

## Audit integrity or storage failure

- Quarantine an affected tenant partition on chain fork/tamper and preserve DB,
  backup and SIEM evidence.
- Fail mandatory mutations/dispatches closed when durable evidence cannot be
  committed; apply bounded admission before disk exhaustion.
- Run the deterministic verifier from a trusted checkpoint and compare the
  independent export.
- Repair through an approved, append-only procedure; never rewrite history to
  hide a gap. Rotate affected credentials if compromise is plausible.

## SIEM backlog

- Check oldest age, depth, claim/ack rate, lease expiry, retries and dead-letter
  count.
- Restore connectivity/credentials, then scale bounded workers if the sink can
  accept traffic.
- Replay idempotently by event ID. Do not delete or skip unexpired events.
- If hard safety capacity approaches, throttle/fail mandatory operations per
  mode and escalate.

## Secret manager outage or suspected leak

- Stop new operations whose required secret has no valid safe lease.
- Revoke/rotate the affected secret reference and provider credential; clear
  caches through the supported invalidation path.
- Search sanitized logs/audit/build artifacts with canaries, not by printing the
  suspected value.
- In bank mode, do not introduce raw environment/file secrets as an emergency
  bypass. Use narrowly approved break-glass only within its non-bypass limits.

## Provider outage or overload

- Inspect endpoint-specific health, transport/quota classification, in-flight
  saturation and eligible-set routing evidence.
- Before commit, use only policy-compliant fallback candidates. Preserve hard
  continuation affinity unless the owning target is explicitly revoked.
- After commit, expose the natural transport failure to the client; do not
  synthesize quota or rotate mid-stream.
- Roll back a bad registry/score snapshot only to a still-approved compatible
  revision.

## Identity/session compromise

- Disable the principal/credential, increment its credential or policy epoch,
  revoke active sessions and invalidate caches across replicas.
- Preserve tenant-scoped audit evidence and assess cross-tenant attempts.
- Rotate signing/session keys through the approved dual-read window if needed.
- Verify old sessions and continuation credentials fail before restoration.

## Database failover or restore

- Freeze control-plane mutations during uncertain leadership.
- Promote the documented PostgreSQL target, validate schema/migration revision,
  RLS, audit chain heads, active/LKG pointers and outbox claims.
- Rebuild Redis/cache state from durable authority.
- Meet the declared RPO/RTO, run tenant isolation and smoke tests, then resume
  staged traffic. A successful database start is not a successful restore drill.

## Emergency rollback

- Trigger for cross-tenant access, raw sensitive output, mandatory audit loss,
  provider eligibility bypass, postcommit rotation or bank fail-open behavior.
- Stop rollout, select the previous compatible image and immutable approved
  snapshots, preserve invalidation/revocation state, and discard ephemeral
  caches.
- Validate identity, inspection, policy, routing, audit and streaming invariants
  on synthetic traffic before resuming the smallest cohort.
- Record root cause, recovery timing, data/evidence impact and follow-up owner.
