# ADR 0987: Gateway billing debug redaction

## Status

Accepted.

## Context

Gateway billing ledger and summary DTOs carry virtual-key names, tenant/team/
project/user/budget selectors, request and call identifiers, token counts,
costs, model names, response sizes, and timestamps. Derived `Debug` output can
copy that accounting and tenant metadata into panic output, test logs, or
runtime diagnostics.

## Decision

`RuntimeGatewayBillingLedgerEntry`, `RuntimeGatewayBillingSummaryRecord`,
`RuntimeGatewayBillingSummaryKeyDimensions`, and
`RuntimeGatewayBillingSummaryBucket` use hand-written `Debug` implementations.
They preserve the DTO type and low-cardinality phase/status shape while
redacting key, tenant, identity, request/call, model, token, cost, byte, and
timing fields.

## Consequences

JSON serialization and existing billing behavior stay unchanged. Debug output
is safe to include in diagnostics without exposing tenant or accounting data.
Regression coverage lives in
`crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_ledger_types.rs`
and
`crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_billing_summary.rs`.
