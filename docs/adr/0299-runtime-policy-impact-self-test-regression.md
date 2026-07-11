# ADR 0299: Runtime Policy Impact Self-Test Regression

## Status

Accepted

## Context

ADR 0298 made the runtime policy documentation guard self-test part of standard
docs validation and the changed-test planner. That wiring protects the
Prometheus label-cardinality contract only if the planner keeps scheduling both
the self-test and the document check for runtime policy doc or metadata changes.

## Decision

The CI impact regression tests now assert that runtime policy documentation
changes schedule both `docs-runtime-policy-self-test` and
`docs-runtime-policy-check`.

## Consequences

- The changed-test planner cannot silently drop the negative Prometheus label
  self-test while still running the positive docs check.
- The regression stays in Node CI tooling and does not add Rust runtime or
  request-path work.
