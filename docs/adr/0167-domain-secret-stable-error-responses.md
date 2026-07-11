# 0167: Domain Secret Stable Error Responses

## Status

Accepted

## Context

Enterprise deployments require domain objects to reference secrets through
`SecretRef` rather than raw secret strings. Resolution and rotation are handled
by adapters such as development env/file providers or external secret managers.
Resolution failures and invalid rotation policies can include provider names,
secret paths, versions, refresh timestamps, or backend topology when surfaced
directly.

Those details are useful for trusted diagnostics but must not appear in
client-visible API responses.

## Decision

`prodex-domain` owns stable secret response planners:

- `plan_secret_resolution_error_response`
- `plan_secret_rotation_policy_error_response`

The planners map secret lookup, authorization, provider availability, stale
version, and rotation policy failures to stable status/code/message response
plans. They deliberately omit secret names, provider paths, versions, refresh
timing, raw material, and backend topology.

## Consequences

- Domain and application adapters can fail closed without exposing secret
  references or secret-manager internals.
- Rotation policy validation remains reusable across control-plane and gateway
  composition roots.
- Existing secret material redaction is complemented by stable error
  envelopes.
