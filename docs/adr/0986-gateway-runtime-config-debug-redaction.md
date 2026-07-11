# ADR 0986: Gateway runtime config debug redaction

## Status

Accepted.

## Context

Gateway launch configuration structs carry admin token hashes, SSO proxy token
hashes, OIDC issuer/audience metadata, state-store URLs, observability bearer
tokens, guardrail webhook bearer tokens, local filesystem paths, and tenant or
scope selectors. Derived `Debug` output can copy those values into panic output,
test failure output, or runtime diagnostics.

## Decision

`RuntimeGatewayAdminToken`, `RuntimeGatewaySsoConfig`,
`RuntimeGatewayOidcConfig`, `RuntimeGatewayStateStore`,
`RuntimeGatewayObservabilityConfig`, and
`RuntimeGatewayGuardrailWebhookConfig` use hand-written `Debug`
implementations that preserve the variant/type shape while redacting token,
URL, path, claim/header, tenant/scope, and endpoint fields.

## Consequences

Operators still get enough shape information to distinguish the configured
backend or feature block, but debug output cannot disclose launch secrets or
sensitive routing metadata. Typed accessors and runtime behavior are unchanged.
Regression coverage lives in
`crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_config.rs`.
