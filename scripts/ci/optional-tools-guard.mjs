#!/usr/bin/env node
import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import { git, normalizeGitPath } from "./guard-common.mjs";
import { repoRoot } from "../npm/common.mjs";

const REMOVED_PACKAGE = ["prodex", "caveman", "assets"].join("-");
const EMBEDDED_INSTRUCTION = ["CAVEMAN", "MODE", "ACTIVE"].join(" ");
const LEGACY_RUNTIME_MODULE = ["runtime", "caveman"].join("_");

export function validateFiles(files) {
  const violations = [];
  for (const [filePath, contents] of files) {
    if (/Cargo\.(?:toml|lock)$/u.test(filePath) && contents.includes(REMOVED_PACKAGE)) {
      violations.push(`${filePath}: removed Caveman asset package remains in the Cargo graph`);
    }
    if (/\.(?:rs|mjs|js)$/u.test(filePath) && contents.includes(EMBEDDED_INSTRUCTION)) {
      violations.push(`${filePath}: embedded Caveman developer instruction remains`);
    }
    if (filePath.startsWith("crates/prodex-optional-tools/src/")) {
      for (const marker of ["include_str!", "include_bytes!", "Local::now", "last_updated", "git clone"]) {
        if (contents.includes(marker) && contents.toLowerCase().includes("caveman")) {
          violations.push(`${filePath}: forbidden optional-tool coupling: ${marker}`);
        }
      }
    }
    if (filePath.startsWith("crates/prodex-app/src/") && contents.includes(LEGACY_RUNTIME_MODULE)) {
      violations.push(`${filePath}: generic runtime launch remains Caveman-named`);
    }
  }
  return violations;
}

async function repositoryFiles() {
  const listed = await git(["ls-files", "--cached", "--others", "--exclude-standard"], {
    cwd: repoRoot,
  });
  const files = [];
  for (const filePath of [...new Set(listed.stdout.split(/\r?\n/u).filter(Boolean).map(normalizeGitPath))].sort()) {
    if (!/\.(?:rs|mjs|js|toml|lock)$/u.test(filePath)) continue;
    try {
      files.push([filePath, await fs.readFile(path.join(repoRoot, filePath), "utf8")]);
    } catch (error) {
      if (error?.code !== "ENOENT") throw error;
    }
  }
  return files;
}

function selfTest() {
  assert.deepEqual(validateFiles([["Cargo.toml", "name = 'prodex-optional-tools'"]]), []);
  assert.equal(validateFiles([["Cargo.toml", `name = '${REMOVED_PACKAGE}'`]]).length, 1);
  assert.equal(validateFiles([["src/main.rs", `const X: &str = '${EMBEDDED_INSTRUCTION}';`]]).length, 1);
  assert.equal(
    validateFiles([["crates/prodex-app/src/lib.rs", `mod ${LEGACY_RUNTIME_MODULE};`]]).length,
    1,
  );
}

async function main() {
  if (process.argv.includes("--self-test")) selfTest();
  const violations = validateFiles(await repositoryFiles());
  if (violations.length > 0) {
    throw new Error(violations.join("\n"));
  }
  process.stdout.write("optional tools guard: ok\n");
}

main().catch((error) => {
  process.stderr.write(`optional-tools-guard: ${error.message}\n`);
  process.exitCode = 1;
});
