# ADR 0032: Verify gateway virtual-key model-policy denials as authorization audit events

## Status

Accepted

## Context

Gateway virtual keys may restrict callers to an explicit set of models. Requests for a
model outside that allow-list return `403 model_not_allowed` before upstream provider
transport. This is a data-plane authorization decision and should be observable in the
same immutable audit trail used for other gateway security denials.

The virtual-key bearer token is credential material and must not be persisted in audit
records.

## Decision

Model allow-list denials are covered by the gateway virtual-key rejection audit path and
are verified as `gateway_data_plane` events with action `authorization_denied`, outcome
`failure`, reason `model_not_allowed`, and the request path. The audit record omits the
Authorization header and virtual-key token value.

## Consequences

Operators can distinguish authentication failures from authorized-key policy denials while
investigating tenant or automation behavior. Existing HTTP compatibility remains unchanged:
model allow-list violations still return `403 model_not_allowed` before any upstream call.
