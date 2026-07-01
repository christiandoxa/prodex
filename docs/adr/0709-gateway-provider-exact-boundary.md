# ADR 0709: Gateway Provider Exact Boundary

## Status

Accepted.

## Context

`gateway.provider` selects the upstream provider adapter used by gateway launch.
Policy validation previously only rejected blank-after-trim values, and runtime
provider parsing trimmed before matching provider aliases. A padded provider
selector could therefore be silently normalized into an active provider.

## Decision

Treat `gateway.provider` as an exact configuration boundary. Policy validation
rejects empty or whitespace-bearing provider selectors, and runtime provider
alias parsing refuses invalid selectors instead of trimming them.

## Consequences

Operators must fix padded provider selectors in `policy.toml`. Gateway provider
routing remains auditable and no longer depends on hidden launch-time cleanup.
