# ADR 0559: Redact selection quota probe errors

## Status

Accepted

## Context

Runtime profile selection probes quota-compatible profiles before launch and for
selection/status helpers. Probe failures are stored in `RunProfileProbeReport`
as strings and later rendered by preflight, doctor, info, or selection views.
Raw quota errors can contain authorization headers, key-bearing upstream URLs,
or provider routing details.

## Decision

Quota probe errors are redacted at the selection report boundary before they are
stored as strings. The report type, readiness logic, and quota success handling
are unchanged.

## Consequences

- All renderers that consume selection probe reports receive safer diagnostics.
- Bearer tokens and key-bearing URL query values are removed before the error
  leaves the probe boundary.
- Regression coverage pins the local selection error redaction helper.
