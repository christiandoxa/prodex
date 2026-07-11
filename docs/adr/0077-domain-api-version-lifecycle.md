# ADR 0077: Domain API version lifecycle policy

## Status
Accepted

## Context
Prodex must preserve supported gateway and control-plane API compatibility while
still allowing insecure or obsolete API surfaces to be deprecated and eventually
removed. Version decisions should be deterministic and testable outside the HTTP
framework so every serving implementation can enforce the same lifecycle rules.

## Decision
`prodex-domain` now defines API version lifecycle primitives:

- `ApiVersion` for stable public major/minor versions;
- `ApiVersionPolicy` and `ApiVersionStatus` for current, deprecated, and sunset
  versions; and
- `evaluate_api_version` for request admission decisions.

Deprecated versions remain allowed until their sunset timestamp, with an explicit
deprecation decision that HTTP adapters can surface as headers or warnings.
Unknown or sunset versions are rejected by the domain policy.

## Consequences
Gateway and control-plane HTTP crates can share one compatibility model without
coupling the domain crate to routing libraries. Future version removals require a
policy change plus tests, reducing accidental breaking changes to existing CLI,
gateway API, and provider workflows.
