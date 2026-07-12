# ADR 1074: Production Control-Plane Service Mode

## Status

Accepted. Supersedes the production placeholder in ADR 0098 and the
control-plane deployment portions of ADRs 0287, 0289, and 1061. ADR 1075
supersedes this ADR's original loopback transport staging.

## Context

The dedicated control-plane listener is now wired, but it previously reused a
gateway production policy that required data-plane bearer and provider
credentials. Kubernetes kept the workload at zero replicas and passed storage
secrets through `envFrom`, so scaling it up would either fail policy validation
or grant unnecessary inference capability.

## Decision

Add a typed top-level runtime-policy `service_mode`. It defaults to `gateway`
for compatibility; `control-plane` must be explicit and must match the
dedicated binary's requested mode before secret resolution or listener bind.

Control-plane policy requires production projected secrets, an explicit
projected admin-role token, and shared PostgreSQL or Redis state. It rejects
provider, data-plane authentication, routing, virtual-key, SSO, outbound
observability, request-constraint, and guardrail configuration. ADR 1075 later
moved the route-isolated control plane fully in-process and removed the empty
provider-shaped loopback placeholder from the dedicated production entrypoint.

Run one control-plane replica on port 4100 with a dedicated policy ConfigMap and
an ExternalSecret containing only the admin token plus PostgreSQL and Redis
references. Project those values as read-only files, retain restricted
DNS/datastore-only egress, drop all Linux capabilities, and use the bounded
pre-stop/graceful-drain window.

## Consequences

- Existing gateway policies remain gateway mode without edits.
- A gateway cannot accidentally start a control-plane policy, and a missing or
  gateway-mode policy cannot start the dedicated control plane.
- The control-plane pod has no provider credential or public provider egress.
- CI rejects reintroducing secret environment injection, zero replicas,
  placeholder policy reuse, or data-plane/provider capability.
