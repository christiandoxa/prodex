# ADR 0982: Gateway compatibility schema contractions require explicit choreography

## Status

Accepted

## Context

The gateway compatibility schema now uses explicit versioned migrations and the
external `prodex-gateway migrate` entrypoint. That closes request-path DDL, but
it does not by itself define how legacy compatibility tables or columns can be
removed safely across release boundaries.

Without an explicit contract plan, a future cleanup could drop compatibility
data that older gateways, rollback targets, or staged cutovers still read.

## Decision

Treat every destructive compatibility-schema change as an explicit
expand/backfill/contract rollout:

- **Expand:** add the new canonical table, column, index, or typed identifier
  while preserving the old compatibility read path.
- **Backfill:** migrate legacy rows into the new shape and fail the rollout if
  any row cannot be converted deterministically.
- **Dual-read / dual-write window:** keep one release boundary where new
  adapters can serve from the canonical shape while rollback-compatible
  deployments can still read the legacy compatibility shape.
- **Contract:** remove the old table or column only after staging and
  production evidence prove no supported rollback target still requires it.

Every future destructive compatibility migration must document:

1. the legacy readers/writers being retired;
2. the deterministic backfill rule;
3. the release boundary where dual-read or dual-write remains active;
4. the evidence required before the contract migration is allowed.

## Consequences

- Compatibility cleanup now follows the same operational discipline as the main
  enterprise schema.
- Rollback safety remains an explicit release concern instead of an implicit
  assumption in one migration file.
- Future compatibility removals need one more ADR or migration note, but that
  cost is smaller than silent production data loss or failed rollback.
