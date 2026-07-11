# ADR 0286: Kubernetes Migration Secret Scope

## Status

Accepted.

## Context

The Kubernetes migration Job is the future external migrator path. It needs
database credentials, but it does not need gateway bearer tokens, provider API
keys, Redis URLs, or data-plane runtime secrets. Mounting the full gateway
secret set into the migrator increases blast radius for a job that is intended
to run schema operations only.

## Decision

Add a dedicated `prodex-gateway-migration-secrets` ExternalSecret containing
only `PRODEX_GATEWAY_POSTGRES_URL`, and make the migration Job read that secret
instead of `prodex-gateway-secrets`.

The deployment security guard now requires the migration-only secret marker and
its self-test rejects manifests without that secret.

## Consequences

- The external migrator path follows least privilege for Kubernetes secrets.
- Provider credentials, gateway tokens, and Redis coordination secrets are not
  mounted into migration pods.
- Future migrator commands can still use the durable PostgreSQL source of truth
  without making request-serving gateway pods responsible for DDL.
