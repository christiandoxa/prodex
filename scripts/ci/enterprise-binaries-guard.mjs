#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..", "..");

const BINARIES = [
  {
    path: "src/bin/prodex-gateway.rs",
    required: ["Data-plane gateway entrypoint", "DedicatedServerMode::DataPlane"],
  },
  {
    path: "src/bin/prodex-control-plane.rs",
    required: ["Control-plane entrypoint", "DedicatedServerMode::ControlPlane"],
  },
];
const FORBIDDEN_PATTERNS = [
  { name: "business runtime module", pattern: /runtime_launch|runtime_proxy|local_rewrite/u },
  { name: "HTTP framework", pattern: /\b(axum|hyper|tower|tiny_http)\s*::/u },
  { name: "database driver", pattern: /\b(postgres|rusqlite|redis|sqlx)\s*::/u },
  { name: "provider SDK", pattern: /\b(openai|anthropic|gemini|copilot)\s*::/u },
];

function validateBinary(binary) {
  const filePath = path.join(repoRoot, binary.path);
  const errors = [];
  if (!fs.existsSync(filePath)) {
    return [`${binary.path}: required enterprise binary entrypoint is missing`];
  }
  const source = fs.readFileSync(filePath, "utf8");
  for (const phrase of binary.required) {
    if (!source.includes(phrase)) {
      errors.push(`${binary.path}: missing required entrypoint phrase '${phrase}'`);
    }
  }
  source.split(/\r?\n/u).forEach((line, index) => {
    for (const forbidden of FORBIDDEN_PATTERNS) {
      if (forbidden.pattern.test(line)) {
        errors.push(`${binary.path}:${index + 1}: binary entrypoint must stay thin; found ${forbidden.name} '${line.trim()}'`);
      }
    }
  });
  return errors;
}

function runSelfTest() {
  const bad = "fn main() { tiny_http::Server::http(\"127.0.0.1:0\"); }";
  if (!FORBIDDEN_PATTERNS.some((forbidden) => forbidden.pattern.test(bad))) {
    throw new Error("self-test failed: forbidden pattern did not match");
  }
}

function main() {
  if (process.argv.includes("--self-test")) {
    runSelfTest();
    return;
  }
  const errors = BINARIES.flatMap(validateBinary);
  if (errors.length > 0) {
    for (const error of errors) process.stderr.write(`${error}\n`);
    process.exitCode = 1;
  }
}

main();
