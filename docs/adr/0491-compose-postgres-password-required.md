# ADR 0491: Require Compose Postgres Password Environment

## Status

Accepted.

## Context

Enterprise deployment examples must not start with blank, static, or placeholder credentials. The Compose Postgres service read `PRODEX_POSTGRES_PASSWORD` but did not fail fast when it was unset.

## Decision

The Compose Postgres service now uses shell required-variable expansion for `PRODEX_POSTGRES_PASSWORD`. The deployment security guard rejects Compose files that do not require that variable.

## Consequences

Local Postgres-profile users must set `PRODEX_POSTGRES_PASSWORD` explicitly before startup. This avoids silently starting with an empty database password.
