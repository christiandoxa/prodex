# ADR 0973: Application Quota Read Debug Redaction

## Status

Accepted

## Context

Application quota-read planning carries HTTP request metadata, gateway
authorization planning, and route or authorization errors. Derived debug
output would expose tenant identifiers, quota route paths, and authorization
details in diagnostics.

## Decision

Use custom `Debug` implementations for `ApplicationQuotaReadRequest`,
`ApplicationQuotaReadPlan`, and `ApplicationQuotaReadError`. Redact HTTP
request metadata, authorization planning, and route/authorization error
details while preserving planner and error variant names.

## Consequences

Application diagnostics can distinguish quota-read planner shapes without
leaking tenant identifiers, quota paths, or authorization details.
