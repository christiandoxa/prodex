# 0218: HTTP Control-Plane Idempotency Fingerprint

## Status

Accepted

## Context

Control-plane idempotency replay detects reused keys by comparing request
fingerprints. The application boundary previously accepted an arbitrary
fingerprint string, which made it easy for HTTP composition roots to create
inconsistent fingerprints for the same mutating admin request.

The HTTP boundary does not own request body bytes, but concrete HTTP adapters
can compute a body digest while reading the request body.

## Decision

Add `control_plane_request_fingerprint` to `prodex-gateway-http`. The helper
builds a deterministic fingerprint from the HTTP method, normalized path, and a
body digest supplied by the adapter. It rejects empty paths, empty body digests,
and body digest values containing non-graphic characters. Errors use a stable
redacted response.

Add `plan_application_control_plane_idempotency_from_http_digest` to
`prodex-application`. It builds the fingerprint with the HTTP helper and then
reuses the existing HTTP idempotency-key planner.

## Consequences

- HTTP composition roots have one canonical fingerprint recipe for mutating
  admin requests.
- Fingerprint errors are rejected before replay storage planning and do not
  expose raw body digests.
- Body hashing remains in framework adapters that already read the request
  stream; the boundary stays side-effect-free and body-free.
- Reused idempotency keys are compared against stable method/path/body-digest
  fingerprints.
