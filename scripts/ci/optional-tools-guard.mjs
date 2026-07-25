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
      for (const marker of ["include_str!", "include_bytes!", "git clone", "Command::new(\"git\")", "reqwest::", "ureq::", "curl ", "wget ", "npm install"]) {
        if (contents.includes(marker)) {
          violations.push(`${filePath}: forbidden optional-tool coupling: ${marker}`);
        }
      }
      if (
        filePath.endsWith("/launch_home.rs") &&
        ["Local::now", "Utc::now", "last_updated"].some((marker) => contents.includes(marker))
      ) {
        violations.push(`${filePath}: generated overlay configuration contains a launch timestamp`);
      }
    }
    if (filePath.startsWith("crates/prodex-app/src/") && contents.includes(LEGACY_RUNTIME_MODULE)) {
      violations.push(`${filePath}: generic runtime launch remains Caveman-named`);
    }
    if (filePath === "crates/prodex-app/src/runtime_tools.rs") {
      const production = contents.split("#[cfg(test)]", 1)[0];
      for (const marker of ["trust_level", "--dangerously-bypass-hook-trust"]) {
        if (production.includes(marker)) {
          violations.push(`${filePath}: optional-tool selection implies ${marker}`);
        }
      }
    }
    if (filePath === "crates/prodex-cli/src/runtime_args/optional_tools.rs") {
      const start = contents.indexOf("fn extract_super_leading_launch_prefixes");
      const end = contents.indexOf("impl fmt::Debug", start);
      const extractor = start >= 0 && end > start ? contents.slice(start, end) : "";
      if (!extractor.includes("_ => break") || !extractor.includes("skip(consumed)") || extractor.includes("retain(")) {
        violations.push(`${filePath}: legacy tool compatibility must consume leading typed prefixes only`);
      }
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
  assert.equal(
    validateFiles([["crates/prodex-optional-tools/src/install.rs", 'Command::new("git").arg("clone")']]).length,
    1,
  );
  assert.equal(
    validateFiles([["crates/prodex-optional-tools/src/launch_home.rs", "last_updated = Utc::now()"]]).length,
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
