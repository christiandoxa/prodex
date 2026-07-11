# ADR 0152: API version stable error responses

## Status

Accepted

## Context

The domain API governance boundary rejects unsupported and sunset API versions.
Raw `ApiVersionError` values include requested public version identifiers and
sunset timestamps. Those details are useful for trusted diagnostics and
deprecation planning, but client-visible error envelopes should remain stable
and should not couple generated clients to policy timing internals.

## Decision

Add `plan_api_version_error_response` to `prodex-domain`. It maps API lifecycle
failures into stable response plans:

- unsupported versions become `api_version_unsupported` with a not-found status;
  and
- sunset versions become `api_version_sunset` with a gone status.

The response messages stay generic and exclude requested version values, sunset
timestamps, and policy internals.

## Consequences

Gateway and control-plane composition roots can expose deterministic API
lifecycle failures while retaining raw `ApiVersionError` values for trusted
operator diagnostics, deprecation reports, and audit trails.
