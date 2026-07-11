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
- `--backend postgres --url-ref <NAME> --secret-provider <PROVIDER> --secret-root <PATH>`
- `--backend postgres --url-env <ENV>`

The production Kubernetes migration Job uses the projected-reference form. It mounts the
migration-only Secret read-only at `/run/secrets/prodex`; the URL is resolved through the hardened
projected provider and never enters argv or the process environment. `--url-env` remains only as a
backward-compatible development/restore-drill input.

## Consequences

- Production manifests no longer advertise a placeholder migration path.
- Gateway request-serving open paths remain DDL-free; migrations run through the
  dedicated external migrator command.
- Migration connection material stays in a bounded, zeroizing `SecretMaterial` until the database
  adapter consumes it.
- The serving subcommand remains gated until the async gateway adapter is wired.
