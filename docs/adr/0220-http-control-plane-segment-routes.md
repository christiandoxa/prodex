# 0220: HTTP Control-Plane Segment Routes

## Status

Accepted.

## Context

Control-plane route classification is a security boundary for admin and SCIM
HTTP adapters. The gateway must recognize supported legacy and versioned
control-plane mounts, but unrelated routes that only share a string prefix must
not inherit control-plane behavior.

Plain prefix checks would classify paths such as `/administrator` or
`/scimitar` as control-plane traffic.

## Decision

Match control-plane mounts on path segment boundaries only. A route is
control-plane when it is exactly one of the supported mounts or when the mount
is followed by `/`:

- `/admin`
- `/v1/admin`
- `/scim`
- `/v1/scim`

## Consequences

- Versioned and legacy control-plane routes remain supported.
- Prefix-confusable paths fail closed as unknown routes.
- Authorization and tenant isolation remain owned by the application/control
  plane instead of the HTTP classifier.
