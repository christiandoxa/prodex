# ADR 0298: Runtime Policy Docs Guard Self-Test

## Status

Accepted

## Context

Runtime policy documentation is part of the enterprise operations contract. The
Prometheus virtual-key metrics section must preserve the ADR 0009
label-cardinality boundary by documenting `key_hash` plus bounded scope
booleans and by rejecting raw tenant, team, project, user, budget, or key names
as metric labels.

The docs check enforced the current document but did not prove its own negative
cases independently. A future edit could weaken the guard while keeping the
current document green.

## Decision

`scripts/docs/runtime-policy.mjs` now has `--self-test`. The self-test accepts a
minimal valid label contract, rejects the old raw-governance-label wording, and
rejects missing required `key_hash` or scope-boolean phrases.

`npm run docs:lint` runs the self-test before the runtime policy document check,
and the changed-test planner schedules the self-test whenever runtime policy
docs or policy metadata change.

## Consequences

- The runtime policy docs guard can be tested without mutating repository docs.
- Prometheus label-cardinality documentation has an explicit negative test path.
- Standard docs validation and impact-based checks now exercise the guard's
  negative cases instead of leaving them as an optional command.
- The guard remains docs-only and does not add runtime dependencies or request
  path work.
