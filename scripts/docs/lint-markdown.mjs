#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { repoRoot } from "../npm/common.mjs";

async function defaultMarkdownFiles() {
  const markdownFiles = [];
  const ignoredDirectories = new Set([".git", "node_modules", "target"]);
  async function collect(directory) {
    for (const entry of await fs.readdir(directory, { withFileTypes: true })) {
      if (entry.isDirectory() && ignoredDirectories.has(entry.name)) {
        continue;
      }
      const absolute = path.join(directory, entry.name);
      if (entry.isDirectory()) {
        await collect(absolute);
      } else if (entry.isFile() && entry.name.endsWith(".md")) {
        markdownFiles.push(path.relative(repoRoot, absolute));
      }
    }
  }
  await collect(repoRoot);
  return markdownFiles.sort();
}

async function duplicateGovernancePrefixes() {
  const root = path.join(repoRoot, "docs", "enterprise-governance");
  const files = await fs.readdir(root, { withFileTypes: true });
  const byPrefix = new Map();
  for (const entry of files) {
    const match = entry.isFile() && entry.name.match(/^(\d{2})-.*\.md$/u);
    if (!match) continue;
    const names = byPrefix.get(match[1]) ?? [];
    names.push(entry.name);
    byPrefix.set(match[1], names);
  }
  return [...byPrefix.entries()]
    .filter(([, names]) => names.length > 1)
    .map(([prefix, names]) =>
      `docs/enterprise-governance: duplicate numeric prefix ${prefix}: ${names.sort().join(", ")}`,
    );
}

function parseArgs(argv) {
  const files = [];
  let help = false;

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--help" || value === "-h") {
      help = true;
      continue;
    }
    files.push(value);
  }

  return { files, help };
}

function fenceInfo(line) {
  const match = line.match(/^(\s*)([`~]{3,})(.*)$/);
  if (!match) {
    return null;
  }

  const [, , fence] = match;
  const marker = fence[0];
  if (!fence.split("").every((character) => character === marker)) {
    return null;
  }

  return { marker, length: fence.length };
}

function resolveLinkTarget(filePath, rawTarget) {
  const trimmed = rawTarget.trim();
  const target = trimmed.replace(/^<|>$/g, "");
  if (!target || target.startsWith("#")) {
    return null;
  }
  if (/^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(target)) {
    return null;
  }

  const [relativeTarget] = target.split(/\s+/);
  const [pathTarget] = relativeTarget.split("#", 1);
  if (!pathTarget) {
    return null;
  }

  return path.resolve(path.dirname(filePath), pathTarget);
}

async function lintFile(relativePath) {
  const filePath = path.join(repoRoot, relativePath);
  const contents = await fs.readFile(filePath, "utf8");
  const lines = contents.split(/\r?\n/);
  const errors = [];
  let fence = null;

  for (let lineIndex = 0; lineIndex < lines.length; lineIndex += 1) {
    const line = lines[lineIndex];
    const fenceCandidate = fenceInfo(line);

    if (fence) {
      if (
        fenceCandidate &&
        fenceCandidate.marker === fence.marker &&
        fenceCandidate.length >= fence.length &&
        /^\s*[`~]{3,}\s*$/.test(line)
      ) {
        fence = null;
      }
      continue;
    }

    if (fenceCandidate) {
      fence = fenceCandidate;
      continue;
    }

    for (const match of line.matchAll(/!?\[[^\]]*\]\(([^)]+)\)/g)) {
      const targetPath = resolveLinkTarget(filePath, match[1]);
      if (!targetPath) {
        continue;
      }
      const exists = await fs
        .access(targetPath)
        .then(() => true)
        .catch(() => false);
      if (!exists) {
        errors.push(`${relativePath}:${lineIndex + 1}: broken local link ${match[1]}`);
      }
    }
  }

  if (fence) {
    errors.push(
      `${relativePath}:${lines.length}: unterminated code fence opened with ${fence.marker.repeat(
        fence.length,
      )}`,
    );
  }

  return errors;
}

async function main() {
  const args = parseArgs(process.argv);
  const files = args.files.length > 0 ? args.files : await defaultMarkdownFiles();

  if (args.help) {
    process.stdout.write(
      [
        "Usage: node scripts/docs/lint-markdown.mjs [files...]",
        "",
        "Checks markdown fence balance and local file links.",
      ].join("\n") + "\n",
    );
    return;
  }

  const errors = [];
  for (const relativePath of files) {
    errors.push(...(await lintFile(relativePath)));
  }
  if (args.files.length === 0) {
    errors.push(...(await duplicateGovernancePrefixes()));
  }

  if (errors.length > 0) {
    for (const error of errors) {
      process.stderr.write(`${error}\n`);
    }
    process.exitCode = 1;
    return;
  }

  process.stdout.write(`linted ${files.length} markdown file(s)\n`);
}

await main();
