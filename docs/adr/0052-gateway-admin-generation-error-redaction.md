# ADR 0052: Redact gateway admin generation failures

## Status

Accepted

## Context

Gateway admin key creation, key rotation, and SCIM user provisioning generate
opaque identifiers or virtual-key tokens using OS randomness. Although failures
are rare, returning the raw generation error would expose implementation details
from the randomness provider and create an unstable API contract for regulated
control-plane-style endpoints.

## Decision

Admin generation failures now use stable, redacted error envelopes:

- `gateway_key_generation_failed`: `gateway key token could not be generated`
- `gateway_scim_user_generation_failed`: `gateway SCIM user id could not be generated`

The handlers ignore the raw generation error when constructing client-visible
responses. Unit coverage pins the stable status, code, and message and verifies
that low-level randomness diagnostics are not part of the response message.

## Consequences

- Admin clients receive deterministic machine-readable errors for rare token or
  SCIM identifier generation failures.
- OS randomness provider details remain internal diagnostics rather than API
  response content.
- The response envelope remains compatible with existing error-code handling
  while improving secure-by-default behavior.
