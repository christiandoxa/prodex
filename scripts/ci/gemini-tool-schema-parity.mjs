#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, "..", "..");
const sourcePath = path.join(
  repoRoot,
  "crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request.rs",
);

const PARITY = Object.freeze([
  {
    tool: "read_file",
    description: [
      "targeted, surgical ranges",
      "start_line and end_line",
    ],
    parameters: {
      start_line: "1-based first line",
      end_line: "1-based last line",
    },
  },
  {
    tool: "read_many_files",
    description: [
      "Read multiple files",
      "honor git, Gemini, custom ignore rules",
      "default heavy-directory excludes",
    ],
  },
  {
    tool: "grep",
    description: [
      "ripgrep-style behavior",
      "Use this before broad reads",
    ],
    parameters: {
      pattern: "Literal or regex pattern",
    },
  },
  {
    tool: "exec_command",
    description: [
      "Run a shell command",
      "non-interactive commands",
      "continue background sessions",
    ],
  },
  {
    tool: "write_file",
    description: [
      "Write complete file content",
      "Do not use placeholders",
      "preserve unrelated user changes",
    ],
  },
  {
    tool: "apply_patch",
    description: [
      "Apply targeted file edits",
      "Keep edits narrow",
      "instead of describing changes",
    ],
  },
]);

function parseArgs(argv) {
  const args = { json: false };
  for (const value of argv.slice(2)) {
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
      "Usage: node scripts/ci/gemini-tool-schema-parity.mjs [--json]",
      "",
      "Validates Gemini 3 tool schema compatibility text in gemini_request.rs.",
      "--json prints the generated parity manifest used by the validator.",
    ].join("\n") + "\n",
  );
}

function validate(source) {
  const failures = [];
  for (const item of PARITY) {
    for (const snippet of item.description) {
      if (!source.includes(snippet)) {
        failures.push(`${item.tool}: missing description snippet ${JSON.stringify(snippet)}`);
      }
    }
    for (const [parameter, snippet] of Object.entries(item.parameters ?? {})) {
      if (!source.includes(`"${parameter}"`) || !source.includes(snippet)) {
        failures.push(
          `${item.tool}.${parameter}: missing parameter description snippet ${JSON.stringify(snippet)}`,
        );
      }
    }
  }
  return failures;
}

function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }
  if (args.json) {
    process.stdout.write(JSON.stringify({ version: 1, parity: PARITY }, null, 2) + "\n");
    return;
  }
  const source = fs.readFileSync(sourcePath, "utf8");
  const failures = validate(source);
  if (failures.length > 0) {
    for (const failure of failures) {
      process.stderr.write(`gemini schema parity: ${failure}\n`);
    }
    process.exitCode = 1;
    return;
  }
  process.stdout.write(`gemini schema parity: ${PARITY.length} tool contracts validated\n`);
}

main();
