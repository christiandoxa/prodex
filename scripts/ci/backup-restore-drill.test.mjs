import assert from "node:assert/strict";
import test from "node:test";
import { assessDrill, parseThreshold } from "./backup-restore-drill.mjs";

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
      rlsIsolated: true,
    }),
    { passed: true, failures: [] },
  );
});
