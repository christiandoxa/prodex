# ADR 0446: Control-plane mounts reject data-plane credentials

## Status

Accepted.

## Context

Client/data-plane keys must not become admin credentials. Prodex currently
supports control-plane routes under `/admin`, `/v1/admin`, `/scim`, and
`/v1/scim`.

## Decision

Application authentication regression coverage now checks each current
control-plane mount against a data-plane credential. The boundary must reject
the request with `CredentialScopeMismatch` before control-plane handling.

## Consequences

No runtime behavior changes. New control-plane mounts should extend the same
matrix.
