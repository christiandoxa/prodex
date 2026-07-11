# ADR 0285: Kubernetes Migration Job Network Policy

## Status

Accepted.

## Context

The Kubernetes baseline includes a safe-failing migration Job to make clear that
request-serving gateway pods must not run schema migrations. The Job uses the
gateway secret set because a future external migrator will need database
credentials. Without a dedicated NetworkPolicy label and selector, the Job could
inherit namespace defaults and have broader network access than a migrator needs.

## Decision

Label the migration Job pods as `app.kubernetes.io/name:
prodex-gateway-migration` and add a dedicated egress-only NetworkPolicy for
that label. The policy allows DNS and PostgreSQL in namespaces labeled
`prodex.dev/network-tier=data-store`; it does not allow ingress, Redis, or public
provider HTTPS.

The deployment security guard now requires the migration Job/network policy
surface and label marker, and its self-test rejects manifests without the
migration network policy label.

## Consequences

- The external migrator path is isolated from request-serving gateway ingress
  and provider egress.
- The future migrator can reach PostgreSQL without making gateway startup a
  migration mechanism.
- Operators that need different database endpoint policy can replace the
  selector deliberately while keeping migrator access separate from gateway
  traffic.
