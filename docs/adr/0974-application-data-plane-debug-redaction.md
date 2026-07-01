# ADR 0974: Application Data-Plane Debug Redaction

## Status

Accepted

## Context

Application data-plane planning carries HTTP request metadata, gateway
admission planning, and route or admission errors. Derived debug output would
expose tenant identifiers, route paths, and admission details in diagnostics.

## Decision

Use custom `Debug` implementations for `ApplicationDataPlaneRequest`,
`ApplicationDataPlanePlan`, and `ApplicationDataPlaneError`. Redact HTTP
request metadata, admission planning, and route/admission error details while
preserving planner and error variant names.

## Consequences

Application diagnostics can distinguish data-plane planner shapes without
leaking tenant identifiers, route paths, or admission details.
