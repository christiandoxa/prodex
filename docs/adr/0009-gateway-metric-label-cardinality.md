# ADR 0009: Gateway metric label cardinality

## Status

Accepted.

## Context

Gateway virtual-key metrics are exported in Prometheus text format. Raw virtual
key names, tenant IDs, user IDs, prompts, and request IDs are security-sensitive
or high-cardinality values and must not be used as metric labels in regulated
multi-tenant deployments.

## Decision

Gateway metrics must not expose raw tenant/user/key identifiers as labels.
Virtual-key series keep a stable non-secret `key_hash` label for compatibility
with per-key aggregate counters, while tenant/team/project/user/budget metadata
is reduced to boolean scope-presence labels. Raw identifiers remain available
only through authenticated admin JSON/CSV endpoints and audit/trace/log surfaces
that apply their own authorization and redaction policy.

## Consequences

- Prometheus scraping no longer leaks raw tenant IDs, user IDs, or virtual-key
  names.
- Existing dashboards can still distinguish virtual-key series by `key_hash`, but
  operators must join back to raw key names via authorized admin tooling when
  needed.
- Future OTel metrics should preserve this boundary and avoid high-cardinality
  request/call identifiers as metric labels.
