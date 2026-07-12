# ADR 1076: Canonical Admin Mutation Execution

## Status

Accepted.

## Context

The compatibility gateway admin path authorized a typed control-plane route, but key and SCIM
handlers still duplicated mutation policy. A process-local idempotency preclaim, storage mutation,
and success audit could occur as separate steps. That left restart and multi-process gaps, allowed
authoritative records to change between preflight and commit, and could fork an audit chain when
concurrent writers selected the same previous digest.

Secret rotation also had to preserve the existing PATCH compatibility surface without treating a
generic key update as an authorized rotate action. SCIM replacement required PUT compatibility and
the same optimistic-concurrency behavior as key mutations.

## Decision

All gateway key and SCIM mutations use one `RuntimeGatewayAdminMutationExecution` derived from the
canonical HTTP route and retained authorized action. The execution binds the exact principal,
tenant, operation, and resource ID, then obtains application-owned governance, idempotency, audit,
and precondition plans. The legacy PATCH rotate shape is an explicit alias that is reauthorized as
`VirtualKeyRotateSecret`; `/keys/{id}/secret` and `/keys/{id}/secrets` bind `{id}` directly.

Presented idempotency keys are SHA-256 hashed and scoped to the authenticated principal before
storage. File, SQLite, PostgreSQL, and Redis adapters reject a duplicate durably before invoking
the mutation closure. A successful projection, completed idempotency marker, and canonical audit
chain link commit under the same file lock, SQL transaction, or Redis WATCH/MULTI execution. The
audit digest is computed from the terminal chain head inside that boundary. Duplicate requests
return the stable conflict response without a second audit event or stored response/secret body.

Authoritative key and SCIM pre-images are rechecked inside the same boundary. `If-Match` is applied
there for key update/delete and SCIM PATCH/PUT/delete, including `*`; stale values return an audited
412 without mutation. Application identity planners validate governance and produce the stored
projection. Generated or supplied key tokens remain zeroizing values, are created only after the
durable duplicate check, and are returned once without entering idempotency or audit storage.

File storage retains bounded version-2 idempotency and audit histories. SQLite and PostgreSQL
ensure the action tenant exists before inserting metadata. Redis uses compare-and-delete lock
release and a single watched versioned state write. Backend-specific failures map at the atomic
storage boundary to the same stable local error classes.

## Consequences

- `prodex-app` remains the composition and storage-adapter boundary; mutation authorization,
  governance, identity, audit, idempotency, and precondition policy is application/domain owned.
- Process-local idempotency state, direct success audit calls, static lifecycle digests, and legacy
  key/SCIM mutation helpers are removed.
- SCIM PUT is an explicit update method. Existing PATCH behavior and the documented rotate aliases
  remain compatible, but every effective rotate uses the exact rotate authorization and audit
  action.
- Denials never persist raw credentials, request tokens, generated tokens, or response bodies.

## Verification

```bash
cargo test -q gateway_admin_ -- --test-threads=1
cargo test -q --test enterprise_binaries -- --test-threads=1
cargo clippy -q -p prodex-app -p prodex-application -p prodex-domain \
  -p prodex-gateway-http --all-targets --all-features -- -D warnings
node scripts/ci/production-boundary-guard.mjs --self-test
node scripts/ci/production-boundary-guard.mjs
```
