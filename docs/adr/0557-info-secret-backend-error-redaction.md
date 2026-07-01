# ADR 0557: Redact info secret-backend errors

## Status

Accepted

## Context

`prodex info` and doctor bundle helpers expose the configured secret backend in
human-readable and JSON output. When secret backend selection failed, the output
used the raw error string. Selection errors can include environment-derived
values, keyring service names, or secret-like URL/token material from
misconfiguration.

## Decision

Secret-backend selection errors now pass through the shared secret-like text
redactor before being rendered in info summaries or JSON values. Successful
backend kind and keyring service reporting is unchanged.

## Consequences

- Operators still see that secret backend selection failed.
- Bearer tokens and key-bearing URL query values are removed from info/doctor
  secret backend diagnostics.
- Regression coverage pins the local boundary helper.
