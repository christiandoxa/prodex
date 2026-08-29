import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import test from "node:test";
import { validateManifest } from "./mojo-authority-guard.mjs";

test("Mojo authority guard self-test passes independently of the worktree manifest", () => {
  const output = execFileSync(
    process.execPath,
    ["scripts/ci/mojo-authority-guard.mjs", "--self-test"],
    { encoding: "utf8" },
  );
  if (!output.includes("mojo authority guard: self-test ok")) {
    throw new Error(`unexpected authority guard output: ${output}`);
  }
});

test("authority guard rejects zero cleanup and duplicate reductions", () => {
  const operation = {
    name: "new_operation",
    introduced_in: "0.420.0",
    final_state: "authoritative",
    production_fallback: false,
    duplicate_production_owner: false,
    platform_fallback: false,
    rust_state_after: "adapter-only",
  };
  const reduction = {
    operation: operation.name,
    file: "crates/example.rs",
    symbol: "cleanup_symbol",
    cleanup_loc: 1,
  };
  const manifest = {
    baseline_sha: "2531c7a345f1607a18aa926e204b4d02cc322167",
    release_target: "0.420.0",
    authoritative_operations: [operation],
    rust_semantic_reductions: [reduction],
  };
  assert.equal(validateManifest(manifest, []), true);
  assert.throws(
    () => validateManifest({
      ...manifest,
      rust_semantic_reductions: [{ ...reduction, cleanup_loc: 0 }],
    }, []),
    /cleanup_loc must be positive/u,
  );
  assert.throws(
    () => validateManifest({
      ...manifest,
      rust_semantic_reductions: [reduction, { ...reduction }],
    }, []),
    /duplicate Rust cleanup record/u,
  );
});
