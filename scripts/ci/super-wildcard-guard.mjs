#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { git, normalizeGitPath } from "./guard-common.mjs";
import { repoRoot } from "../npm/common.mjs";

const SCAN_ROOT = "crates/prodex-app/src";
const SUPER_WILDCARD_LINE = "use super::*;";
const SUPER_WILDCARD_CAP = 99;

function parseArgs(argv) {
  const args = { json: false };
  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--json") {
      args.json = true;
      continue;
    }
    if (value === "--help" || value === "-h") {
      args.help = true;
      continue;
    }
    throw new Error(`unknown argument: ${value}`);
  }
  return args;
}

function printHelp() {
  process.stdout.write(
    [
      "Usage: node scripts/ci/super-wildcard-guard.mjs [--json]",
      "",
      `Fails when Rust lines exactly matching ${JSON.stringify(SUPER_WILDCARD_LINE)} under ${SCAN_ROOT}`,
      "exceed the ratchet cap.",
      "Lower the cap when explicit imports replace existing wildcard imports.",
    ].join("\n") + "\n",
  );
}

async function rustFiles() {
  const result = await git(
    ["ls-files", "--cached", "--others", "--exclude-standard", "--", SCAN_ROOT],
    { cwd: repoRoot },
  );
  return [
    ...new Set(
      result.stdout
        .split(/\r?\n/)
        .filter(Boolean)
        .map(normalizeGitPath)
        .filter((filePath) => filePath.endsWith(".rs")),
    ),
  ].sort();
}

async function readExistingRustFile(filePath) {
  try {
    return await fs.readFile(path.join(repoRoot, filePath), "utf8");
  } catch (error) {
    if (error?.code === "ENOENT") {
      return null;
    }
    throw error;
  }
}

function countSuperWildcardLines(contents) {
  let count = 0;
  for (const line of contents.split(/\r\n|\n|\r/)) {
    if (line === SUPER_WILDCARD_LINE) {
      count += 1;
    }
  }
  return count;
}

async function scan() {
  let count = 0;
  const files = [];
  for (const filePath of await rustFiles()) {
    const contents = await readExistingRustFile(filePath);
    if (contents === null) {
      continue;
    }
    const fileCount = countSuperWildcardLines(contents);
    if (fileCount === 0) {
      continue;
    }
    count += fileCount;
    files.push({ filePath, count: fileCount });
  }

  return {
    cap: SUPER_WILDCARD_CAP,
    count,
    exceeded: count > SUPER_WILDCARD_CAP,
    files,
    line: SUPER_WILDCARD_LINE,
    scanRoot: SCAN_ROOT,
  };
}

function printHuman(report) {
  const ratio = `count=${report.count} cap=${report.cap}`;
  if (!report.exceeded) {
    process.stdout.write(`super wildcard guard: ok (${ratio})\n`);
    return;
  }

  process.stderr.write(`super wildcard guard failed: ${ratio}\n`);
  process.stderr.write(
    `\nReplace new ${JSON.stringify(report.line)} imports under ${report.scanRoot} with explicit imports, or deliberately update the cap.\n`,
  );
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }

  const report = await scan();
  if (args.json) {
    process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
  } else {
    printHuman(report);
  }
  if (report.exceeded) {
    process.exitCode = 1;
  }
}

main().catch((error) => {
  process.stderr.write(`super-wildcard-guard: ${error.message}\n`);
  process.exitCode = 1;
});
