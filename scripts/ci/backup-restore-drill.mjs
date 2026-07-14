#!/usr/bin/env node
import { spawn } from "node:child_process";
import { createCipheriv, createDecipheriv, createHash, randomBytes } from "node:crypto";
import { createReadStream } from "node:fs";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { performance } from "node:perf_hooks";
import { fileURLToPath } from "node:url";

const modulePath = fileURLToPath(import.meta.url);
const scriptDir = path.dirname(modulePath);
const repoRoot = path.resolve(scriptDir, "..", "..");
const postgresImage = "postgres:16-alpine";
const sourceDatabase = "prodex_drill_source";
const restoreDatabase = "prodex_drill_restore";
const backupArtifactMagic = Buffer.from("PRODEX-BACKUP-V1", "ascii");
const evidencePath = path.resolve(
  repoRoot,
  process.env.PRODEX_BACKUP_DRILL_EVIDENCE_PATH ??
    "target/backup-restore-drill/evidence.json",
);

const tenantA = "00000000-0000-7000-8000-000000000001";
const tenantB = "00000000-0000-7000-8000-000000000002";

const seedSql = String.raw`
INSERT INTO prodex_tenants VALUES
  ('${tenantA}', 'tenant-a', 1000, 1000),
  ('${tenantB}', 'tenant-b', 1000, 1000);
INSERT INTO prodex_virtual_keys VALUES
  ('${tenantA}', '00000000-0000-7000-8000-000000000011', '00000000-0000-7000-8000-000000000101', 'key-a', 'external', 'key-a', '1', 1000, 1000, NULL),
  ('${tenantB}', '00000000-0000-7000-8000-000000000012', '00000000-0000-7000-8000-000000000102', 'key-b', 'external', 'key-b', '1', 1000, 1000, NULL);
INSERT INTO prodex_service_identities VALUES
  ('${tenantA}', '00000000-0000-7000-8000-000000000101', 'service-a', 1000, NULL),
  ('${tenantB}', '00000000-0000-7000-8000-000000000102', 'service-b', 1000, NULL);
INSERT INTO prodex_users VALUES
  ('${tenantA}', '00000000-0000-7000-8000-000000000201', 'user-a', 'User A', 1000, 1000, NULL),
  ('${tenantB}', '00000000-0000-7000-8000-000000000202', 'user-b', 'User B', 1000, 1000, NULL);
INSERT INTO prodex_role_bindings VALUES
  ('${tenantA}', '00000000-0000-7000-8000-000000000301', '00000000-0000-7000-8000-000000000201', 'Viewer', 1000, NULL),
  ('${tenantB}', '00000000-0000-7000-8000-000000000302', '00000000-0000-7000-8000-000000000202', 'Viewer', 1000, NULL);
INSERT INTO prodex_provider_credentials VALUES
  ('${tenantA}', '00000000-0000-7000-8000-000000000401', 'openai', 'external', 'provider-a', '1', 1000),
  ('${tenantB}', '00000000-0000-7000-8000-000000000402', 'openai', 'external', 'provider-b', '1', 1000);
INSERT INTO prodex_budget_counters (
  tenant_id, storage_scope, virtual_key_id, reserved_tokens,
  reserved_cost_micros, committed_tokens, committed_cost_micros,
  request_count, updated_at_unix_ms
) VALUES
  ('${tenantA}', 'tenant-default', NULL, 0, 0, 15, 150, 0, 1000),
  ('${tenantB}', 'tenant-default', NULL, 0, 0, 25, 250, 0, 1000);
INSERT INTO prodex_budget_policies VALUES
  ('${tenantA}', 'tenant-default', 1000, 10000, 1000),
  ('${tenantB}', 'tenant-default', 1000, 10000, 1000);
INSERT INTO prodex_reservations VALUES
  ('${tenantA}', '00000000-0000-7000-8000-000000000501', '00000000-0000-7000-8000-000000000601', NULL, 'reservation-a', 15, 150, 1000, 2000, 1500, NULL),
  ('${tenantB}', '00000000-0000-7000-8000-000000000502', '00000000-0000-7000-8000-000000000602', NULL, 'reservation-b', 25, 250, 1000, 2000, 1500, NULL);
INSERT INTO prodex_usage_ledger VALUES
  ('${tenantA}', '00000000-0000-7000-8000-000000000701', '00000000-0000-7000-8000-000000000501', '00000000-0000-7000-8000-000000000601', 'committed', 15, 150, 1500),
  ('${tenantB}', '00000000-0000-7000-8000-000000000702', '00000000-0000-7000-8000-000000000502', '00000000-0000-7000-8000-000000000602', 'committed', 25, 250, 1500);
INSERT INTO prodex_audit_log VALUES
  ('${tenantA}', '00000000-0000-7000-8000-000000000801', NULL, 'digest-a', 1500, '00000000-0000-7000-8000-000000000101', 'drill.seed', 'tenant', 'tenant-a', 'success', NULL),
  ('${tenantB}', '00000000-0000-7000-8000-000000000802', NULL, 'digest-b', 1500, '00000000-0000-7000-8000-000000000102', 'drill.seed', 'tenant', 'tenant-b', 'success', NULL);
INSERT INTO prodex_idempotency_records VALUES
  ('${tenantA}', 'request-a', 'fingerprint-a', 'completed', 1000, 1500, decode('01', 'hex')),
  ('${tenantB}', 'request-b', 'fingerprint-b', 'completed', 1000, 1500, decode('02', 'hex'));
INSERT INTO prodex_policy_revisions VALUES
  ('${tenantA}', '00000000-0000-7000-8000-000000000901', 'policy-checksum-a', '{}', 'active', '00000000-0000-7000-8000-000000000201', 1000),
  ('${tenantB}', '00000000-0000-7000-8000-000000000902', 'policy-checksum-b', '{}', 'active', '00000000-0000-7000-8000-000000000202', 1000);
INSERT INTO prodex_policy_pointers VALUES
  ('${tenantA}', '00000000-0000-7000-8000-000000000901', '00000000-0000-7000-8000-000000000901', 'policy-etag-a', 1000),
  ('${tenantB}', '00000000-0000-7000-8000-000000000902', '00000000-0000-7000-8000-000000000902', 'policy-etag-b', 1000);
INSERT INTO prodex_policy_activation_history VALUES
  ('${tenantA}', '00000000-0000-7000-8000-000000000911', '00000000-0000-7000-8000-000000000901', NULL, 'activate', '00000000-0000-7000-8000-000000000201', 1000),
  ('${tenantB}', '00000000-0000-7000-8000-000000000912', '00000000-0000-7000-8000-000000000902', NULL, 'activate', '00000000-0000-7000-8000-000000000202', 1000);
INSERT INTO prodex_approvals VALUES
  ('${tenantA}', 'approval-a', 'policy_revision', 'policy', 'approval-fingerprint-a', '00000000-0000-7000-8000-000000000201', 'active', 1, 10000, 1000, 1),
  ('${tenantB}', 'approval-b', 'policy_revision', 'policy', 'approval-fingerprint-b', '00000000-0000-7000-8000-000000000202', 'active', 1, 10000, 1000, 1);
INSERT INTO prodex_approval_votes VALUES
  ('${tenantA}', 'approval-a', '00000000-0000-7000-8000-000000000201', 1000),
  ('${tenantB}', 'approval-b', '00000000-0000-7000-8000-000000000202', 1000);
INSERT INTO prodex_classification_rule_revisions VALUES
  ('${tenantA}', 'classification-1', 'classification-checksum-a', '{}', 'active', 1000),
  ('${tenantB}', 'classification-1', 'classification-checksum-b', '{}', 'active', 1000);
INSERT INTO prodex_provider_registry_revisions VALUES
  ('${tenantA}', 'registry-1', 'registry-checksum-a', 'active', 1000),
  ('${tenantB}', 'registry-1', 'registry-checksum-b', 'active', 1000);
INSERT INTO prodex_pricing_revisions VALUES
  ('${tenantA}', 'pricing-1', 'pricing-checksum-a', '{}', 'active', 1000),
  ('${tenantB}', 'pricing-1', 'pricing-checksum-b', '{}', 'active', 1000);
INSERT INTO prodex_provider_descriptors VALUES
  ('${tenantA}', 'registry-1', 'provider-a', 'openai', 'active', 'trusted', 'hosted', ARRAY['region-a'], '{}', '{}', 0, 'pricing-1', 'projected', 'credential-a', '1'),
  ('${tenantB}', 'registry-1', 'provider-b', 'openai', 'active', 'trusted', 'hosted', ARRAY['region-b'], '{}', '{}', 0, 'pricing-1', 'projected', 'credential-b', '1');
INSERT INTO prodex_routing_score_revisions VALUES
  ('${tenantA}', 'routing-1', 'routing-checksum-a', '{}', 'active', 1000),
  ('${tenantB}', 'routing-1', 'routing-checksum-b', '{}', 'active', 1000);
INSERT INTO prodex_governance_sessions VALUES
  ('${tenantA}', 'session-hash-a', '00000000-0000-7000-8000-000000000201', 'api', 'inference', 'internal', '00000000-0000-7000-8000-000000000901', 'registry-1', 'provider-a', 1000, 1500, 10000, 5000),
  ('${tenantB}', 'session-hash-b', '00000000-0000-7000-8000-000000000202', 'api', 'inference', 'internal', '00000000-0000-7000-8000-000000000902', 'registry-1', 'provider-b', 1000, 1500, 10000, 5000);
INSERT INTO prodex_session_revocations VALUES
  ('${tenantA}', 'session-hash-a', 1600, 'logout'),
  ('${tenantB}', 'session-hash-b', 1600, 'logout');
UPDATE prodex_tenants SET session_revocation_epoch = 1;
INSERT INTO prodex_siem_outbox VALUES
  ('${tenantA}', '00000000-0000-7000-8000-000000000921', '00000000-0000-7000-8000-000000000801', '{}', 0, 2000, 1600, NULL),
  ('${tenantB}', '00000000-0000-7000-8000-000000000922', '00000000-0000-7000-8000-000000000802', '{}', 0, 2000, 1600, NULL);
INSERT INTO prodex_siem_dead_letters VALUES
  ('${tenantA}', '00000000-0000-7000-8000-000000000931', '00000000-0000-7000-8000-000000000801', '{}', 1, 'delivery_failed', 2000),
  ('${tenantB}', '00000000-0000-7000-8000-000000000932', '00000000-0000-7000-8000-000000000802', '{}', 1, 'delivery_failed', 2000);
INSERT INTO prodex_governance_revision_artifacts VALUES
  ('${tenantA}', 'policy', '00000000-0000-7000-8000-000000000901', 'policy-checksum-a', decode('01', 'hex'), '00000000-0000-7000-8000-000000000201', 1000),
  ('${tenantB}', 'policy', '00000000-0000-7000-8000-000000000902', 'policy-checksum-b', decode('02', 'hex'), '00000000-0000-7000-8000-000000000202', 1000);
INSERT INTO prodex_classification_rule_pointers VALUES
  ('${tenantA}', 'classification-1', 'classification-1', 'classification-etag-a', 1000),
  ('${tenantB}', 'classification-1', 'classification-1', 'classification-etag-b', 1000);
INSERT INTO prodex_provider_registry_pointers VALUES
  ('${tenantA}', 'registry-1', 'registry-1', 'registry-etag-a', 1000),
  ('${tenantB}', 'registry-1', 'registry-1', 'registry-etag-b', 1000);
INSERT INTO prodex_routing_score_pointers VALUES
  ('${tenantA}', 'routing-1', 'routing-1', 'routing-etag-a', 1000),
  ('${tenantB}', 'routing-1', 'routing-1', 'routing-etag-b', 1000);
INSERT INTO prodex_governance_activation_history VALUES
  ('${tenantA}', '00000000-0000-7000-8000-000000000941', 'classification_rules', 'classification-1', NULL, 'activate', '00000000-0000-7000-8000-000000000201', 'classification-activate-a', 1000),
  ('${tenantB}', '00000000-0000-7000-8000-000000000942', 'classification_rules', 'classification-1', NULL, 'activate', '00000000-0000-7000-8000-000000000202', 'classification-activate-b', 1000);
INSERT INTO prodex_governance_mutation_idempotency VALUES
  ('${tenantA}', 'policy', 'policy-activate-a', 'request-a', 'activate', '00000000-0000-7000-8000-000000000901', 'policy-etag-a', 1000),
  ('${tenantB}', 'policy', 'policy-activate-b', 'request-b', 'activate', '00000000-0000-7000-8000-000000000902', 'policy-etag-b', 1000);
`;

const fingerprintSql = String.raw`
SELECT json_build_object(
  'tenants', (SELECT COUNT(*) FROM prodex_tenants),
  'virtual_keys', (SELECT COUNT(*) FROM prodex_virtual_keys),
  'service_identities', (SELECT COUNT(*) FROM prodex_service_identities),
  'users', (SELECT COUNT(*) FROM prodex_users),
  'role_bindings', (SELECT COUNT(*) FROM prodex_role_bindings),
  'provider_credentials', (SELECT COUNT(*) FROM prodex_provider_credentials),
  'budget_counters', (SELECT COUNT(*) FROM prodex_budget_counters),
  'budget_policies', (SELECT COUNT(*) FROM prodex_budget_policies),
  'reservations', (SELECT COUNT(*) FROM prodex_reservations),
  'ledger_rows', (SELECT COUNT(*) FROM prodex_usage_ledger),
  'audit_rows', (SELECT COUNT(*) FROM prodex_audit_log),
  'idempotency_rows', (SELECT COUNT(*) FROM prodex_idempotency_records),
  'policy_revisions', (SELECT COUNT(*) FROM prodex_policy_revisions),
  'policy_pointers', (SELECT COUNT(*) FROM prodex_policy_pointers),
  'policy_activations', (SELECT COUNT(*) FROM prodex_policy_activation_history),
  'approvals', (SELECT COUNT(*) FROM prodex_approvals),
  'approval_votes', (SELECT COUNT(*) FROM prodex_approval_votes),
  'classification_revisions', (SELECT COUNT(*) FROM prodex_classification_rule_revisions),
  'provider_registry_revisions', (SELECT COUNT(*) FROM prodex_provider_registry_revisions),
  'pricing_revisions', (SELECT COUNT(*) FROM prodex_pricing_revisions),
  'provider_descriptors', (SELECT COUNT(*) FROM prodex_provider_descriptors),
  'routing_score_revisions', (SELECT COUNT(*) FROM prodex_routing_score_revisions),
  'governance_sessions', (SELECT COUNT(*) FROM prodex_governance_sessions),
  'session_revocations', (SELECT COUNT(*) FROM prodex_session_revocations),
  'session_revocation_epochs', (SELECT SUM(session_revocation_epoch) FROM prodex_tenants),
  'siem_outbox_rows', (SELECT COUNT(*) FROM prodex_siem_outbox),
  'siem_dead_letter_rows', (SELECT COUNT(*) FROM prodex_siem_dead_letters),
  'governance_artifacts', (SELECT COUNT(*) FROM prodex_governance_revision_artifacts),
  'classification_pointers', (SELECT COUNT(*) FROM prodex_classification_rule_pointers),
  'provider_registry_pointers', (SELECT COUNT(*) FROM prodex_provider_registry_pointers),
  'routing_score_pointers', (SELECT COUNT(*) FROM prodex_routing_score_pointers),
  'governance_activations', (SELECT COUNT(*) FROM prodex_governance_activation_history),
  'governance_idempotency_rows', (SELECT COUNT(*) FROM prodex_governance_mutation_idempotency),
  'committed_tokens', (SELECT SUM(committed_tokens) FROM prodex_budget_counters),
  'ledger_tokens', (SELECT SUM(tokens) FROM prodex_usage_ledger),
  'unique_ledger_rows', (SELECT COUNT(DISTINCT (tenant_id, reservation_id, event_kind)) FROM prodex_usage_ledger),
  'policy_pointer_links', (
    SELECT COUNT(*) FROM prodex_policy_pointers pointer
    JOIN prodex_policy_revisions revision
      ON revision.tenant_id = pointer.tenant_id
     AND revision.revision_id = pointer.active_revision_id
  ),
  'provider_pricing_links', (
    SELECT COUNT(*) FROM prodex_provider_descriptors provider
    JOIN prodex_pricing_revisions pricing
      ON pricing.tenant_id = provider.tenant_id
     AND pricing.revision_id = provider.pricing_revision
  ),
  'session_revision_links', (
    SELECT COUNT(*) FROM prodex_governance_sessions session
    JOIN prodex_policy_revisions policy
      ON policy.tenant_id = session.tenant_id
     AND policy.revision_id = session.policy_revision_id
    JOIN prodex_provider_registry_revisions registry
      ON registry.tenant_id = session.tenant_id
     AND registry.revision_id = session.provider_registry_revision
  ),
  'session_revocation_links', (
    SELECT COUNT(*) FROM prodex_session_revocations revocation
    JOIN prodex_governance_sessions session
      USING (tenant_id, session_id_hash)
  ),
  'siem_audit_links', (
    (SELECT COUNT(*) FROM prodex_siem_outbox outbox
      JOIN prodex_audit_log audit USING (tenant_id, audit_event_id))
    +
    (SELECT COUNT(*) FROM prodex_siem_dead_letters dead_letter
      JOIN prodex_audit_log audit USING (tenant_id, audit_event_id))
  ),
  'audit_chain_valid_rows', (
    SELECT COUNT(*) FROM prodex_audit_log audit
    WHERE audit.previous_digest IS NULL OR EXISTS (
      SELECT 1 FROM prodex_audit_log predecessor
      WHERE predecessor.tenant_id = audit.tenant_id
        AND predecessor.event_digest = audit.previous_digest
    )
  ),
  'governance_artifact_links', (
    SELECT COUNT(*) FROM prodex_governance_revision_artifacts artifact
    JOIN prodex_policy_revisions policy
      ON policy.tenant_id = artifact.tenant_id
     AND policy.revision_id::text = artifact.revision_id
     AND policy.artifact_checksum = artifact.artifact_checksum
    WHERE artifact.artifact_kind = 'policy'
  ),
  'governance_pointer_links', (
    (SELECT COUNT(*) FROM prodex_classification_rule_pointers pointer
      JOIN prodex_classification_rule_revisions revision
        ON revision.tenant_id = pointer.tenant_id
       AND revision.revision_id = pointer.active_revision_id)
    +
    (SELECT COUNT(*) FROM prodex_provider_registry_pointers pointer
      JOIN prodex_provider_registry_revisions revision
        ON revision.tenant_id = pointer.tenant_id
       AND revision.revision_id = pointer.active_revision_id)
    +
    (SELECT COUNT(*) FROM prodex_routing_score_pointers pointer
      JOIN prodex_routing_score_revisions revision
        ON revision.tenant_id = pointer.tenant_id
       AND revision.revision_id = pointer.active_revision_id)
  )
)::text;
`;

const postBackupWriteSql = String.raw`
INSERT INTO prodex_audit_log VALUES
  ('${tenantA}', '00000000-0000-7000-8000-000000000899', 'digest-a', 'post-backup-marker', 2000, '00000000-0000-7000-8000-000000000101', 'drill.post_backup', 'tenant', 'tenant-a', 'success', NULL);
`;

const rlsSetupSql = String.raw`
DO $role$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'prodex_drill_reader') THEN
    CREATE ROLE prodex_drill_reader NOLOGIN NOSUPERUSER NOBYPASSRLS;
  END IF;
END $role$;
ALTER ROLE prodex_drill_reader NOLOGIN NOSUPERUSER NOBYPASSRLS;
GRANT USAGE ON SCHEMA public TO prodex_drill_reader;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO prodex_drill_reader;
`;

const rlsCheckSql = String.raw`
SET ROLE prodex_drill_reader;
SET prodex.tenant_id = '${tenantA}';
DO $check$
DECLARE
  tenant_table TEXT;
  visible_rows BIGINT;
  changed_rows BIGINT;
BEGIN
  FOREACH tenant_table IN ARRAY ARRAY[
    'prodex_tenants', 'prodex_virtual_keys', 'prodex_service_identities',
    'prodex_users', 'prodex_role_bindings', 'prodex_provider_credentials',
    'prodex_budget_counters', 'prodex_budget_policies', 'prodex_reservations',
    'prodex_usage_ledger', 'prodex_audit_log', 'prodex_idempotency_records',
    'prodex_policy_revisions', 'prodex_policy_pointers',
    'prodex_policy_activation_history', 'prodex_approvals', 'prodex_approval_votes',
    'prodex_classification_rule_revisions', 'prodex_provider_registry_revisions',
    'prodex_pricing_revisions', 'prodex_provider_descriptors',
    'prodex_routing_score_revisions', 'prodex_governance_sessions',
    'prodex_session_revocations', 'prodex_siem_outbox', 'prodex_siem_dead_letters',
    'prodex_governance_revision_artifacts', 'prodex_classification_rule_pointers',
    'prodex_provider_registry_pointers', 'prodex_routing_score_pointers',
    'prodex_governance_activation_history', 'prodex_governance_mutation_idempotency'
  ] LOOP
    EXECUTE format('SELECT COUNT(*) FROM %I', tenant_table) INTO visible_rows;
    IF visible_rows <> 1 THEN
      RAISE EXCEPTION 'tenant isolation failed for %', tenant_table;
    END IF;
    EXECUTE format('SELECT COUNT(*) FROM %I WHERE tenant_id = $1', tenant_table)
      INTO visible_rows USING '${tenantB}'::uuid;
    IF visible_rows <> 0 THEN
      RAISE EXCEPTION 'cross-tenant read succeeded for %', tenant_table;
    END IF;
  END LOOP;

  UPDATE prodex_users SET display_name = 'blocked' WHERE tenant_id = '${tenantB}';
  GET DIAGNOSTICS changed_rows = ROW_COUNT;
  IF changed_rows <> 0 THEN RAISE EXCEPTION 'cross-tenant update succeeded'; END IF;
  DELETE FROM prodex_users WHERE tenant_id = '${tenantB}';
  GET DIAGNOSTICS changed_rows = ROW_COUNT;
  IF changed_rows <> 0 THEN RAISE EXCEPTION 'cross-tenant delete succeeded'; END IF;
  BEGIN
    INSERT INTO prodex_users VALUES (
      '${tenantB}', '00000000-0000-7000-8000-000000000299',
      'blocked-user', 'Blocked User', 2000, 2000, NULL
    );
    RAISE EXCEPTION 'cross-tenant insert succeeded';
  EXCEPTION WHEN insufficient_privilege THEN
    NULL;
  END;
END $check$;
SELECT json_build_object(
  'tenant_tables_checked', 32,
  'cross_tenant_reads_blocked', true,
  'cross_tenant_writes_blocked', true,
  'forced_governance_tables', (
    SELECT COUNT(*) FROM pg_class
    WHERE relnamespace = current_schema()::regnamespace
      AND relname = ANY(ARRAY[
        'prodex_policy_revisions', 'prodex_policy_pointers',
        'prodex_policy_activation_history', 'prodex_approvals', 'prodex_approval_votes',
        'prodex_classification_rule_revisions', 'prodex_provider_registry_revisions',
        'prodex_pricing_revisions', 'prodex_provider_descriptors',
        'prodex_routing_score_revisions', 'prodex_governance_sessions',
        'prodex_session_revocations', 'prodex_siem_outbox', 'prodex_siem_dead_letters',
        'prodex_governance_revision_artifacts', 'prodex_classification_rule_pointers',
        'prodex_provider_registry_pointers', 'prodex_routing_score_pointers',
        'prodex_governance_activation_history', 'prodex_governance_mutation_idempotency'
      ])
      AND relforcerowsecurity
  )
)::text;
`;

function run(command, args, options = {}) {
  return new Promise((resolve, reject) => {
    const hasInput = options.input !== undefined;
    const child = spawn(command, args, {
      cwd: repoRoot,
      env: { ...process.env, ...(options.env ?? {}) },
      stdio: [
        hasInput ? "pipe" : "ignore",
        options.capture ? "pipe" : "inherit",
        options.capture ? "pipe" : "inherit",
      ],
    });
    let stdout = "";
    let stderr = "";
    if (options.capture) {
      child.stdout.setEncoding("utf8");
      child.stderr.setEncoding("utf8");
      child.stdout.on("data", (chunk) => {
        stdout += chunk;
      });
      child.stderr.on("data", (chunk) => {
        stderr += chunk;
      });
    }
    if (hasInput) {
      child.stdin.on("error", (error) => {
        if (error.code !== "EPIPE") reject(error);
      });
      child.stdin.end(options.input);
    }
    child.on("error", reject);
    child.on("close", (code, signal) => {
      if (signal) {
        reject(new Error(`${command} exited with signal ${signal}`));
      } else if (code !== 0) {
        reject(new Error(`${command} exited with code ${code}${stderr.trim() ? `: ${stderr.trim()}` : ""}`));
      } else {
        resolve({ stdout, stderr });
      }
    });
  });
}

export function parseThreshold(value, fallback, name) {
  const parsed = Number(value ?? fallback);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    throw new Error(`${name} must be a positive number`);
  }
  return parsed;
}

export function assessDrill({
  rpoSeconds,
  rtoSeconds,
  maxRpoSeconds,
  maxRtoSeconds,
  fingerprintsMatch,
  postBackupMarkerAbsent,
  accountingMatches,
  tenantDataComplete,
  governanceReferencesValid,
  rlsIsolated,
}) {
  const failures = [];
  if (rpoSeconds > maxRpoSeconds) failures.push("rpo_exceeded");
  if (rtoSeconds > maxRtoSeconds) failures.push("rto_exceeded");
  if (!fingerprintsMatch) failures.push("fingerprint_mismatch");
  if (!postBackupMarkerAbsent) failures.push("post_backup_marker_restored");
  if (!accountingMatches) failures.push("accounting_mismatch");
  if (!tenantDataComplete) failures.push("tenant_data_incomplete");
  if (!governanceReferencesValid) failures.push("governance_reference_invalid");
  if (!rlsIsolated) failures.push("tenant_isolation_failed");
  return { passed: failures.length === 0, failures };
}

async function dockerPsql(containerId, database, sql) {
  const { stdout } = await run(
    "docker",
    ["exec", "-i", containerId, "psql", "-qAt", "-v", "ON_ERROR_STOP=1", "-U", "postgres", "-d", database],
    { capture: true, input: sql },
  );
  return stdout.trim();
}

async function waitForPostgres(containerId) {
  for (let attempt = 0; attempt < 60; attempt += 1) {
    try {
      await run(
        "docker",
        ["exec", containerId, "pg_isready", "-U", "postgres", "-d", sourceDatabase],
        { capture: true },
      );
      return;
    } catch {
      await new Promise((resolve) => setTimeout(resolve, 1000));
    }
  }
  throw new Error("temporary Postgres container did not become ready");
}

async function waitForPublishedPostgres(port) {
  for (let attempt = 0; attempt < 60; attempt += 1) {
    try {
      await run(
        "psql",
        ["-h", "127.0.0.1", "-p", String(port), "-U", "postgres", "-d", sourceDatabase, "-c", "select 1"],
        { capture: true, env: { PGPASSWORD: "postgres" } },
      );
      return;
    } catch {
      await new Promise((resolve) => setTimeout(resolve, 1000));
    }
  }
  throw new Error("temporary published Postgres endpoint did not become ready");
}

async function sha256File(filePath) {
  const hash = createHash("sha256");
  for await (const chunk of createReadStream(filePath)) hash.update(chunk);
  return hash.digest("hex");
}

export async function encryptBackupArtifact(sourcePath, encryptedPath, key) {
  if (!Buffer.isBuffer(key) || key.length !== 32) {
    throw new Error("backup encryption key must contain 32 bytes");
  }
  const nonce = randomBytes(12);
  const plaintext = await fs.readFile(sourcePath);
  try {
    const cipher = createCipheriv("aes-256-gcm", key, nonce);
    cipher.setAAD(backupArtifactMagic);
    const ciphertext = Buffer.concat([cipher.update(plaintext), cipher.final()]);
    const tag = cipher.getAuthTag();
    await fs.writeFile(
      encryptedPath,
      Buffer.concat([backupArtifactMagic, nonce, tag, ciphertext]),
      { mode: 0o600 },
    );
  } finally {
    plaintext.fill(0);
  }
}

export async function decryptBackupArtifact(encryptedPath, destinationPath, key) {
  if (!Buffer.isBuffer(key) || key.length !== 32) {
    throw new Error("backup encryption key must contain 32 bytes");
  }
  const artifact = await fs.readFile(encryptedPath);
  const headerBytes = backupArtifactMagic.length + 12 + 16;
  if (
    artifact.length < headerBytes ||
    !artifact.subarray(0, backupArtifactMagic.length).equals(backupArtifactMagic)
  ) {
    throw new Error("backup artifact envelope is invalid");
  }
  const nonceStart = backupArtifactMagic.length;
  const tagStart = nonceStart + 12;
  const ciphertextStart = tagStart + 16;
  const decipher = createDecipheriv(
    "aes-256-gcm",
    key,
    artifact.subarray(nonceStart, tagStart),
  );
  decipher.setAAD(backupArtifactMagic);
  decipher.setAuthTag(artifact.subarray(tagStart, ciphertextStart));
  const plaintext = Buffer.concat([
    decipher.update(artifact.subarray(ciphertextStart)),
    decipher.final(),
  ]);
  try {
    await fs.writeFile(destinationPath, plaintext, { mode: 0o600 });
  } finally {
    plaintext.fill(0);
    artifact.fill(0);
  }
}

async function writeEvidence(evidence) {
  await fs.mkdir(path.dirname(evidencePath), { recursive: true });
  const temporaryPath = `${evidencePath}.${process.pid}.tmp`;
  await fs.writeFile(temporaryPath, `${JSON.stringify(evidence, null, 2)}\n`, { mode: 0o600 });
  await fs.rename(temporaryPath, evidencePath);
}

async function revision() {
  const { stdout } = await run("git", ["rev-parse", "HEAD"], { capture: true });
  return stdout.trim();
}

async function runManagedDrill() {
  const maxRpoSeconds = parseThreshold(
    process.env.PRODEX_BACKUP_DRILL_MAX_RPO_SECONDS,
    60,
    "PRODEX_BACKUP_DRILL_MAX_RPO_SECONDS",
  );
  const maxRtoSeconds = parseThreshold(
    process.env.PRODEX_BACKUP_DRILL_MAX_RTO_SECONDS,
    300,
    "PRODEX_BACKUP_DRILL_MAX_RTO_SECONDS",
  );
  const workDir = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-backup-drill-"));
  const dumpPath = path.join(workDir, "prodex.dump");
  const encryptedDumpPath = path.join(workDir, "prodex.dump.aes256gcm");
  const restoreDumpPath = path.join(workDir, "prodex.restore.dump");
  const backupKey = randomBytes(32);
  const containerName = `prodex-backup-drill-${process.pid}-${Date.now()}`;
  const evidence = {
    schema_version: 1,
    backend: "postgres",
    result: "failed",
    source_revision: "unavailable",
    thresholds: {
      max_rpo_seconds: maxRpoSeconds,
      max_rto_seconds: maxRtoSeconds,
    },
  };
  let containerId = null;

  try {
    evidence.source_revision = await revision();
    const { stdout } = await run(
      "docker",
      [
        "run", "-d", "--rm", "--name", containerName,
        "-e", "POSTGRES_PASSWORD=postgres",
        "-e", `POSTGRES_DB=${sourceDatabase}`,
        "-P", postgresImage,
      ],
      { capture: true },
    );
    containerId = stdout.trim();
    await waitForPostgres(containerId);
    const { stdout: portOutput } = await run(
      "docker",
      ["port", containerId, "5432/tcp"],
      { capture: true },
    );
    const port = Number.parseInt(portOutput.trim().split(":").at(-1) ?? "", 10);
    if (!Number.isInteger(port) || port <= 0) throw new Error("temporary Postgres port is invalid");
    await waitForPublishedPostgres(port);
    const postgresUrl = `postgres://postgres:postgres@127.0.0.1:${port}/${sourceDatabase}`;

    await run(
      "cargo",
      ["run", "--quiet", "--bin", "prodex-gateway", "--", "migrate", "--backend", "postgres", "--url-env", "PRODEX_GATEWAY_POSTGRES_URL", "--tls-mode", "disable"],
      { capture: true, env: { PRODEX_GATEWAY_POSTGRES_URL: postgresUrl } },
    );
    await dockerPsql(containerId, sourceDatabase, seedSql);
    const sourceFingerprint = JSON.parse(await dockerPsql(containerId, sourceDatabase, fingerprintSql));

    const recoveryPointAt = new Date();
    await run(
      "docker",
      ["exec", containerId, "pg_dump", "--format=custom", "--no-owner", "--no-privileges", "-U", "postgres", "-d", sourceDatabase, "-f", "/tmp/prodex.dump"],
      { capture: true },
    );
    await run("docker", ["cp", `${containerId}:/tmp/prodex.dump`, dumpPath], { capture: true });
    await encryptBackupArtifact(dumpPath, encryptedDumpPath, backupKey);
    await fs.rm(dumpPath, { force: true });
    await run("docker", ["exec", containerId, "rm", "-f", "/tmp/prodex.dump"], { capture: true });
    const artifactSha256 = await sha256File(encryptedDumpPath);
    const artifactBytes = (await fs.stat(encryptedDumpPath)).size;
    await decryptBackupArtifact(encryptedDumpPath, restoreDumpPath, backupKey);
    await run(
      "docker",
      ["cp", restoreDumpPath, `${containerId}:/tmp/prodex.restore.dump`],
      { capture: true },
    );
    await fs.rm(restoreDumpPath, { force: true });
    await dockerPsql(containerId, sourceDatabase, postBackupWriteSql);
    await run(
      "docker",
      ["exec", containerId, "createdb", "-U", "postgres", restoreDatabase],
      { capture: true },
    );

    const restoreStarted = performance.now();
    await run(
      "docker",
      ["exec", containerId, "pg_restore", "--exit-on-error", "--no-owner", "--no-privileges", "-U", "postgres", "-d", restoreDatabase, "/tmp/prodex.restore.dump"],
      { capture: true },
    );
    await run("docker", ["exec", containerId, "rm", "-f", "/tmp/prodex.restore.dump"], { capture: true });
    const restoredFingerprint = JSON.parse(await dockerPsql(containerId, restoreDatabase, fingerprintSql));
    const markerCount = Number(await dockerPsql(
      containerId,
      restoreDatabase,
      "SELECT COUNT(*) FROM prodex_audit_log WHERE event_digest = 'post-backup-marker';",
    ));
    await dockerPsql(containerId, restoreDatabase, rlsSetupSql);
    const rls = JSON.parse(await dockerPsql(containerId, restoreDatabase, rlsCheckSql));
    const restoredAt = new Date();
    const rtoSeconds = (performance.now() - restoreStarted) / 1000;
    const rpoSeconds = (restoredAt.getTime() - recoveryPointAt.getTime()) / 1000;
    const fingerprintsMatch = JSON.stringify(sourceFingerprint) === JSON.stringify(restoredFingerprint);
    const accountingMatches =
      restoredFingerprint.committed_tokens === 40 &&
      restoredFingerprint.ledger_tokens === 40 &&
      restoredFingerprint.ledger_rows === restoredFingerprint.unique_ledger_rows;
    const tenantDataComplete = [
      "tenants", "virtual_keys", "service_identities", "users", "role_bindings",
      "provider_credentials", "budget_counters", "budget_policies", "reservations",
      "ledger_rows", "audit_rows", "idempotency_rows", "policy_revisions",
      "policy_pointers", "policy_activations", "approvals", "approval_votes",
      "classification_revisions", "provider_registry_revisions", "pricing_revisions",
      "provider_descriptors", "routing_score_revisions", "governance_sessions",
      "session_revocations", "siem_outbox_rows", "siem_dead_letter_rows",
      "governance_artifacts", "classification_pointers", "provider_registry_pointers",
      "routing_score_pointers", "governance_activations", "governance_idempotency_rows",
    ].every((field) => restoredFingerprint[field] === 2);
    const governanceReferencesValid =
      restoredFingerprint.policy_pointer_links === 2 &&
      restoredFingerprint.provider_pricing_links === 2 &&
      restoredFingerprint.session_revision_links === 2 &&
      restoredFingerprint.session_revocation_links === 2 &&
      restoredFingerprint.session_revocation_epochs === 2 &&
      restoredFingerprint.siem_audit_links === 4 &&
      restoredFingerprint.audit_chain_valid_rows === restoredFingerprint.audit_rows &&
      restoredFingerprint.governance_artifact_links === 2 &&
      restoredFingerprint.governance_pointer_links === 6;
    const rlsIsolated =
      rls.tenant_tables_checked === 32 &&
      rls.forced_governance_tables === 20 &&
      rls.cross_tenant_reads_blocked === true &&
      rls.cross_tenant_writes_blocked === true;
    const assessment = assessDrill({
      rpoSeconds,
      rtoSeconds,
      maxRpoSeconds,
      maxRtoSeconds,
      fingerprintsMatch,
      postBackupMarkerAbsent: markerCount === 0,
      accountingMatches,
      tenantDataComplete,
      governanceReferencesValid,
      rlsIsolated,
    });

    Object.assign(evidence, {
      result: assessment.passed ? "passed" : "failed",
      failure_codes: assessment.failures,
      recovery_point_at: recoveryPointAt.toISOString(),
      restored_at: restoredAt.toISOString(),
      rpo_seconds: Number(rpoSeconds.toFixed(3)),
      rto_seconds: Number(rtoSeconds.toFixed(3)),
      artifact_sha256: artifactSha256,
      artifact_bytes: artifactBytes,
      artifact_encryption: "aes-256-gcm",
      integrity: {
        fingerprints_match: fingerprintsMatch,
        post_backup_marker_absent: markerCount === 0,
        accounting_matches: accountingMatches,
        tenant_data_complete: tenantDataComplete,
        governance_references_valid: governanceReferencesValid,
        tenant_isolation_enforced: rlsIsolated,
        tenant_tables_checked: rls.tenant_tables_checked,
        forced_governance_tables: rls.forced_governance_tables,
        tenant_rows: restoredFingerprint.tenants,
        ledger_rows: restoredFingerprint.ledger_rows,
        audit_rows: restoredFingerprint.audit_rows,
      },
    });
    await writeEvidence(evidence);
    if (!assessment.passed) {
      throw new Error(`backup/restore drill failed: ${assessment.failures.join(",")}`);
    }
    process.stdout.write(`${JSON.stringify(evidence, null, 2)}\n`);
  } catch (error) {
    evidence.failure_codes ??= ["drill_execution_failed"];
    await writeEvidence(evidence);
    throw error;
  } finally {
    backupKey.fill(0);
    if (containerId) {
      try {
        await run("docker", ["rm", "-f", containerId], { capture: true });
      } catch {}
    }
    await fs.rm(workDir, { recursive: true, force: true });
  }
}

async function runSelfTest() {
  if (parseThreshold("60", 1, "rpo") !== 60) throw new Error("threshold parsing failed");
  const assessment = assessDrill({
    rpoSeconds: 1,
    rtoSeconds: 2,
    maxRpoSeconds: 3,
    maxRtoSeconds: 4,
    fingerprintsMatch: true,
    postBackupMarkerAbsent: true,
    accountingMatches: true,
    tenantDataComplete: true,
    governanceReferencesValid: true,
    rlsIsolated: true,
  });
  if (!assessment.passed) throw new Error("passing drill assessment was rejected");
  if (!(await revision())) throw new Error("source revision lookup failed");
}

export async function main() {
  if (process.argv.includes("--self-test")) {
    await runSelfTest();
    return;
  }
  await runManagedDrill();
}

if (process.argv[1] && path.resolve(process.argv[1]) === modulePath) {
  main().catch((error) => {
    process.stderr.write(`backup-restore-drill: ${error.stack ?? error.message}\n`);
    process.exitCode = 1;
  });
}
