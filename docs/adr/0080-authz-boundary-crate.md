# ADR 0080: Authz boundary crate

## Status
Accepted

## Context
The target architecture separates gateway data-plane authorization from
control-plane authorization while keeping `prodex-app` as a composition root.
Pure domain primitives already model principals, tenant context, credential
scopes, roles, resources, and tenant checks, but serving code needs reusable
boundary policies that prevent data-plane credentials, control-plane
credentials, and break-glass credentials from becoming interchangeable bypasses.

## Decision
Introduce `prodex-authz` as a small application authorization boundary crate on
top of `prodex-domain`. The crate maps named serving boundaries to explicit
requirements and performs strict scope, role, and tenant checks.

Unlike the lower-level domain `authorize_scope` helper, boundary authorization
does not treat break-glass as an implicit bypass. Break-glass credentials are
accepted only for the explicit break-glass boundary. Control-plane credentials
cannot call data-plane inference/quota boundaries, and data-plane credentials
cannot call control-plane admin/export boundaries.

## Consequences
Gateway and control-plane handlers can migrate to a shared authorization layer in
small steps. This keeps endpoint-level authorization consistent, makes negative
tests for horizontal/vertical privilege escalation reusable, and reduces the
risk that root/gateway/admin tokens become inference or quota bypasses.
