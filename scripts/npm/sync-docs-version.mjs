#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { repoRoot, readCargoVersion, packageVersionPattern } from "./common.mjs";

const docs = ["README.md", "QUICKSTART.md"];

function parseArgs(argv) {
  const args = {};
  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--version") {
      index += 1;
      if (!argv[index]) {
        throw new Error("--version requires a value");
      }
      args.version = argv[index];
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

function syncDocVersion(contents, version) {
  let next = contents;
  next = next.replace(
    /(The current local version in this repo is `)([^`]+)(`:)/g,
    `$1${version}$3`,
  );
  next = next.replace(
    /(npm install -g @christiandoxa\/prodex@)([^\s`]+)/g,
    `$1${version}`,
  );
  next = next.replace(
    /(cargo install prodex --force --version )([^\s`]+)/g,
    `$1${version}`,
  );
  return next;
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    process.stdout.write(
      [
        "Usage: node scripts/npm/sync-docs-version.mjs [--version <version>]",
        "",
        "Updates versioned install snippets in README.md and QUICKSTART.md.",
      ].join("\n") + "\n",
    );
    return;
  }

  const version = args.version ?? (await readCargoVersion());
  if (!packageVersionPattern.test(version)) {
    throw new Error(`invalid version: ${version}`);
  }

  let changedCount = 0;
  for (const relativePath of docs) {
    const filePath = path.join(repoRoot, relativePath);
    const current = await fs.readFile(filePath, "utf8");
    const next = syncDocVersion(current, version);
    if (next !== current) {
      await fs.writeFile(filePath, next);
      changedCount += 1;
    }
  }

  process.stdout.write(`synced ${changedCount} doc file(s) to ${version}\n`);
}

await main();
