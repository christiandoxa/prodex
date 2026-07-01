# ADR 0035: Verify gateway virtual-key TPM-limit denials as authorization audit events

## Status

Accepted

## Context

Gateway virtual keys may enforce a per-minute token limit before upstream provider
transport. When estimated request input tokens would exceed the per-minute token budget,
the gateway returns `429 tpm_limit_exceeded`. This protects tenant fairness and local
capacity separately from request-count rate limits.

TPM denials are credentialed data-plane authorization decisions. Audit records must not
persist the virtual-key bearer token.

## Decision

TPM-limit denials are verified through a regression test as `gateway_data_plane` audit
events with action `authorization_denied`, outcome `failure`, reason
`tpm_limit_exceeded`, and request path. The audit event omits the Authorization header and
virtual-key token value.

## Consequences

Operators can distinguish token-throughput denials from authentication, model-policy,
request-budget, and RPM denials in the immutable audit trail. Existing HTTP compatibility
remains unchanged: exhausted TPM limits still return `429 tpm_limit_exceeded` before any
upstream call.
