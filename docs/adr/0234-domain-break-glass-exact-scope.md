# 0234: Domain Break-Glass Exact Scope

## Status

Accepted

## Context

Enterprise security requirements separate data-plane, control-plane, and
break-glass credentials. Break-glass identities must be short-lived, explicitly
authorized, and audited. They must not become an implicit bypass for inference,
quota, or ordinary control-plane authorization.

The boundary authz crate already treats `BreakGlass` as a separate scope, but
the lower-level domain `authorize_scope` primitive still accepted break-glass
credentials for any expected scope.

## Decision

`prodex-domain::authorize_scope` now requires an exact credential-scope match.
Break-glass principals are authorized only when the requirement explicitly asks
for `CredentialScope::BreakGlass`.

Control-plane emergency access continues to use the dedicated break-glass
control-plane flow, which validates reason and expiry before authorizing the
break-glass scope.

## Consequences

- Direct domain authorization callers cannot accidentally let break-glass
  credentials call data-plane inference or ordinary control-plane operations.
- Break-glass remains available through explicit break-glass requirements and
  audited control-plane flows.
- Existing gateway and authz boundary tests continue to enforce the same
  no-bypass behavior at higher layers.
