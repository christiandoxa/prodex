# ADR 0023: Document Authenticated Gateway Admin Dashboard Contract

## Status

Accepted

## Context

The gateway admin dashboard is now an authenticated control-plane route. The OpenAPI document still described the dashboard shell as if only follow-up data requests required an admin token, and it did not list the `401` response for missing or invalid admin authentication.

## Decision

The OpenAPI operation for `/v1/prodex/gateway/admin` now explicitly declares `GatewayBearerAuth` and the `401` gateway error response. The success description also states that the dashboard shell itself is authenticated.

## Consequences

Generated clients and operators see the same security model in the machine-readable contract that the router enforces at runtime. This is documentation/contract alignment only; route behavior and data-plane pass-through semantics are unchanged.
