# ADR 0593: Deployment Image Digest Placeholder Guard

## Status

Accepted

## Context

Production Kubernetes manifests must pin container images by immutable digest.
The deployment guard already rejected missing, malformed, and all-zero sha256
digests, but repeated-pattern hex placeholders such as `aaaaaaaa...` or
`1234567890abcdef` repeated four times still looked valid to the regex.

## Decision

Reject repeated-character and repeated-pattern sha256 digest placeholders in the
deployment security guard and update the baseline Kubernetes manifest/guard
fixture to use a non-repeated example digest.

## Consequences

- Deployment examples can no longer pass the security guard with obvious
  repeated-character or repeated-pattern image digest placeholders.
- The guard still validates manifest shape only; release automation must supply
  the actual published image digest.
