# ADR 0033: Verify gateway virtual-key request-budget denials as authorization audit events

## Status

Accepted

## Context

Gateway virtual keys can share a `budget_id` and enforce a request-count budget before
upstream provider transport. When the shared budget is exhausted, the gateway returns
`403 request_budget_exceeded`. This is a data-plane authorization and budget-protection
decision that supports tenant isolation and spend control.

Budget denials can involve credentialed inference requests, so audit records must not
persist virtual-key bearer tokens.

## Decision

The shared request-budget denial regression now verifies that `request_budget_exceeded`
is emitted through the gateway virtual-key rejection audit path as a `gateway_data_plane`
event with action `authorization_denied`, outcome `failure`, reason
`request_budget_exceeded`, and request path. The audit event omits all virtual-key token
material.

## Consequences

Operators can distinguish budget-protection denials from authentication failures and model
policy denials in the immutable audit trail. Existing budget behavior and HTTP
compatibility are unchanged: exhausted request budgets still return
`403 request_budget_exceeded` before any upstream call.
