# ADR 0006: OIDC discovery and JWKS are cached

## Status

Accepted.

## Context

Phase 0 audit identified OIDC discovery and JWKS network fetches on the admin
request path. Fetching metadata for every request adds latency, couples request
success to IdP availability, and creates an avoidable hot-path network
dependency.

The enterprise target requires asynchronous refresh, cache TTL from HTTP cache
metadata, stale-while-revalidate, backoff, and last-known-good behavior. Those
larger semantics should land incrementally without changing gateway API behavior.

## Decision

Gateway OIDC metadata now uses a process-local bounded-time cache shared by the
local rewrite proxy instance. Discovery documents and JWKS payloads are cached by
URL for five minutes after a successful fetch.

This is a Phase 0 hardening step: it removes repeated network fetches from
normal request handling while preserving current validation and error behavior
on cache miss.

## Consequences

- Repeated OIDC-authenticated admin requests no longer fetch JWKS/discovery on
  every request.
- A process restart or cache expiry still performs synchronous fetch. A later
  phase must add background refresh, HTTP-cache TTL handling, stale last-known
  good keys, metrics, and IdP outage behavior.
