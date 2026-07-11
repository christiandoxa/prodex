# ADR 0139: Application data-plane admission error boundary

## Status

Accepted

## Context

`prodex-application` composes HTTP policy checks with gateway data-plane
admission. ADR 0123 intentionally exposed stable local HTTP-policy errors while
leaving admission errors separate. Gateway admission now has its own stable
redacted response planner, so application composition roots should surface that
planner instead of forcing adapters to inspect raw authorization, reservation,
provider invocation, tenant-affinity, or telemetry errors.

## Decision

Extend `ApplicationDataPlaneErrorResponsePlan` with an `admission` response
field. `plan_application_data_plane_error_response` now returns:

- the stable gateway HTTP response for HTTP policy failures;
- the stable gateway admission response for admission failures; and
- a stable redacted wrong-route response as updated by ADR 0221.

Raw data-plane errors remain available for trusted redacted diagnostics.

## Consequences

Application composition roots can report pre-provider data-plane failures
consistently without leaking credential scopes, tenant IDs, reservation details,
provider secret references, or telemetry internals.
