# Prodex Gateway Backup and Restore Runbook

This runbook covers the current gateway state backends. It is production
operations guidance, not a replacement for application-level migration tests.
Run these commands from an operator workstation or maintenance pod with access to
the same secrets and storage as the gateway.

## Scope

Back up all durable tenant-owned gateway state before upgrades, schema changes,
provider credential rotation, or disaster-recovery drills:

- `policy.toml` and non-secret gateway configuration.
- config-publication transport outbox/ack state when the shared filesystem
  transport is active.
- virtual-key store.
- usage counters.
- append-only billing ledger.
- audit logs.
- runtime logs when incident reconstruction is required.

Secrets should be restored through the configured secret manager, not copied into
ConfigMaps or Git.

## File backend

The file backend stores state under `PRODEX_HOME` and audit/runtime logs under
their configured directories.

If the same deployment also uses the shared config-publication transport,
snapshot that shared transport root in the same maintenance window as gateway
state and compact fully acknowledged records after successful drills with:

```bash
prodex-control-plane compact-config-publication --transport <shared-path> --retain 10
```

Backup:

```bash
stamp=$(date -u +%Y%m%dT%H%M%SZ)
mkdir -p backups
prodex quota --all --once >/dev/null # optional health read before snapshot
tar --numeric-owner -C "$PRODEX_HOME" -czf "backups/prodex-home-$stamp.tgz" .
tar --numeric-owner -C "${PRODEX_AUDIT_LOG_DIR:-$PRODEX_HOME}" -czf "backups/prodex-audit-$stamp.tgz" . || true
tar --numeric-owner -C "${PRODEX_RUNTIME_LOG_DIR:-/tmp}" -czf "backups/prodex-runtime-logs-$stamp.tgz" . || true
tar --numeric-owner -C "${PRODEX_CONFIG_PUBLICATION_TRANSPORT_ROOT:-$PRODEX_HOME}" -czf "backups/prodex-config-publication-$stamp.tgz" config-publication || true
```

Restore:

```bash
systemctl stop prodex-gateway # or scale deployment/compose service to zero
tar -C "$PRODEX_HOME" -xzf backups/prodex-home-<stamp>.tgz
systemctl start prodex-gateway
curl -fsS http://127.0.0.1:4000/readyz
```

Drill acceptance:

- `/readyz` is healthy after restore.
- admin key list returns expected tenant/key metadata.
- ledger query returns the latest known billing rows.
- if shared config-publication transport is enabled, intended gateway replicas
  can still consume or recognize restored publication records correctly.

## SQLite backend

SQLite state is safe to back up online with `VACUUM INTO` or the SQLite backup
API. Do not copy a live database file directly without a filesystem snapshot.

Backup:

```bash
stamp=$(date -u +%Y%m%dT%H%M%SZ)
sqlite3 "$PRODEX_HOME/gateway-state.sqlite" "VACUUM INTO 'backups/gateway-state-$stamp.sqlite';"
sha256sum "backups/gateway-state-$stamp.sqlite" >"backups/gateway-state-$stamp.sqlite.sha256"
```

Restore:

```bash
systemctl stop prodex-gateway
sha256sum -c backups/gateway-state-<stamp>.sqlite.sha256
install -m 0600 backups/gateway-state-<stamp>.sqlite "$PRODEX_HOME/gateway-state.sqlite"
systemctl start prodex-gateway
curl -fsS http://127.0.0.1:4000/readyz
```

Drill acceptance:

- SQLite integrity check passes: `sqlite3 gateway-state.sqlite 'PRAGMA integrity_check;'`.
- tenant-scoped admin reads show only that tenant's keys, usage, and ledger rows.

## PostgreSQL backend

PostgreSQL is the durable source of truth for multi-replica deployments. Use a
consistent dump or your managed database backup facility. Store WAL/PITR backups
outside the cluster.

Backup:

```bash
stamp=$(date -u +%Y%m%dT%H%M%SZ)
pg_dump --format=custom --no-owner --no-privileges "$PRODEX_GATEWAY_POSTGRES_URL" \
  --file "backups/prodex-gateway-$stamp.dump"
pg_dump --schema-only --no-owner --no-privileges "$PRODEX_GATEWAY_POSTGRES_URL" \
  --file "backups/prodex-gateway-schema-$stamp.sql"
sha256sum "backups/prodex-gateway-$stamp.dump" >"backups/prodex-gateway-$stamp.dump.sha256"
```

Restore to a new database first:

```bash
createdb prodex_restore
sha256sum -c backups/prodex-gateway-<stamp>.dump.sha256
pg_restore --dbname prodex_restore --clean --if-exists backups/prodex-gateway-<stamp>.dump
# point a staging gateway at prodex_restore and run smoke checks before cutover
```

Cutover:

1. Freeze writes by denying traffic or scaling gateways to zero.
2. Restore into the production database or promote the restored database.
3. Update `PRODEX_GATEWAY_POSTGRES_URL` through the secret manager.
4. Start gateways and verify `/readyz`, admin tenant scoping, usage counters,
   and ledger summaries.

Drill acceptance:

- no duplicate ledger rows for the same call ID after restore.
- usage totals match ledger summary for sampled tenants.
- cross-tenant admin reads still fail safely.

## Redis backend

Redis is suitable for distributed rate limiting, short-lived cache, and
rebuildable coordination. It is not the preferred durable source of truth for
regulated multi-replica billing. If Redis is used for gateway state, enable AOF
or managed snapshots and treat restore as a consistency drill.

Backup:

```bash
redis-cli --no-auth-warning BGSAVE
redis-cli --no-auth-warning INFO persistence
# copy the managed snapshot/RDB/AOF using your Redis provider tooling
```

Restore:

1. Stop gateway writers.
2. Restore the Redis snapshot/AOF using provider tooling.
3. Start one gateway replica and verify usage/ledger reads.
4. Scale out only after acceptance checks pass.

Drill acceptance:

- usage hashes exist per key instead of one whole-map JSON payload.
- ledger entries are append-only and latest call IDs are present.
- rate-limit counters do not overshoot documented tolerance under a two-replica
  replay test.

## Audit and evidence

For every backup or restore drill, record:

- timestamp, operator, backend, source revision, target revision.
- backup artifact digests and retention class.
- commands executed and exit status.
- `/readyz` result.
- sampled tenant/key/usage/ledger validation results.
- any data loss window or rejected requests during cutover.

Keep drill evidence in the regulated environment's audit store. Do not commit
secrets, raw tokens, or tenant data into this repository.
