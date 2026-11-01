#!/usr/bin/env node
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { repoRoot } from "../npm/common.mjs";

// Hashes prevent the denylist itself from republishing incident identifiers.
const privateIdentifiers = new Set(["286340c0669139cdef633cbb1d12b3a4bbdc657a6ae52978f0461152a6b16313", "bd3b7cabce4e9d2bc95dc2424a4570482c425f39c5eced19c8cef50278006fbe", "f576e2a210b863184ed8e1fe167f9ceb9cbeaae8114df75f29d634786e8548c1", "8547cd0c2cf0b6ad60ecff4a3e3bd46ea55f5ef4e01c1a8ce171b7699b39611d", "310181da6e5bdd26997b001fa254cee06f0822041922e1cb9d641ca2261f4468", "45d34cc2c2be31b0c1c4dd5b38249ebabe1e8b6af9420b8b2a80c2c3c68a0de3"]);
const syntheticUsers = new Set(["test", "test-user", "example-user", "user", "user-1", "workspace", "x", "runner", "root", "build", "ci", "vscode", "ubuntu", "node"]);
const digest = (value) => createHash("sha256").update(value.toLowerCase()).digest("hex");

export function privacyViolations(filePath, contents, denied = privateIdentifiers) {
  const violations = [];
  const record = (index, kind) => violations.push({ file: filePath, line: contents.slice(0, index).split("\n").length, kind });
  if (filePath.startsWith("migration/")) record(0, "retired incident archive");
  // Upstream copyright and license attribution must remain intact.
  if (filePath.startsWith("vendor/")) return violations;
  for (const match of contents.matchAll(/\/(?:home|Users)\/([A-Za-z0-9_.@-]+)/gu)) {
    if (!syntheticUsers.has(match[1])) record(match.index, "non-synthetic local home path");
  }
  for (const match of contents.matchAll(/(?<![A-Za-z0-9_])[A-Za-z][A-Za-z0-9_-]+/gu)) {
    if (!denied.has(digest(match[0]))) continue;
    // This lowercase wire route is a legacy protocol contract, not a hostname.
    const before = contents.slice(Math.max(0, match.index - 1), match.index);
    const after = contents.slice(match.index + match[0].length);
    if (before === "/" && after.startsWith("/v1/") && match[0] === match[0].toLowerCase()) continue;
    record(match.index, "private incident identifier");
  }
  return violations;
}

export function selfTest() {
  const denied = new Set([digest("private-machine")]);
  assert.equal(privacyViolations("docs/example.md", "Checked PRIVATE-MACHINE", denied).length, 1);
  assert.equal(privacyViolations("docs/example.md", ["/home", "private-person", "work"].join("/"), denied).length, 1);
  assert.deepEqual(privacyViolations("tests/example.rs", "/home/test-user/work", denied), []);
  assert.equal(privacyViolations("migration/example.md", "synthetic", denied).length, 1);
  assert.deepEqual(privacyViolations("vendor/library/LICENSE", "Upstream attribution", denied), []);
}

function main() {
  selfTest();
  const files = execFileSync("git", ["ls-files", "-z"], { cwd: repoRoot }).toString().split("\0").filter(Boolean);
  const violations = [];
  for (const filePath of files) {
    const file = path.join(repoRoot, filePath);
    if (!fs.existsSync(file) || !fs.lstatSync(file).isFile()) continue;
    const bytes = fs.readFileSync(file);
    if (bytes.includes(0)) continue;
    violations.push(...privacyViolations(filePath, bytes.toString("utf8")));
  }
  for (const item of violations) process.stderr.write(`${item.file}:${item.line}: ${item.kind}\n`);
  if (violations.length) process.exitCode = 1;
  else process.stdout.write("repository privacy guard: no private incident identifiers or retired archive\n");
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main();
