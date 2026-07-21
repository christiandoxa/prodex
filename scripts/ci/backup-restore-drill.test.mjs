import assert from "node:assert/strict";
import { randomBytes } from "node:crypto";
import fs from "node:fs/promises";
import { createServer } from "node:net";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import {
  assessDrill,
  decryptBackupArtifact,
  encryptBackupArtifact,
  parseThreshold,
  waitForPublishedPostgres,
} from "./backup-restore-drill.mjs";

test("parseThreshold accepts positive values and rejects invalid limits", () => {
  assert.equal(parseThreshold(undefined, 60, "rpo"), 60);
  assert.equal(parseThreshold("2.5", 60, "rpo"), 2.5);
  assert.throws(() => parseThreshold("0", 60, "rpo"), /positive number/u);
  assert.throws(() => parseThreshold("invalid", 60, "rpo"), /positive number/u);
});

test("assessDrill reports every recovery invariant failure", () => {
  assert.deepEqual(
    assessDrill({
      rpoSeconds: 61,
      rtoSeconds: 301,
      maxRpoSeconds: 60,
      maxRtoSeconds: 300,
      fingerprintsMatch: false,
      postBackupMarkerAbsent: false,
      accountingMatches: false,
      tenantDataComplete: false,
      governanceReferencesValid: false,
      rlsIsolated: false,
    }),
    {
      passed: false,
      failures: [
        "rpo_exceeded",
        "rto_exceeded",
        "fingerprint_mismatch",
        "post_backup_marker_restored",
        "accounting_mismatch",
        "tenant_data_incomplete",
        "governance_reference_invalid",
        "tenant_isolation_failed",
      ],
    },
  );
});

test("assessDrill accepts an intact restore within thresholds", () => {
  assert.deepEqual(
    assessDrill({
      rpoSeconds: 1,
      rtoSeconds: 2,
      maxRpoSeconds: 60,
      maxRtoSeconds: 300,
      fingerprintsMatch: true,
      postBackupMarkerAbsent: true,
      accountingMatches: true,
      tenantDataComplete: true,
      governanceReferencesValid: true,
      rlsIsolated: true,
    }),
    { passed: true, failures: [] },
  );
});

test("backup artifact encryption round-trips and rejects tampering", async () => {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-backup-envelope-test-"));
  const source = path.join(directory, "source.dump");
  const encrypted = path.join(directory, "backup.dump.aes256gcm");
  const restored = path.join(directory, "restored.dump");
  const key = randomBytes(32);
  try {
    await fs.writeFile(source, "synthetic pg_dump bytes", { mode: 0o600 });
    await encryptBackupArtifact(source, encrypted, key);
    assert.notDeepEqual(await fs.readFile(encrypted), await fs.readFile(source));
    await decryptBackupArtifact(encrypted, restored, key);
    assert.equal(await fs.readFile(restored, "utf8"), "synthetic pg_dump bytes");

    const tampered = await fs.readFile(encrypted);
    tampered[tampered.length - 1] ^= 1;
    await fs.writeFile(encrypted, tampered, { mode: 0o600 });
    await assert.rejects(
      decryptBackupArtifact(encrypted, restored, key),
      /authenticate data|unable to authenticate/u,
    );
  } finally {
    key.fill(0);
    await fs.rm(directory, { recursive: true, force: true });
  }
});

test("published Postgres readiness only requires a reachable TCP endpoint", async () => {
  const server = createServer();
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  try {
    const address = server.address();
    assert.ok(address && typeof address !== "string");
    await waitForPublishedPostgres(address.port);
  } finally {
    await new Promise((resolve, reject) => {
      server.close((error) => (error ? reject(error) : resolve()));
    });
  }
});

test("script self-test runs with stdin closed", async () => {
  const { execFile } = await import("node:child_process");
  const { promisify } = await import("node:util");
  await promisify(execFile)(process.execPath, [
    "scripts/ci/backup-restore-drill.mjs",
    "--self-test",
  ], {
    cwd: new URL("../..", import.meta.url),
  });
});
