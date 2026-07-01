# ADR 0859: Redact provider invocation display topology

Status: Accepted

## Context

Provider invocation validation errors distinguish missing tenant context,
cross-tenant principals, and credential-scope mismatches. Earlier display
redaction removed concrete scope values, but generic display text still named
tenant and credential-scope topology.

## Decision

Keep typed validation variants for trusted diagnostics and response planning,
but render all provider invocation display errors as the stable authorization
failure message.

## Consequences

Provider invocation validation is unchanged, while stringified errors no longer
expose tenant or credential-scope topology.
