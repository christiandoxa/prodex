# ADR 0305: Enterprise Guard Alias Manifest Validation

## Status

Accepted

## Context

ADR 0304 pinned enterprise guard package aliases in
`scripts/ci/test-impact-manifest.json`, but that manifest could still drift from
the enterprise guard list used by the workflow/package-script checks. If an
alias were removed, `package:changed-aliases` would stop checking that command.

## Decision

`scripts/ci/enterprise-docs-guard.mjs` now verifies that
`scripts/ci/test-impact-manifest.json` contains package aliases for every
enterprise guard script required by the CI workflow. Its self-test rejects a
manifest missing an enterprise guard alias.

## Consequences

- The workflow command list, `package.json` script list, and changed-test alias
  manifest are checked against the same enterprise guard set.
- Removing an enterprise alias now fails the docs guard instead of silently
  weakening package script drift detection.
- The change is static CI metadata validation only.
