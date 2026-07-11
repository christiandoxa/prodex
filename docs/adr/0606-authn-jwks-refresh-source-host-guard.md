# ADR 0606: Authn JWKS Refresh Source Host Guard

## Status

Accepted.

## Context

OIDC/JWKS refresh planning is restricted to background control-plane mode and
requires HTTPS URLs. A string such as `https://` still satisfied the prefix
check while naming no host.

## Decision

JWKS refresh source validation now requires `https://` plus a non-empty host.
The same validation is applied to configured JWKS URLs and discovery URLs built
from the issuer.

## Consequences

Background refresh adapters receive only network targets with an explicit HTTPS
host. Request-path authentication remains network-free.
