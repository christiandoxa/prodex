# ADR 0988: Gateway store debug redaction

## Status

Accepted.

## Context

Gateway virtual-key store DTOs and governance scopes carry token hashes,
virtual-key names, SCIM user identifiers, tenant/team/project/user/budget
selectors, model allow-lists, budgets, rate limits, and timestamps. Derived
`Debug` output can leak that tenant and credential metadata through panics,
test failures, or diagnostic logs.

## Decision

`RuntimeGatewayVirtualKeyEntry`, `RuntimeGatewayVirtualKeyStoreFile`,
`RuntimeGatewayScimUser`, `RuntimeGatewayStoredVirtualKey`, and
`RuntimeGatewayGovernanceScope` use hand-written `Debug` implementations that
preserve low-cardinality shape while redacting credential, identity, tenant,
model, quota, and timing fields.

## Consequences

Serialization, matching, and store behavior are unchanged. Diagnostic output can
still distinguish DTO types, source variants, active/disabled state, and store
counts without exposing tenant or credential metadata. Regression coverage lives
in
`crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_store_types.rs`
and
`crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_scope.rs`.
