#!/usr/bin/env node
import { spawn } from "node:child_process";
import { RUNTIME_CI_TEST_CASES } from "./runtime-test-manifest.mjs";

const CARGO_LIST_ARGS = ["test", "--lib", "main_internal_tests::", "--", "--list"];

function parseArgs(argv) {
  const args = {};
  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--help" || value === "-h") {
      args.help = true;
      continue;
    }
    throw new Error(`unknown argument: ${value}`);
  }
  return args;
}

function formatCommand(command, args) {
  return [command, ...args].join(" ");
}

function formatCase(testCase, index) {
  const identity = testCase.id ?? testCase.name ?? testCase.filter ?? testCase.label;
  return identity ? `case[${index}] ${identity}` : `case[${index}]`;
}

function collectDuplicates(values) {
  const indexesByValue = new Map();
  values.forEach(({ value, index }) => {
    const indexes = indexesByValue.get(value) ?? [];
    indexes.push(index);
    indexesByValue.set(value, indexes);
  });
  return [...indexesByValue.entries()]
    .filter(([, indexes]) => indexes.length > 1)
    .map(([value, indexes]) => ({ value, indexes }));
}

function isNonEmptyString(value) {
  return typeof value === "string" && value.trim() === value && value.length > 0;
}

function parseCargoTestList(output) {
  const tests = [];
  for (const line of output.split(/\r?\n/)) {
    const match = line.match(/^\s*(.+): test$/);
    if (match) {
      tests.push(match[1]);
    }
  }
  return tests;
}

function tailLines(text, lineCount) {
  const lines = text.split(/\r?\n/);
  while (lines.length > 0 && lines.at(-1) === "") {
    lines.pop();
  }
  return lines.slice(-lineCount).join("\n");
}

function run(command, args) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: process.cwd(),
      stdio: ["ignore", "pipe", "pipe"],
    });

    let stdout = "";
    let stderr = "";
    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    child.on("error", reject);
    child.on("close", (code, signal) => {
      if (signal) {
        reject(new Error(`${formatCommand(command, args)} exited with signal ${signal}`));
        return;
      }
      if (code !== 0) {
        const details = [stderr.trim(), tailLines(stdout, 40)].filter(Boolean).join("\n");
        reject(new Error(`${formatCommand(command, args)} exited with code ${code}${details ? `\n${details}` : ""}`));
        return;
      }
      resolve({ stdout, stderr });
    });
  });
}

function testsByLeafName(testNames) {
  const result = new Map();
  for (const testName of testNames) {
    const leafName = testName.split("::").at(-1);
    const existing = result.get(leafName) ?? [];
    existing.push(testName);
    result.set(leafName, existing);
  }
  return result;
}

function validateManifest(testCases, cargoTestNames) {
  const issues = [];
  const ids = [];
  const names = [];
  const leafNames = testsByLeafName(cargoTestNames);

  testCases.forEach((testCase, index) => {
    const label = formatCase(testCase, index);
    const hasName = Object.hasOwn(testCase, "name");
    const hasFilter = Object.hasOwn(testCase, "filter");
    const hasId = Object.hasOwn(testCase, "id");

    if (!hasName && !hasFilter) {
      issues.push(`${label}: expected "name" or "filter"`);
    }

    if (hasId) {
      if (!isNonEmptyString(testCase.id)) {
        issues.push(`${label}: id must be a non-empty trimmed string`);
      } else {
        ids.push({ value: testCase.id, index });
      }
    }

    if (hasName) {
      if (!isNonEmptyString(testCase.name)) {
        issues.push(`${label}: name must be a non-empty trimmed string`);
      } else {
        names.push({ value: testCase.name, index });
        const matches = leafNames.get(testCase.name) ?? [];
        if (matches.length === 0) {
          issues.push(
            `${label}: name "${testCase.name}" does not appear in cargo test list; expected an exact test leaf name`,
          );
        } else if (matches.length > 1) {
          issues.push(
            `${label}: name "${testCase.name}" is ambiguous and matches ${matches.length} tests: ${matches.join(", ")}`,
          );
        }
      }
    }

    if (hasFilter) {
      if (!isNonEmptyString(testCase.filter)) {
        issues.push(`${label}: filter must be a non-empty trimmed string`);
      } else {
        const matches = cargoTestNames.filter((testName) => testName.includes(testCase.filter));
        if (matches.length === 0) {
          issues.push(
            `${label}: filter "${testCase.filter}" matched zero tests; cargo test filters are substring matches against full test paths`,
          );
        }
      }
    }
  });

  for (const duplicate of collectDuplicates(ids)) {
    issues.push(`duplicate id "${duplicate.value}" in manifest cases ${duplicate.indexes.join(", ")}`);
  }
  for (const duplicate of collectDuplicates(names)) {
    issues.push(`duplicate name "${duplicate.value}" in manifest cases ${duplicate.indexes.join(", ")}`);
  }

  return issues;
}

function printHelp() {
  process.stdout.write(
    [
      "Usage: node scripts/ci/runtime-test-manifest-guard.mjs",
      "",
      "Validates scripts/ci/runtime-test-manifest.mjs against:",
      `  ${formatCommand("cargo", CARGO_LIST_ARGS)}`,
      "",
      "Checks:",
      "  - manifest name entries resolve to one Cargo test leaf name",
      "  - manifest filter entries match one or more full Cargo test paths",
      "  - manifest ids and names are not duplicated",
      "",
      "This catches renamed tests and filter typos that would otherwise run zero tests in CI.",
    ].join("\n") + "\n",
  );
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }

  process.stdout.write(`runtime manifest guard: ${formatCommand("cargo", CARGO_LIST_ARGS)}\n`);
  const { stdout } = await run("cargo", CARGO_LIST_ARGS);
  const cargoTestNames = parseCargoTestList(stdout);
  if (cargoTestNames.length === 0) {
    throw new Error("cargo test list returned zero parsed tests; expected main_internal_tests entries");
  }

  const issues = validateManifest(RUNTIME_CI_TEST_CASES, cargoTestNames);
  if (issues.length > 0) {
    throw new Error(
      [
        `runtime CI manifest guard failed with ${issues.length} issue(s):`,
        ...issues.map((issue) => `  - ${issue}`),
        "",
        "Run with --help for validation rules.",
      ].join("\n"),
    );
  }

  process.stdout.write(
    `runtime manifest guard: validated ${RUNTIME_CI_TEST_CASES.length} manifest cases against ${cargoTestNames.length} cargo tests\n`,
  );
}

try {
  await main();
} catch (error) {
  process.stderr.write(`${error.message}\n`);
  process.exitCode = 1;
}
