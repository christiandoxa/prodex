# ADR 0972: Application Provider Capability Debug Redaction

## Status

Accepted

## Context

Application provider-capability planning carries capability requests, route
candidates, and negotiation outcomes. Derived debug output would expose raw
provider route names and model identifiers in diagnostics.

## Decision

Use custom `Debug` implementations for
`ApplicationProviderCapabilityRequest`,
`ApplicationProviderCapabilityPlan`, and
`ApplicationProviderCapabilityError`. Redact capability requests, route
candidates, negotiation plans, and negotiation errors while preserving planner
and error variant names.

## Consequences

Application diagnostics can distinguish provider-capability planner shapes
without leaking raw provider route names or model identifiers.
