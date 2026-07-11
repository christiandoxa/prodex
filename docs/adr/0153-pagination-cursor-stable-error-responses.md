# ADR 0153: Pagination cursor stable error responses

## Status

Accepted

## Context

The domain API governance boundary validates pagination cursors before
control-plane list operations can use them. Raw `CursorError` values distinguish
empty cursors from overlong cursors and may include the supplied cursor length.
That detail is useful for trusted diagnostics, but client-visible API responses
should remain stable and should not leak parser limits or request-controlled
cursor metadata.

## Decision

Add `plan_cursor_error_response` to `prodex-domain`. It maps every cursor
validation failure to one stable response plan:

- status: `BadRequest`;
- code: `pagination_cursor_invalid`; and
- message: `pagination cursor is invalid`.

The raw `CursorError` remains available to trusted diagnostics and tests. Public
API boundaries should use the response plan before returning cursor validation
failures.

## Consequences

Gateway and control-plane list endpoints can provide deterministic
machine-readable pagination failures without coupling clients to internal cursor
length limits or parser distinctions.
