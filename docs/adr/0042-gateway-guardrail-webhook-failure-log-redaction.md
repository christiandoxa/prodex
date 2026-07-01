# ADR 0042: Gateway guardrail webhook failure diagnostics redact endpoints

## Status

Accepted

## Context

Guardrail webhook URLs are configuration values, but deployments may still include sensitive routing data in paths or query parameters. When a webhook call failed, runtime diagnostics previously logged the raw webhook URL and full reqwest error. Some reqwest error messages can include the URL, which could leak secret-bearing webhook endpoints into shared runtime logs.

Webhook failures are operationally important, especially when `fail_closed` turns them into policy denials, but diagnostics should not expose endpoint secrets or bearer credentials.

## Decision

Guardrail webhook failure runtime logs no longer include the raw URL or full error text. They record low-cardinality metadata only:

- request id;
- webhook phase;
- `endpoint=redacted`;
- coarse `error_kind` such as `connect`, `timeout`, `request`, `body`, `decode`, or `other`.

Fail-closed behavior is unchanged. The resulting policy denial is still audited through the guardrail webhook denial audit event with metadata-only reason `webhook_error`.

## Consequences

Operators can distinguish webhook connectivity/request failure classes without leaking webhook endpoint details into runtime logs. Deep debugging of exact endpoint configuration must use controlled configuration inspection rather than aggregated diagnostic logs.

## Validation

A regression test configures a fail-closed pre-request webhook URL containing a query secret and points it at an unavailable endpoint. The test verifies that:

- the gateway fails closed with the existing policy error;
- the denial is audited as `guardrail_webhook_blocked` with reason `webhook_error`;
- audit and runtime logs omit the gateway token, webhook bearer token, URL query secret, and raw endpoint;
- runtime diagnostics retain `gateway_guardrail_webhook_failed`, `endpoint=redacted`, and `error_kind` metadata.
