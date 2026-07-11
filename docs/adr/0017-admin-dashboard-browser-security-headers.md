# ADR 0017: Add Browser Security Headers to Gateway Admin Dashboard

## Status

Accepted

## Context

The gateway admin dashboard is a browser-facing control-plane surface. Even when it is served from localhost or behind a trusted proxy, enterprise deployments may involve shared browsers, reverse proxies, or embedded admin portals. The dashboard should therefore opt out of framing, MIME sniffing, referrer leakage, and broad browser resource loading by default.

## Decision

The Prodex-owned admin dashboard response now includes:

- `Cache-Control: no-store`
- `X-Content-Type-Options: nosniff`
- `X-Frame-Options: DENY`
- `Referrer-Policy: no-referrer`
- a restrictive `Content-Security-Policy` that defaults to `none`, permits only the dashboard's inline script/style requirements, allows same-origin API calls, and denies framing via `frame-ancestors 'none'`

This applies only to the Prodex admin dashboard response and does not alter upstream data-plane pass-through responses.

## Consequences

The dashboard is safer by default for regulated deployments and reverse-proxy exposure. The current single-file inline dashboard remains compatible because the CSP explicitly allows the inline script and style it already uses.
