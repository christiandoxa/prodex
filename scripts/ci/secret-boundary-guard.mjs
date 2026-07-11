#!/usr/bin/env node
import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import { git } from "./guard-common.mjs";
import { repoRoot } from "../npm/common.mjs";

const SENSITIVE = "(?:access[_-]?token|api[_-]?key|auth[_-]?token|bearer|capability|credential|password|secret|token)";
const ARG_BLOCK = /#\s*\[\s*arg\s*\((?<options>[\s\S]{0,1000}?)\)\s*\]\s*(?:pub(?:\([^)]*\))?\s+)?(?<field>[A-Za-z_][A-Za-z0-9_]*)\s*:/gu;
const EXPLICIT_LONG = new RegExp(`\\blong\\s*=\\s*"(${SENSITIVE})"`, "iu");
const LONG_OPTION = new RegExp(`\\blong\\s*=\\s*"(${SENSITIVE})"`, "giu");
const MANUAL_FLAG = new RegExp(`"--(${SENSITIVE})(?:=)?"`, "giu");
const QUERY_CAPABILITY = new RegExp(
  `(?:[a-z][a-z0-9+.-]*://|["'])[^\\s"']{0,1024}[?&]${SENSITIVE}=`,
  "giu",
);
const PATH_CAPABILITY = new RegExp(
  `(?:[a-z][a-z0-9+.-]*://|["'])[^\\s"']{0,1024}/\\{[^}]*${SENSITIVE}[^}]*\\}`,
  "giu",
);
const USERINFO_CAPABILITY = new RegExp(
  `[a-z][a-z0-9+.-]*://[^\\s/:"']{1,256}:\\{[^}]*${SENSITIVE}[^}]*\\}@`,
  "giu",
);
const DOCUMENTED_SECRET_ARG = new RegExp(
  `^\\s*(?:\\$\\s*)?prodex\\b[^\\n]*--${SENSITIVE}(?:=|\\s+\\S)`,
  "gimu",
);
const TEST_PATH = /(?:^|\/)(?:tests?|fixtures?)(?:\/|$)|(?:^|\/)(?:tests?|[^/]+_tests?)\.rs$|\.test\.mjs$|^scripts\/ci\/secret-boundary-guard\.mjs$/u;

// Compatibility debt only: new occurrences exhaust these per-file budgets and fail.
const LEGACY_SECRET_FLAG_BUDGET = new Map([
  ["crates/prodex-cli/src/runtime_args.rs:api-key", 2],
  ["crates/prodex-cli/src/runtime_args.rs:auth-token", 1],
  ["crates/prodex-cli/src/runtime_args/super_tail_extract.rs:api-key", 3],
  ["crates/prodex-app/src/app_commands/runtime_launch/providers_env.rs:api-key", 4],
]);

function lineNumber(contents, index) {
  return contents.slice(0, index).split(/\r\n|\n|\r/u).length;
}

function normalizedFlag(value) {
  return value.toLowerCase().replaceAll("_", "-");
}

function isTestFixture(filePath, contents, index) {
  if (TEST_PATH.test(filePath)) return true;
  const testModule = contents.search(/^#\s*\[cfg\(test\)\]/mu);
  return testModule !== -1 && index >= testModule;
}

function pushViolation(violations, filePath, contents, index, kind) {
  if (isTestFixture(filePath, contents, index)) return;
  const end = contents.indexOf("\n", index);
  violations.push({
    filePath,
    line: lineNumber(contents, index),
    kind,
    snippet: contents.slice(index, end === -1 ? undefined : end).trim().slice(0, 240),
  });
}

export function validateFiles(files) {
  const violations = [];
  const legacyHits = new Map();
  const recordFlag = (filePath, contents, index, rawFlag) => {
    if (isTestFixture(filePath, contents, index)) return;
    const flag = normalizedFlag(rawFlag);
    const key = `${filePath}:${flag}`;
    legacyHits.set(key, (legacyHits.get(key) ?? 0) + 1);
    if ((legacyHits.get(key) ?? 0) > (LEGACY_SECRET_FLAG_BUDGET.get(key) ?? 0)) {
      pushViolation(violations, filePath, contents, index, `secret-bearing CLI flag --${flag}`);
    }
  };

  for (const { filePath, contents } of files) {
    LONG_OPTION.lastIndex = 0;
    for (const match of contents.matchAll(LONG_OPTION)) {
      recordFlag(filePath, contents, match.index, match[1]);
    }

    for (const match of contents.matchAll(ARG_BLOCK)) {
      if (EXPLICIT_LONG.test(match.groups.options) || !/\blong\b/u.test(match.groups.options)) continue;
      const inferred = normalizedFlag(match.groups.field);
      if (new RegExp(`^${SENSITIVE}$`, "iu").test(inferred)) {
        recordFlag(filePath, contents, match.index, inferred);
      }
    }

    for (const match of contents.matchAll(MANUAL_FLAG)) {
      recordFlag(filePath, contents, match.index, match[1]);
    }

    for (const [pattern, kind] of [
      [QUERY_CAPABILITY, "secret-bearing URL query"],
      [PATH_CAPABILITY, "secret-bearing URL path"],
      [USERINFO_CAPABILITY, "secret-bearing URL userinfo"],
      [DOCUMENTED_SECRET_ARG, "documented secret-bearing CLI argument"],
    ]) {
      pattern.lastIndex = 0;
      for (const match of contents.matchAll(pattern)) {
        pushViolation(violations, filePath, contents, match.index, kind);
      }
    }
  }

  return violations;
}

function selfTest() {
  const safe = validateFiles([
    {
      filePath: "src/safe.rs",
      contents: 'let header = format!("Authorization: Bearer {token}");\nlet url = "/expose#bootstrap=opaque";',
    },
    {
      filePath: "crates/example/tests/redaction.rs",
      contents: 'let fixture = format!("https://example.test/run?token={secret}");',
    },
    {
      filePath: "src/embedded.rs",
      contents: '#[cfg(test)]\nmod tests { fn redacts() { let _ = format!("/run/{capability}"); } }',
    },
  ]);
  assert.deepEqual(safe, []);

  for (const [contents, expected] of [
    ['#[derive(Args)] struct Bad { #[arg(long = "secret")] pub secret: String }', "CLI flag"],
    ['match arg { "--password" => take(), _ => {} }', "CLI flag"],
    ['let url = format!("https://api.invalid/run?token={token}");', "URL query"],
    ['let url = format!("https://api.invalid/run/{capability}");', "URL path"],
    ['let url = format!("postgres://user:{password}@db.invalid/app");', "URL userinfo"],
    ['prodex gateway --auth-token "$PRODEX_GATEWAY_TOKEN"', "CLI argument"],
  ]) {
    const hits = validateFiles([{ filePath: "src/bad.rs", contents }]);
    assert.equal(hits.length, 1, `${expected} fixture was not rejected`);
    assert.match(hits[0].kind, new RegExp(expected, "iu"));
  }
}

async function repositoryFiles() {
  const result = await git(
    ["ls-files", "--cached", "--others", "--exclude-standard", "--", "*.rs", "*.md", "*.html", "*.mjs", "*.js", "*.toml", "*.yaml", "*.yml"],
    { cwd: repoRoot },
  );
  const paths = result.stdout.split(/\r?\n/u).filter(Boolean).sort();
  const files = await Promise.all(
    paths.map(async (filePath) => ({
      filePath,
      contents: await fs.readFile(path.join(repoRoot, filePath), "utf8").catch((error) => {
        if (error.code === "ENOENT") return null;
        throw error;
      }),
    })),
  );
  return files.filter((file) => file.contents !== null);
}

async function main() {
  if (process.argv.includes("--self-test")) selfTest();
  const violations = validateFiles(await repositoryFiles());
  if (violations.length === 0) {
    process.stdout.write("secret boundary guard: ok\n");
    return;
  }
  process.stderr.write("secret boundary guard failed:\n");
  for (const hit of violations) {
    process.stderr.write(`  - ${hit.filePath}:${hit.line}: ${hit.kind}: ${hit.snippet}\n`);
  }
  process.exitCode = 1;
}

main().catch((error) => {
  process.stderr.write(`secret-boundary-guard: ${error.message}\n`);
  process.exitCode = 1;
});
