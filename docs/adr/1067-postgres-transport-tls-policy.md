# ADR 1067: PostgreSQL Transport TLS Policy

## Status

Accepted.

## Context

The gateway used caller-supplied PostgreSQL URLs but opened pooled accounting,
blocking compatibility, and migration connections with `NoTls`. URL parameters
could not provide encryption because the clients had no TLS connector.

## Decision

`gateway.state.postgres_tls_mode` accepts only `verify-full` or `disable`.
Production defaults to `verify-full` and rejects `disable`; development defaults
to `disable` for local compatibility. `verify-full` forces PostgreSQL
`sslmode=require` and uses rustls with native trust roots plus the optional PEM
bundle at `postgres_tls_ca_path`. Rustls verifies both the certificate chain and
server hostname.

The selected policy is carried with the PostgreSQL state store and reused by
the async pool, blocking admin/usage/ledger operations, and compatibility
migrations. The standalone migrator defaults to `verify-full`; local plaintext
migrations require the explicit `--tls-mode disable` flag.

## Consequences

Production PostgreSQL traffic cannot silently downgrade to plaintext, including
when a URL contains a conflicting SSL mode. Private database issuers require an
operator-mounted CA bundle. The Docker proof enables PostgreSQL TLS, verifies
encrypted pooled and blocking sessions through `pg_stat_ssl`, and confirms that
a hostname mismatch fails closed.
