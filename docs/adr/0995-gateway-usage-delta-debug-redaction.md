# ADR 0995: Gateway usage delta debug redaction

## Status

Accepted.

## Context

Gateway virtual-key usage deltas carry request and call identifiers, key names,
tenant/team/project/user/budget selectors, model names, token counters, cost
estimates, and timestamps. Derived `Debug` output can leak accounting and tenant
metadata through panic output, assertions, or diagnostics.

## Decision

`RuntimeGatewayVirtualKeyUsageDelta` uses a hand-written `Debug`
implementation that preserves the accounting DTO shape while redacting
request/call, key, tenant, model, token, cost, and timing fields.

## Consequences

Usage persistence and ledger emission are unchanged. Diagnostics can still
identify the delta DTO without exposing tenant or accounting metadata.
Regression coverage lives in
`crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_usage_backend.rs`.
