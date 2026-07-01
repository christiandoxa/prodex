# ADR 0710: Gateway Base URL Exact Boundary

## Status

Accepted.

## Context

`gateway.base_url` controls the upstream HTTP endpoint used by gateway launch.
Policy validation previously only rejected blank-after-trim values, and runtime
launch trimmed the URL before parsing it. A padded endpoint could therefore be
silently normalized into a different accepted upstream target.

## Decision

Treat `gateway.base_url` as an exact configuration boundary. Policy validation
rejects empty or whitespace-bearing values, and runtime launch parses the
configured URL directly. Existing trailing-slash normalization remains.

## Consequences

Operators must fix padded base URL values in `policy.toml` or CLI input.
Gateway upstream routing remains auditable and fails closed instead of accepting
hidden cleanup.
