# ADR 0303: Enterprise Docs Guard Impact Selection

## Status

Accepted

## Context

ADR 0301 and ADR 0302 made `scripts/ci/enterprise-docs-guard.mjs` validate the
enterprise evidence documents, GitHub Actions guard coverage, and matching
`package.json` scripts. The changed-test planner still needed an explicit
selection rule so local impact-based checks run that guard when workflow,
package, guard, or enterprise evidence files change.

## Decision

`scripts/ci/changed-tests.mjs` now schedules `enterprise-docs-guard` for
enterprise evidence docs, the CI workflow, `package.json`, the guard itself,
changed-test metadata, and ADR 0300+ files. The CI impact regression tests
assert that workflow, package, and enterprise docs guard changes select the
guard.

## Consequences

- Local impact-based checks catch workflow/package drift before push.
- The enterprise docs, workflow, and npm script contracts have one focused
  changed-test selection path.
- The change affects Node CI tooling only and does not add request-path runtime
  work.
