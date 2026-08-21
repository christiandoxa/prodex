#!/usr/bin/env node

import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import { repoRoot } from "../npm/common.mjs";

const PROMOTED_FILES = [
  "crates/prodex-mojo-core/build.rs",
  "crates/prodex-mojo-core/src/lib.rs",
  "crates/prodex-mojo-core/src/quota.rs",
  "crates/prodex-mojo-core/src/routing.rs",
  "crates/prodex-mojo-core/src/runtime.rs",
  "crates/prodex-mojo-core/src/runtime_decisions.rs",
  "crates/prodex-mojo-core/src/provider_constraints.rs",
  "crates/prodex-mojo-core/src/policy.rs",
  "crates/prodex-mojo-core/src/context.rs",
];

const FORBIDDEN_MARKERS = [
  "prodex_mojo_fallback",
  "use_rust_fallback",
  "rust_fallback",
  "fallback-to-rust",
];

export function findViolations(files) {
  return files.flatMap(([filePath, contents]) =>
    FORBIDDEN_MARKERS.filter((marker) => contents.includes(marker)).map(
      (marker) => `${filePath}: promoted Mojo code contains ${marker}`,
    ),
  );
}

async function promotedFiles() {
  return Promise.all(
    PROMOTED_FILES.map(async (filePath) => [
      filePath,
      await fs.readFile(path.join(repoRoot, filePath), "utf8"),
    ]),
  );
}

function selfTest() {
  assert.deepEqual(findViolations([["x.rs", "fn main() {}"]]), []);
  assert.equal(findViolations([["x.rs", "prodex_mojo_fallback();"]]).length, 1);
}

async function main() {
  if (process.argv.includes("--self-test")) selfTest();
  const violations = findViolations(await promotedFiles());
  if (violations.length > 0) throw new Error(violations.join("\n"));
  process.stdout.write("mojo no-fallback guard: ok\n");
}

main().catch((error) => {
  process.stderr.write(`mojo no-fallback guard: ${error.message}\n`);
  process.exitCode = 1;
});
