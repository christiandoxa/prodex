# ADR 0009: External Secrets and Vault

- Status: Accepted
- Scope: production provider and service credentials

## Context

Embedding secret values in policy, registry, configuration, logs or audit makes
review, rotation and tenant isolation unsafe. Per-request Vault calls would add
an external dependency to the hot path.

## Decision

Persist opaque `secret_ref` values only. An allow-list binds tenant, purpose and
provider to approved secret namespaces. Resolve through an approved external
secret injector at launch/background refresh into a bounded in-memory value;
never during a streaming chunk path. The Vault Agent, Secrets Store CSI driver
or equivalent deployment component owns Vault authentication, lease renewal
and projection. Exclude values from serialization, Debug/display, errors, logs,
metrics, audit and diagnostic bundles. Rotation and revocation invalidate
caches and do not require policy content changes.
`bank_enforce` rejects raw production secrets, unauthorized namespaces and
missing external-secret prerequisites at startup.

## Consequences

Availability behavior is explicit: a valid projected version may continue only
where policy permits; otherwise required operations deny. Canary scans,
rotation/revocation and injector-outage drills are mandatory deployment
evidence. Development-only local backends remain clearly labelled and cannot
satisfy bank mode.

## Implementation status

Secret references, projected-secret resolution, redacted wrappers, purpose
binding, exact projected versions, anchored filesystem validation, atomic
projection rotation and bank raw-secret rejection are implemented. The
projected-secret provider is the production boundary; a second direct Vault
HTTP authority is intentionally excluded. Managed injector authentication,
lease renewal and outage drills remain deployment evidence. No request-path
Vault call is permitted.
