# ADR 0588: Gateway External Migrator Command

## Status

Accepted

## Context

Gateway request-serving SQL open paths must not run DDL. The Kubernetes
migration Job already had a dedicated ServiceAccount, secret, and network
policy, but it still ran a placeholder shell command because no versioned
gateway migrator entrypoint existed.

## Decision

Add `prodex-gateway migrate` as the external migration entrypoint. The command
executes the versioned storage migration plans for:

- `--backend sqlite --path <PATH>`
- `--backend postgres --url-env <ENV>`

The production Kubernetes migration Job now runs
`prodex-gateway migrate --backend postgres --url-env PRODEX_GATEWAY_POSTGRES_URL`
with migration-only credentials.

## Consequences

- Production manifests no longer advertise a placeholder migration path.
- Gateway request-serving open paths remain DDL-free; migrations run through the
  dedicated external migrator command.
- The serving subcommand remains gated until the async gateway adapter is wired.
