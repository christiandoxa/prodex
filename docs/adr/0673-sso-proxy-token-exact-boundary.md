# ADR 0673: SSO Proxy Token Exact Boundary

## Status

Accepted.

## Context

Gateway SSO admin authentication can use a profile-scoped proxy token header to
trust upstream SSO metadata. The role, tenant, scope, and key-prefix fields
already fail closed on whitespace-bearing values, but the proxy token itself was
trimmed before verification.

Trimming credential material can turn a padded header value into a valid
control-plane credential and hides malformed proxy integrations.

## Decision

The SSO proxy token header is verified exactly as received. Padded or otherwise
different token bytes do not authenticate.

## Consequences

Canonical SSO proxy token headers are unchanged. Malformed padded token headers
fail with the existing unauthenticated admin response.
