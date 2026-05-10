#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { git } from "./guard-common.mjs";
import { repoRoot } from "../npm/common.mjs";

const ALLOW_ATTRIBUTE_CAPS = Object.freeze({
  dead_code: 24,
  "unused_imports": 9,
  "clippy::large_enum_variant": 5,
  "clippy::result_large_err": 2,
  "clippy::type_complexity": 1,
});

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
      "Usage: node scripts/ci/allow-attribute-guard.mjs [--json]",
      "",
      "Fails when Rust #[allow(...)] counts exceed the checked-in ratchet caps.",
      "Lower caps when refactors remove compatibility shims or lint allowances.",
    ].join("\n") + "\n",
  );
}

async function rustFiles() {
  const result = await git(["ls-files", "--", "*.rs"], { cwd: repoRoot });
  return result.stdout.split(/\r?\n/).filter(Boolean).sort();
}

function countAllowAttributes(contents, counts) {
  const pattern = /#\s*\[\s*allow\s*\(([^)]*)\)\s*\]/g;
  for (const match of contents.matchAll(pattern)) {
    for (const rawName of match[1].split(",")) {
      const name = rawName.trim();
      if (!name) {
        continue;
      }
      counts.set(name, (counts.get(name) ?? 0) + 1);
    }
  }
}

async function scan() {
  const counts = new Map();
  for (const filePath of await rustFiles()) {
    const contents = await fs.readFile(path.join(repoRoot, filePath), "utf8");
    countAllowAttributes(contents, counts);
  }

  const violations = [];
  for (const [name, count] of [...counts.entries()].sort()) {
    const cap = ALLOW_ATTRIBUTE_CAPS[name];
    if (cap === undefined) {
      violations.push({ name, count, cap: 0, type: "uncapped-allow" });
      continue;
    }
    if (count > cap) {
      violations.push({ name, count, cap, type: "cap-exceeded" });
    }
  }
  for (const name of Object.keys(ALLOW_ATTRIBUTE_CAPS)) {
    if (!counts.has(name)) {
      counts.set(name, 0);
    }
  }
  return {
    caps: ALLOW_ATTRIBUTE_CAPS,
    counts: Object.fromEntries([...counts.entries()].sort()),
    violations,
  };
}

function printHuman(report) {
  if (report.violations.length === 0) {
    process.stdout.write(
      `allow attribute guard: ok (${Object.keys(report.counts).length} allow kind(s))\n`,
    );
    return;
  }

  process.stderr.write("allow attribute guard failed:\n");
  for (const violation of report.violations) {
    process.stderr.write(
      `  - ${violation.name}: ${violation.count} > ${violation.cap} (${violation.type})\n`,
    );
  }
  process.stderr.write("\nRemove the allowance, or deliberately lower/update the cap with rationale.\n");
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
  if (report.violations.length > 0) {
    process.exitCode = 1;
  }
}

main().catch((error) => {
  process.stderr.write(`allow-attribute-guard: ${error.message}\n`);
  process.exitCode = 1;
});
