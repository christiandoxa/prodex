# ADR 0098: Control Plane Kubernetes Placeholder

## Status

Accepted

## Context

The enterprise target requires separate data-plane and control-plane operational
surfaces. The repository now contains dedicated `prodex-gateway` and
`prodex-control-plane` binary entrypoints, but their async `serve` adapters are
not wired yet. The existing Kubernetes baseline only modeled the legacy gateway
serving path, which made the control-plane deployment surface invisible in
production artifacts.

## Decision

Copy the new `prodex-gateway` and `prodex-control-plane` binaries into the
container image. Extend `deploy/kubernetes/prodex-gateway.yaml` with a
`prodex-control-plane` `Deployment`, `Service`, and `NetworkPolicy` that use the
same hardened pod/container defaults as the gateway surface.

The control-plane deployment is intentionally annotated as a placeholder and set
to `replicas: 0`. Its command references `prodex-control-plane serve` so the
intended operational contract is explicit, but it cannot receive traffic until
the adapter is implemented and operators deliberately scale it up after
validation.

## Consequences

Production manifests now show both enterprise surfaces without changing runtime
traffic behavior prematurely. The deployment security guard verifies the binary
copies, hardened control-plane resources, explicit placeholder annotation, and
scaled-to-zero state so the placeholder cannot be mistaken for a live control
plane.
