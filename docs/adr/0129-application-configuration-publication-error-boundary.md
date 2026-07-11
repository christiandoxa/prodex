# ADR 0129: Application configuration publication error boundary

## Status

Accepted

## Context

`prodex-application` composes control-plane configuration publication with
append-only audit storage planning. A publication can be denied by the
control-plane/configuration boundary, or the composed use case can fail while
planning durable audit storage. Composition roots need stable response plans for
both cases so raw storage errors, SQL backend names, tenant identifiers,
revision identifiers, and configuration payloads do not leak into admin API
responses.

## Decision

Add two HTTP-neutral helpers to `prodex-application`:

- `plan_application_configuration_publication_decision_error_response` maps a
  denied `ConfigurationPublicationDecision` through the control-plane
  publication response boundary; authorized decisions return no client-visible
  error; and
- `plan_application_configuration_publication_error_response` maps audit-storage
  planning failures to `audit_storage_unavailable` with a service-unavailable
  status.

The raw decision and storage error remain available for trusted diagnostics and
append-only audit workflows.

## Consequences

Application composition roots can expose deterministic configuration publication
errors without duplicating control-plane/config response mapping or leaking
storage topology, SQL planning details, tenant IDs, revision IDs, or payload
contents.
