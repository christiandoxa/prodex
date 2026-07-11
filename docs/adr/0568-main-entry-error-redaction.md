# ADR 0568: Redact root CLI error output

## Status

Accepted

## Context

`main_entry` is the final CLI error boundary. It printed detailed `{err:#}`
chains directly to stderr before exiting. Lower-level commands increasingly
redact their own API and diagnostic responses, but any unhandled chain reaching
the root boundary could still include bearer tokens, key-bearing URLs, or
provider diagnostics.

## Decision

Keep the existing `Error:` stderr shape and exit-code behavior, but pass the full
error chain through the shared secret-like text redactor before printing.

## Consequences

- Root CLI failures retain useful context while removing secret-like material.
- Command-specific exit codes are unchanged.
- Lower-level redaction boundaries remain useful; the root boundary is a final
  defense-in-depth guard.
