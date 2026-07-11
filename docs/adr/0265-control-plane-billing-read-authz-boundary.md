# ADR 0265: Control-Plane Billing Read Authorization Boundary

## Status

Accepted

## Context

Billing ledger reads are a control-plane use case, but they are intentionally
viewer-eligible. The generic control-plane admin boundary requires `Admin` and
is too broad for billing dashboards or exports, while data-plane credentials
must not become an inference/quota bypass into billing records.

Without an explicit authorization boundary, adapters could either over-grant by
checking only tenant access or over-require admin privileges and encourage
custom bypasses outside the shared authorization crate.

## Decision

Add `BoundaryKind::ControlPlaneBillingRead` to `prodex-authz`.

The boundary requires:

- `CredentialScope::ControlPlane`;
- `Role::Viewer` or higher;
- `ResourceKind::Billing`;
- `ResourceAction::Read`;
- tenant access through the existing `authorize_boundary_resource` path.

Data-plane and break-glass scopes remain rejected for this normal billing read
boundary. Break-glass usage must continue to use its separate audited boundary.

## Consequences

HTTP and control-plane adapters get a reusable billing-read authorization
contract that matches `ControlPlaneOperation::BillingRead` without falling back
to generic admin authorization. Client-visible authorization errors continue to
use stable redacted response plans.
