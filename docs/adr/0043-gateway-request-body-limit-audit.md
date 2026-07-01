# ADR 0043: Gateway request body limit rejections are audited

## Status

Accepted

## Context

The gateway enforces a maximum request body size before request parsing and before any upstream provider call. This protects the data plane from memory pressure and oversized payload abuse. The rejection path returned `413` and wrote a runtime diagnostic, but it did not append an immutable audit event.

In multi-tenant regulated deployments, local admission and guardrail denials are security-sensitive events. Operators need audit evidence that an oversized request was rejected before provider routing without recording request bodies or bearer credentials.

## Decision

When the local rewrite gateway rejects an HTTP request because the proxied body exceeds the configured limit, it now appends a metadata-only data-plane audit event:

- `component`: `gateway_data_plane`
- `action`: `request_body_too_large`
- `outcome`: `failure`
- `details.reason`: `request_body_too_large`
- `details.path`: request path
- `state_backend`: active gateway state backend label

Runtime diagnostics use the same stable reason and path metadata instead of logging the raw error chain. The HTTP response remains the existing `413` text response for compatibility.

## Consequences

Oversized payload rejection is now visible in immutable audit logs and operational diagnostics without leaking request bodies, virtual-key tokens, or provider credentials. Upstream behavior remains unchanged: rejected oversized requests are not forwarded.

## Validation

The existing gateway oversized-body regression now also verifies that:

- no upstream request is sent;
- an audit event with `request_body_too_large` metadata is written;
- audit and runtime logs do not contain the virtual-key token or oversized input body;
- runtime diagnostics include the stable `local_rewrite_request_body_too_large` event and reason.
