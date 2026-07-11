# 0656: Role Claim Exact Boundary

## Status

Accepted

## Context

`ExplicitRoleMapper` trimmed configured and incoming role claim strings before
lookup. That could turn a padded claim such as ` admins ` into the privileged
`admins` mapping. Missing and unknown claims already failed closed, but
normalizing whitespace at the authorization boundary weakens explicit mapping.

## Decision

Role mappings now ignore empty configured claims and configured claims
containing whitespace. Incoming role claims are matched exactly; only absent or
zero-length claims are missing, while whitespace-only and other
whitespace-containing claims are unknown.

## Consequences

- Admin privileges require an exact configured claim.
- Existing exact mappings keep working.
- Padded or whitespace-bearing claims fail closed to denial or Viewer fallback.
