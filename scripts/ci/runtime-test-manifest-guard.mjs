#!/usr/bin/env node
import { readFile } from "node:fs/promises";
import { spawn } from "node:child_process";
import {
  RUNTIME_CI_BROAD_SHARD_FILTERS,
  RUNTIME_CI_TEST_CASES,
  RUNTIME_TEST_TAGS,
} from "./runtime-test-manifest.mjs";

const CARGO_LIST_ARGS = ["test", "--lib", "main_internal_tests::", "--", "--list"];
const CI_WORKFLOW_PATH = ".github/workflows/ci.yml";
const MAIN_INTERNAL_RUNTIME_PROXY_PREFIX = "main_internal_tests::runtime_proxy_";
const MAIN_INTERNAL_RUNTIME_CI_PREFIXES = Object.freeze([
  MAIN_INTERNAL_RUNTIME_PROXY_PREFIX,
  "main_internal_tests::prepare_runtime_proxy_claude_",
]);
const RUNTIME_PROXY_WORKFLOW_JOB = "main-internal-runtime-proxy";
const KNOWN_RUNTIME_TAGS = new Set(Object.values(RUNTIME_TEST_TAGS));
const SERIAL_OR_QUARANTINE_TAGS = new Set([RUNTIME_TEST_TAGS.serial, RUNTIME_TEST_TAGS.quarantine]);
const LEGACY_QUARANTINE_EVIDENCE_TAGS = new Set([
  RUNTIME_TEST_TAGS.stressSerialized,
  RUNTIME_TEST_TAGS.stressContinuation,
  RUNTIME_TEST_TAGS.envParallel,
]);

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

function formatBroadShard(shard, index) {
  const identity = shard?.id ?? shard?.filter ?? shard?.label;
  return identity ? `broadShard[${index}] ${identity}` : `broadShard[${index}]`;
}

function formatWorkflowShard(shard) {
  const location = shard.lineNumber ? ` (${CI_WORKFLOW_PATH}:${shard.lineNumber})` : "";
  return `"${shard.label}|${shard.filter}"${location}`;
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

function formatTagList(tags) {
  return tags.map((tag) => `"${tag}"`).join(", ");
}

function validateCaseTags(testCase, label, issues) {
  const hasTags = Object.hasOwn(testCase, "tags");
  if (!hasTags) {
    issues.push(`${label}: expected tags array`);
    return;
  }

  if (!Array.isArray(testCase.tags)) {
    issues.push(`${label}: tags must be an array`);
    return;
  }

  if (testCase.tags.length === 0) {
    issues.push(`${label}: tags must not be empty`);
    return;
  }

  const tags = [];
  testCase.tags.forEach((tag, tagIndex) => {
    if (!isNonEmptyString(tag)) {
      issues.push(`${label}: tags[${tagIndex}] must be a non-empty trimmed string`);
      return;
    }

    tags.push({ value: tag, index: tagIndex });
    if (!KNOWN_RUNTIME_TAGS.has(tag)) {
      issues.push(`${label}: unknown tag "${tag}"; expected one of ${formatTagList([...KNOWN_RUNTIME_TAGS])}`);
    }
  });

  for (const duplicate of collectDuplicates(tags)) {
    issues.push(`${label}: duplicate tag "${duplicate.value}" at indexes ${duplicate.indexes.join(", ")}`);
  }

  const tagSet = new Set(tags.map(({ value }) => value));
  const hasParallelSafe = tagSet.has(RUNTIME_TEST_TAGS.parallelSafe);
  const serialOrQuarantineTags = [...SERIAL_OR_QUARANTINE_TAGS].filter((tag) => tagSet.has(tag));
  const legacyEvidenceTags = [...LEGACY_QUARANTINE_EVIDENCE_TAGS].filter((tag) => tagSet.has(tag));

  if (hasParallelSafe && serialOrQuarantineTags.length > 0) {
    issues.push(
      `${label}: tag "${RUNTIME_TEST_TAGS.parallelSafe}" conflicts with ${formatTagList(serialOrQuarantineTags)}`,
    );
  }

  if (hasParallelSafe && legacyEvidenceTags.length > 0) {
    issues.push(
      `${label}: tag "${RUNTIME_TEST_TAGS.parallelSafe}" conflicts with legacy quarantine evidence ${formatTagList(legacyEvidenceTags)}`,
    );
  }

  if (legacyEvidenceTags.length > 0 && serialOrQuarantineTags.length === 0) {
    issues.push(
      `${label}: legacy quarantine evidence ${formatTagList(legacyEvidenceTags)} requires "${RUNTIME_TEST_TAGS.serial}" or "${RUNTIME_TEST_TAGS.quarantine}"`,
    );
  }
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

function testLeafName(testName) {
  return testName.split("::").at(-1);
}

function isMainInternalRuntimeProxyTest(testName) {
  return testName.startsWith(MAIN_INTERNAL_RUNTIME_PROXY_PREFIX);
}

function isMainInternalRuntimeCiTest(testName) {
  return MAIN_INTERNAL_RUNTIME_CI_PREFIXES.some((prefix) => testName.startsWith(prefix));
}

function manifestCaseMatchesTest(testCase, testName) {
  return (
    (typeof testCase.name === "string" && testLeafName(testName) === testCase.name) ||
    (typeof testCase.filter === "string" && testName.includes(testCase.filter))
  );
}

function broadShardMatchesTest(shard, testName) {
  return typeof shard.filter === "string" && testName.includes(shard.filter);
}

function lineIndent(line) {
  const match = line.match(/^ */);
  return match ? match[0].length : 0;
}

function parseRuntimeProxyWorkflowShardFilters(workflowText) {
  const lines = workflowText.split(/\r?\n/);
  const issues = [];
  const filters = [];
  const jobHeader = `  ${RUNTIME_PROXY_WORKFLOW_JOB}:`;
  const jobStartIndex = lines.findIndex((line) => line === jobHeader);

  if (jobStartIndex === -1) {
    return {
      filters,
      issues: [`${CI_WORKFLOW_PATH}: missing ${RUNTIME_PROXY_WORKFLOW_JOB} job`],
    };
  }

  const jobIndent = lineIndent(lines[jobStartIndex]);
  let jobEndIndex = lines.length;
  for (let index = jobStartIndex + 1; index < lines.length; index += 1) {
    const line = lines[index];
    if (line.trim() !== "" && lineIndent(line) === jobIndent && /^\s{2}[-\w]+:/.test(line)) {
      jobEndIndex = index;
      break;
    }
  }

  for (let index = jobStartIndex + 1; index < jobEndIndex; index += 1) {
    const line = lines[index];
    if (line.trim() !== "filters: |") {
      continue;
    }

    const filtersIndent = lineIndent(line);
    for (let blockIndex = index + 1; blockIndex < jobEndIndex; blockIndex += 1) {
      const blockLine = lines[blockIndex];
      if (blockLine.trim() === "") {
        continue;
      }

      if (lineIndent(blockLine) <= filtersIndent) {
        index = blockIndex - 1;
        break;
      }

      const entry = blockLine.trim();
      const separatorIndex = entry.indexOf("|");
      const lineNumber = blockIndex + 1;
      if (separatorIndex <= 0 || separatorIndex === entry.length - 1) {
        issues.push(`${CI_WORKFLOW_PATH}:${lineNumber}: invalid runtime proxy shard filter entry "${entry}"`);
        continue;
      }

      const label = entry.slice(0, separatorIndex).trim();
      const filter = entry.slice(separatorIndex + 1).trim();
      if (!isNonEmptyString(label) || !isNonEmptyString(filter)) {
        issues.push(`${CI_WORKFLOW_PATH}:${lineNumber}: runtime proxy shard label and filter must be non-empty`);
        continue;
      }

      filters.push({ label, filter, lineNumber });
    }
  }

  if (filters.length === 0 && issues.length === 0) {
    issues.push(`${CI_WORKFLOW_PATH}: ${RUNTIME_PROXY_WORKFLOW_JOB} job has no matrix filters blocks`);
  }

  return { filters, issues };
}

function validateWorkflowBroadShardFilters(broadShardFilters, workflowShardFilters, issues) {
  if (!Array.isArray(broadShardFilters) || !Array.isArray(workflowShardFilters)) {
    return;
  }

  const workflowLabels = workflowShardFilters.map((shard, index) => ({ value: shard.label, index }));
  const workflowFilters = workflowShardFilters.map((shard, index) => ({ value: shard.filter, index }));
  const broadLabels = broadShardFilters
    .filter((shard) => shard && typeof shard === "object" && !Array.isArray(shard))
    .map((shard, index) => ({ value: shard.label, index }));

  for (const duplicate of collectDuplicates(workflowLabels)) {
    issues.push(`duplicate workflow runtime proxy shard label "${duplicate.value}" in entries ${duplicate.indexes.join(", ")}`);
  }
  for (const duplicate of collectDuplicates(workflowFilters)) {
    issues.push(`duplicate workflow runtime proxy shard filter "${duplicate.value}" in entries ${duplicate.indexes.join(", ")}`);
  }
  for (const duplicate of collectDuplicates(broadLabels)) {
    issues.push(`duplicate broad shard label "${duplicate.value}" in indexes ${duplicate.indexes.join(", ")}`);
  }

  const manifestByLabel = new Map();
  const manifestByFilter = new Map();
  for (const shard of broadShardFilters) {
    if (!shard || typeof shard !== "object" || Array.isArray(shard)) {
      continue;
    }
    if (typeof shard.label === "string" && typeof shard.filter === "string") {
      manifestByLabel.set(shard.label, shard);
      manifestByFilter.set(shard.filter, shard);
    }
  }

  const workflowByLabel = new Map(workflowShardFilters.map((shard) => [shard.label, shard]));
  const workflowByFilter = new Map(workflowShardFilters.map((shard) => [shard.filter, shard]));

  for (const workflowShard of workflowShardFilters) {
    const manifestShard = manifestByLabel.get(workflowShard.label);
    if (manifestShard) {
      if (manifestShard.filter !== workflowShard.filter) {
        issues.push(
          [
            `runtime CI broad shard filter mismatch for label "${workflowShard.label}":`,
            `workflow has "${workflowShard.filter}"`,
            `RUNTIME_CI_BROAD_SHARD_FILTERS has "${manifestShard.filter}"`,
          ].join(" "),
        );
      }
      continue;
    }

    const sameFilterShard = manifestByFilter.get(workflowShard.filter);
    if (sameFilterShard) {
      issues.push(
        [
          `runtime CI broad shard label mismatch for filter "${workflowShard.filter}":`,
          `workflow has "${workflowShard.label}"`,
          `RUNTIME_CI_BROAD_SHARD_FILTERS has "${sameFilterShard.label}"`,
        ].join(" "),
      );
      continue;
    }

    issues.push(`workflow runtime proxy shard ${formatWorkflowShard(workflowShard)} is missing from RUNTIME_CI_BROAD_SHARD_FILTERS`);
  }

  for (const manifestShard of broadShardFilters) {
    if (!manifestShard || typeof manifestShard !== "object" || Array.isArray(manifestShard)) {
      continue;
    }
    if (workflowByLabel.has(manifestShard.label)) {
      continue;
    }
    if (workflowByFilter.has(manifestShard.filter)) {
      continue;
    }
    issues.push(
      `RUNTIME_CI_BROAD_SHARD_FILTERS broad shard "${manifestShard.label}|${manifestShard.filter}" is missing from ${CI_WORKFLOW_PATH}`,
    );
  }
}

function validateBroadShardFilters(broadShardFilters, cargoTestNames, issues) {
  if (!Array.isArray(broadShardFilters)) {
    issues.push("RUNTIME_CI_BROAD_SHARD_FILTERS must be an array");
    return [];
  }

  const ids = [];
  const filters = [];
  const validShards = [];

  broadShardFilters.forEach((shard, index) => {
    const label = formatBroadShard(shard, index);
    if (!shard || typeof shard !== "object" || Array.isArray(shard)) {
      issues.push(`${label}: expected object with id, filter, and label`);
      return;
    }

    if (!isNonEmptyString(shard.id)) {
      issues.push(`${label}: id must be a non-empty trimmed string`);
    } else {
      ids.push({ value: shard.id, index });
    }

    if (!isNonEmptyString(shard.label)) {
      issues.push(`${label}: label must be a non-empty trimmed string`);
    }

    if (!isNonEmptyString(shard.filter)) {
      issues.push(`${label}: filter must be a non-empty trimmed string`);
      return;
    }

    filters.push({ value: shard.filter, index });
    const matches = cargoTestNames.filter((testName) => testName.includes(shard.filter));
    if (matches.length === 0) {
      issues.push(
        `${label}: filter "${shard.filter}" matched zero tests; broad shard filters should mirror active CI shard filters`,
      );
      return;
    }

    const nonRuntimeCiMatches = matches.filter((testName) => !isMainInternalRuntimeCiTest(testName));
    if (nonRuntimeCiMatches.length > 0) {
      issues.push(
        `${label}: filter "${shard.filter}" matched tests outside runtime CI ownership: ${nonRuntimeCiMatches.join(", ")}`,
      );
      return;
    }

    validShards.push(shard);
  });

  for (const duplicate of collectDuplicates(ids)) {
    issues.push(`duplicate broad shard id "${duplicate.value}" in indexes ${duplicate.indexes.join(", ")}`);
  }
  for (const duplicate of collectDuplicates(filters)) {
    issues.push(`duplicate broad shard filter "${duplicate.value}" in indexes ${duplicate.indexes.join(", ")}`);
  }

  return validShards;
}

function validateRuntimeProxyCoverage(testCases, broadShardFilters, cargoTestNames, issues) {
  const runtimeProxyTests = cargoTestNames.filter(isMainInternalRuntimeProxyTest);
  const uncovered = runtimeProxyTests.filter(
    (testName) =>
      !testCases.some((testCase) => manifestCaseMatchesTest(testCase, testName)) &&
      !broadShardFilters.some((shard) => broadShardMatchesTest(shard, testName)),
  );

  if (uncovered.length > 0) {
    issues.push(
      [
        `${uncovered.length} main_internal_tests::runtime_proxy_ test(s) are not covered by manifest cases or broad CI shard filters:`,
        ...uncovered.map((testName) => `    ${testName}`),
        `  Add a manifest case in RUNTIME_CI_TEST_CASES or an intentional broad shard filter in RUNTIME_CI_BROAD_SHARD_FILTERS.`,
      ].join("\n"),
    );
  }

  return {
    broadShardCoveredCount: runtimeProxyTests.filter((testName) =>
      broadShardFilters.some((shard) => broadShardMatchesTest(shard, testName)),
    ).length,
    manifestCoveredCount: runtimeProxyTests.filter((testName) =>
      testCases.some((testCase) => manifestCaseMatchesTest(testCase, testName)),
    ).length,
    runtimeProxyTestCount: runtimeProxyTests.length,
  };
}

function validateManifest(testCases, broadShardFilters, workflowShardFilters, cargoTestNames) {
  const issues = [];
  const ids = [];
  const names = [];
  const leafNames = testsByLeafName(cargoTestNames);

  testCases.forEach((testCase, index) => {
    const label = formatCase(testCase, index);
    const hasName = Object.hasOwn(testCase, "name");
    const hasFilter = Object.hasOwn(testCase, "filter");
    const hasId = Object.hasOwn(testCase, "id");

    validateCaseTags(testCase, label, issues);

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

  const validBroadShardFilters = validateBroadShardFilters(broadShardFilters, cargoTestNames, issues);
  validateWorkflowBroadShardFilters(broadShardFilters, workflowShardFilters, issues);
  const coverage = validateRuntimeProxyCoverage(testCases, validBroadShardFilters, cargoTestNames, issues);

  return { coverage, issues };
}

function printHelp() {
  process.stdout.write(
    [
      "Usage: node scripts/ci/runtime-test-manifest-guard.mjs",
      "",
      "Validates scripts/ci/runtime-test-manifest.mjs against:",
      `  ${formatCommand("cargo", CARGO_LIST_ARGS)}`,
      `  ${CI_WORKFLOW_PATH}`,
      "",
      "Checks:",
      "  - manifest name entries resolve to one Cargo test leaf name",
      "  - manifest filter entries match one or more full Cargo test paths",
      "  - manifest ids and names are not duplicated",
      "  - manifest tags are known, non-duplicated, and not contradictory",
      "  - each main_internal_tests::runtime_proxy_ test is covered by a manifest case",
      "    or by an intentional RUNTIME_CI_BROAD_SHARD_FILTERS entry",
      "  - broad CI shard filters resolve and do not match tests outside runtime CI ownership",
      "  - broad CI shard filter labels and filters exactly match the runtime proxy workflow matrix",
      "",
      "This catches renamed tests, filter typos, workflow drift, and unclassified runtime proxy tests that would otherwise drift out of manifest-owned CI coverage.",
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
  const workflowText = await readFile(CI_WORKFLOW_PATH, "utf8");
  const parsedWorkflowFilters = parseRuntimeProxyWorkflowShardFilters(workflowText);
  const { stdout } = await run("cargo", CARGO_LIST_ARGS);
  const cargoTestNames = parseCargoTestList(stdout);
  if (cargoTestNames.length === 0) {
    throw new Error("cargo test list returned zero parsed tests; expected main_internal_tests entries");
  }

  const { coverage, issues } = validateManifest(
    RUNTIME_CI_TEST_CASES,
    RUNTIME_CI_BROAD_SHARD_FILTERS,
    parsedWorkflowFilters.issues.length === 0 ? parsedWorkflowFilters.filters : null,
    cargoTestNames,
  );
  issues.unshift(...parsedWorkflowFilters.issues);
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
    [
      `runtime manifest guard: validated ${RUNTIME_CI_TEST_CASES.length} manifest cases`,
      `${RUNTIME_CI_BROAD_SHARD_FILTERS.length} broad shard filters`,
      `${parsedWorkflowFilters.filters.length} workflow shard filters`,
      `against ${cargoTestNames.length} cargo tests`,
      `(${coverage.runtimeProxyTestCount} main_internal_tests::runtime_proxy_ tests;`,
      `${coverage.manifestCoveredCount} matched manifest, ${coverage.broadShardCoveredCount} matched broad shard)`,
    ].join(" ") + "\n",
  );
}

try {
  await main();
} catch (error) {
  process.stderr.write(`${error.message}\n`);
  process.exitCode = 1;
}
