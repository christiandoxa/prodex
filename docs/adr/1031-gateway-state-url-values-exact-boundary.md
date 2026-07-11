# ADR 1031: Gateway state URL values exact boundary

## Status

Accepted.

## Context

Gateway state backends read PostgreSQL and Redis connection URLs from
environment variables. Runtime resolution previously trimmed those URL values,
and the multi-replica topology planner treated a blank `PRODEX_GATEWAY_REDIS_URL`
as if Redis coordination was simply absent. That hid malformed deployment
secrets and could make readiness decisions from a different value than the one
operators configured.

## Decision

Gateway state URL values are now exact runtime inputs. PostgreSQL and Redis URL
environment values must be non-empty and must not contain whitespace. The
multi-replica shared Redis presence check also fails closed when
`PRODEX_GATEWAY_REDIS_URL` is explicitly empty or whitespace-bearing.

## Consequences

Operators see malformed state backend URL secrets at startup instead of
silently trimming them or treating them as absent. Deployments that do not use
Redis coordination should leave `PRODEX_GATEWAY_REDIS_URL` unset.
