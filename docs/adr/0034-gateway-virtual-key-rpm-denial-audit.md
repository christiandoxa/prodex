# ADR 0034: Verify gateway virtual-key RPM-limit denials as authorization audit events

## Status

Accepted

## Context

Gateway virtual keys may enforce a per-minute request limit before upstream provider
transport. When the per-key rate limit is exhausted, the gateway returns
`429 rpm_limit_exceeded`. This protects local capacity and tenant fairness and is part of
enterprise rate-limit/accounting behavior.

Rate-limit denials are credentialed data-plane authorization decisions. Audit records must
not persist the virtual-key bearer token.

## Decision

RPM-limit denials are verified through a regression test as `gateway_data_plane` audit
events with action `authorization_denied`, outcome `failure`, reason
`rpm_limit_exceeded`, and request path. The audit event omits the Authorization header and
virtual-key token value.

## Consequences

Operators can distinguish rate-limit denials from authentication, model-policy, and budget
denials in the immutable audit trail. Existing HTTP compatibility remains unchanged:
exhausted RPM limits still return `429 rpm_limit_exceeded` before any upstream call.
