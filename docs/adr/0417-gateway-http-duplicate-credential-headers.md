# 0417: Gateway HTTP duplicate credential headers

## Status

Accepted

## Context

Gateway adapters replace upstream `Authorization` and `ChatGPT-Account-Id`
headers after selecting the effective profile. Duplicate inbound credential
headers are ambiguous because HTTP stacks may preserve first, preserve last, or
merge values before authentication code sees them.

## Decision

`prodex-gateway-http` rejects duplicate `Authorization` and
`ChatGPT-Account-Id` headers during request planning. The failure uses a generic
redacted `credential_header_invalid` envelope.

## Consequences

- Credential source ambiguity is rejected at the HTTP boundary.
- Data-plane and control-plane credential separation does not depend on
  framework header folding behavior.
- Raw credential names and values remain out of client-visible error messages.
