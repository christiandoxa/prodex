# 0413: Kubernetes rejects all-zero image digests

## Status

Accepted

## Context

The Kubernetes baseline requires immutable image digest references. An all-zero
`sha256` value satisfies a shallow string check but is still a placeholder, so a
review can miss that the production manifest has not been pinned to a real
release image.

## Decision

`deploy/kubernetes/prodex-gateway.yaml` keeps digest-pinned image references.
The deployment security guard now fails if a Kubernetes manifest contains a
malformed `@sha256:` value or a 64-character all-zero placeholder digest.

## Consequences

- Checked-in Kubernetes artifacts cannot claim digest pinning with malformed or
  all-zero placeholder values.
- Operators still replace the sample digest with the published digest for the
  release they deploy.
