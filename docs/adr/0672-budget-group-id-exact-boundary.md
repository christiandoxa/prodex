# ADR 0672: Budget Group IDs Match Exactly

## Status

Accepted.

## Context

Legacy gateway virtual keys can share a `budget_id` so local compatibility
admission can aggregate request and spend limits. Server-side admin boundaries
now treat budget IDs as exact governance identifiers, but the local budget-group
aggregation path still trimmed IDs before comparing them.

Trimming can merge a padded budget ID such as ` shared ` into the canonical
`shared` group and deny or charge against the wrong compatibility group.

## Decision

Budget-group aggregation now compares `budget_id` values exactly. Empty budget
IDs still disable group aggregation for compatibility, but whitespace is no
longer normalized into another budget ID.

## Consequences

Padded budget IDs do not aggregate with canonical IDs. Canonical existing budget
groups are unchanged, and durable multi-replica enforcement still belongs behind
the reservation backend.
