#!/usr/bin/env node
import { fileURLToPath } from "node:url";

const SELECTION_PREFIX = "main_internal_tests::runtime_proxy_selection_and_pressure::";
const LAUNCH_PREFIX = "runtime_launch::proxy_startup::";
const ADMISSION_PREFIX = `${SELECTION_PREFIX}admission::`;
const MAIN_INTERNAL_FILTER = "main_internal_tests::";
const PROFILE_COMMANDS_INTERNAL_FILTER = "profile_commands_internal_tests::";

const TARGETED_SHARDS = Object.freeze([
  {
    suite: "selection",
    label: "prodex-app selection",
    filters: [`${SELECTION_PREFIX}selection::`, `${SELECTION_PREFIX}pressure::`, `${SELECTION_PREFIX}incidents::`],
  },
  {
    suite: "admission-core",
    label: "prodex-app admission core",
    filters: [
      `${ADMISSION_PREFIX}compact::`,
      `${ADMISSION_PREFIX}continuation_store::`,
      `${ADMISSION_PREFIX}doctor_summary::`,
      `${ADMISSION_PREFIX}pressure_budget::`,
      `${ADMISSION_PREFIX}turn_state::`,
      `${SELECTION_PREFIX}state::`,
      `${SELECTION_PREFIX}rotation::`,
      `${SELECTION_PREFIX}health::`,
    ],
  },
  {
    suite: "admission-affinity",
    label: "prodex-app admission guards and affinity",
    filters: [
      `${ADMISSION_PREFIX}cli_mount::`,
      `${ADMISSION_PREFIX}guards::`,
      `${ADMISSION_PREFIX}pre_send::`,
      `${ADMISSION_PREFIX}previous_response::`,
      `${ADMISSION_PREFIX}response_affinity::`,
      `${ADMISSION_PREFIX}sse_tap::`,
    ],
  },
  {
    suite: "launch-local",
    label: "prodex-app local rewrite",
    filters: [`${LAUNCH_PREFIX}local_rewrite_tests::`],
  },
  {
    suite: "launch-gemini",
    label: "prodex-app Gemini runtime",
    filters: [`${LAUNCH_PREFIX}gemini`, `${LAUNCH_PREFIX}local_rewrite_gemini`],
  },
  {
    suite: "launch-gateway",
    label: "prodex-app gateway runtime",
    filters: [`${LAUNCH_PREFIX}local_rewrite_gateway`],
  },
  {
    suite: "launch-providers",
    label: "prodex-app provider runtimes",
    filters: [
      `${LAUNCH_PREFIX}provider`,
      `${LAUNCH_PREFIX}deepseek`,
      `${LAUNCH_PREFIX}local_rewrite_deepseek`,
      `${LAUNCH_PREFIX}local_rewrite_transport`,
      `${LAUNCH_PREFIX}local_rewrite_copilot`,
      `${LAUNCH_PREFIX}local_rewrite_anthropic`,
      `${LAUNCH_PREFIX}anthropic`,
      `${LAUNCH_PREFIX}local_rewrite_kiro`,
    ],
  },
  {
    suite: "commands",
    label: "prodex-app commands",
    filters: ["app_commands::"],
  },
  {
    suite: "runtime",
    label: "prodex-app runtime",
    filters: ["runtime_proxy::"],
  },
  {
    suite: "profiles",
    label: "prodex-app profiles",
    filters: ["profile_commands::"],
  },
  {
    suite: "brokers",
    label: "prodex-app brokers",
    filters: [
      "main_internal_tests::app_server_broker::",
      "main_internal_tests::runtime_proxy_claude_and_anthropic::",
    ],
  },
  {
    suite: "support",
    label: "prodex-app support modules",
    filters: [
      "quota_support::",
      "runtime_state_shared::",
      "runtime_broker::",
      "runtime_tools::",
      "runtime_model_preferences::",
      "runtime_gemini_cli::",
      "runtime_kiro_acp::",
      "runtime_config::",
      "expose::",
      "runtime_gemini_auth::",
    ],
  },
]);

export const PRODEX_APP_LIB_FILTERS = Object.freeze(TARGETED_SHARDS.flatMap((shard) => shard.filters));

export const PRODEX_APP_LIB_SHARDS = Object.freeze([
  ...TARGETED_SHARDS,
  {
    suite: "remainder",
    label: "prodex-app remaining library tests",
    skipFilters: PRODEX_APP_LIB_FILTERS,
  },
]);

// Filters in each targeted shard are disjoint. Give each independent filter its
// own runner; Windows stays grouped below to avoid recompiling the workspace
// for every filter on the slower platform.
function splitIndependentShards(shards) {
  return shards.flatMap((shard) => {
    if (!Array.isArray(shard.filters) || shard.filters.length < 2) return [shard];
    return shard.filters.map((filter, index) => {
      const split = {
        suite: `${shard.suite}-${index + 1}`,
        label: `${shard.label} ${index + 1}/${shard.filters.length}`,
        filters: [filter],
      };
      if (shard.skipFilters) split.skipFilters = shard.skipFilters;
      return split;
    });
  });
}

export const PRODEX_APP_FULL_TEST_SHARDS = Object.freeze(splitIndependentShards(PRODEX_APP_LIB_SHARDS));

// Dedicated Ubuntu jobs own all main-internal and profile-command-internal tests.
// Keep related filters together so each push/PR job reuses one compilation.
const CI_TARGETED_SHARDS = Object.freeze(
  TARGETED_SHARDS.map((shard) => ({
    ...shard,
    filters: shard.filters.filter((filter) => !filter.startsWith(MAIN_INTERNAL_FILTER)),
    skipFilters: [MAIN_INTERNAL_FILTER, PROFILE_COMMANDS_INTERNAL_FILTER],
  })).filter((shard) => shard.filters.length > 0),
);
const CI_REMAINDER_SHARD = Object.freeze({
  ...PRODEX_APP_LIB_SHARDS.at(-1),
  skipFilters: [
    ...PRODEX_APP_LIB_FILTERS.filter((filter) => !filter.startsWith(MAIN_INTERNAL_FILTER)),
    MAIN_INTERNAL_FILTER,
    PROFILE_COMMANDS_INTERNAL_FILTER,
  ],
});

const WORKSPACE_SHARD = Object.freeze({
  suite: "workspace",
  label: "workspace and auto-rotate",
});

const WINDOWS_SHARD_GROUPS = Object.freeze([
  {
    suite: "commands-gemini",
    label: "prodex-app commands and Gemini runtime",
    members: ["commands", "launch-gemini"],
  },
  {
    suite: "local-brokers-profiles",
    label: "prodex-app local rewrite, brokers, and profiles",
    members: ["launch-local", "brokers", "profiles"],
  },
  {
    suite: "gateway-support-affinity",
    label: "prodex-app gateway, support, and affinity",
    members: ["launch-gateway", "support", "admission-affinity"],
  },
  {
    suite: "selection-providers-runtime",
    label: "prodex-app selection, providers, and runtime",
    members: ["selection", "admission-core", "launch-providers", "runtime"],
  },
]);

function collectDuplicates(values) {
  const indexesByValue = new Map();
  values.forEach((value, index) => {
    const indexes = indexesByValue.get(value) ?? [];
    indexes.push(index);
    indexesByValue.set(value, indexes);
  });
  return [...indexesByValue.entries()]
    .filter(([, indexes]) => indexes.length > 1)
    .map(([value, indexes]) => `${value} at indexes ${indexes.join(", ")}`);
}

export function validateShards(shards = PRODEX_APP_LIB_SHARDS) {
  const issues = [];
  if (!Array.isArray(shards) || shards.length === 0) {
    return ["prodex-app shard manifest must be a non-empty array"];
  }

  const suites = [];
  const filters = [];
  let remainder;
  for (const [index, shard] of shards.entries()) {
    if (!shard || typeof shard !== "object" || Array.isArray(shard)) {
      issues.push(`shard[${index}] must be an object`);
      continue;
    }
    if (typeof shard.suite !== "string" || shard.suite.trim() === "") {
      issues.push(`shard[${index}] suite must be a non-empty string`);
    } else {
      suites.push(shard.suite);
    }
    if (typeof shard.label !== "string" || shard.label.trim() === "") {
      issues.push(`shard[${index}] label must be a non-empty string`);
    }
    const hasFilters = Object.hasOwn(shard, "filters");
    const hasSkipFilters = Object.hasOwn(shard, "skipFilters");
    if (hasFilters === hasSkipFilters) {
      issues.push(`shard[${index}] must define exactly one of filters or skipFilters`);
      continue;
    }
    if (hasFilters) {
      if (!Array.isArray(shard.filters) || shard.filters.length === 0) {
        issues.push(`shard[${index}] filters must be a non-empty array`);
      } else {
        for (const filter of shard.filters) {
          if (typeof filter !== "string" || filter.trim() === "") {
            issues.push(`shard[${index}] filters must contain only non-empty strings`);
          } else {
            filters.push(filter);
          }
        }
      }
    } else if (!Array.isArray(shard.skipFilters) || shard.skipFilters.length === 0) {
      issues.push(`shard[${index}] skipFilters must be a non-empty array`);
    } else {
      remainder = shard;
    }
  }

  for (const duplicate of collectDuplicates(suites)) {
    issues.push(`duplicate shard suite: ${duplicate}`);
  }
  for (const duplicate of collectDuplicates(filters)) {
    issues.push(`duplicate shard filter: ${duplicate}`);
  }
  for (const [index, filter] of filters.entries()) {
    for (const sibling of filters.slice(index + 1)) {
      if (filter.includes(sibling) || sibling.includes(filter)) {
        issues.push(`overlapping shard filters: ${filter} and ${sibling}`);
      }
    }
  }

  if (!remainder) {
    issues.push("prodex-app shard manifest must contain a remainder shard");
  } else if (JSON.stringify(remainder.skipFilters) !== JSON.stringify(PRODEX_APP_LIB_FILTERS)) {
    issues.push("remainder shard must skip every targeted prodex-app filter in manifest order");
  }
  if (JSON.stringify(filters) !== JSON.stringify(PRODEX_APP_LIB_FILTERS)) {
    issues.push("targeted prodex-app filters must cover the manifest filter list exactly once");
  }

  return issues;
}

function matrixEntry(shard, index) {
  return {
    suite: shard.suite,
    label: shard.label,
    save_cache: index === 0,
    filters: shard.filters?.join("\n") ?? "",
    skip_filters: shard.skipFilters?.join("\n") ?? "",
  };
}

export function githubMatrix({ includeWorkspace = false } = {}) {
  const shards = includeWorkspace ? PRODEX_APP_FULL_TEST_SHARDS : PRODEX_APP_LIB_SHARDS;
  const include = shards.map((shard, index) =>
    matrixEntry(shard, includeWorkspace ? index + 1 : index),
  );
  if (includeWorkspace) {
    include.unshift({
      suite: WORKSPACE_SHARD.suite,
      label: WORKSPACE_SHARD.label,
      save_cache: true,
      filters: "",
      skip_filters: "",
    });
  }
  return { include };
}

export function ciGithubMatrix() {
  return {
    include: [...CI_TARGETED_SHARDS, CI_REMAINDER_SHARD].map(matrixEntry),
  };
}

export function windowsGithubMatrix() {
  const grouped = WINDOWS_SHARD_GROUPS.map((group) => ({
    suite: group.suite,
    label: group.label,
    filters: group.members.flatMap((suite) => {
      const shard = TARGETED_SHARDS.find((candidate) => candidate.suite === suite);
      if (!shard) throw new Error(`unknown Windows prodex-app shard: ${suite}`);
      return shard.filters;
    }),
  }));
  return {
    include: [...grouped, PRODEX_APP_LIB_SHARDS.at(-1)].map(matrixEntry),
  };
}

function stepCommand(shard) {
  if (shard.suite === "workspace") {
    return "node scripts/ci/full-rust-test.mjs --jobs 6 --test-threads 4 --timings --timings-json --no-prodex-app-lib";
  }
  if (shard.filters) {
    return shard.filters
      .map(
        (filter) =>
          `cargo test --locked -q -p prodex-app --lib --all-features '${filter}' -- --test-threads=1`,
      )
      .join(" && ");
  }
  return [
    "cargo test --locked -q -p prodex-app --lib --all-features -- --test-threads=1",
    ...shard.skipFilters.map((filter) => `--skip '${filter}'`),
  ].join(" ");
}

function parseArgs(argv) {
  const args = {};
  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--check") {
      args.check = true;
      continue;
    }
    if (value === "--dry-run") {
      args.dryRun = true;
      continue;
    }
    if (value === "--github-matrix") {
      args.githubMatrix = true;
      continue;
    }
    if (value === "--windows-github-matrix") {
      args.windowsGithubMatrix = true;
      continue;
    }
    if (value === "--full-test-matrix") {
      args.fullTestMatrix = true;
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
      "Usage: node scripts/ci/prodex-app-test-shards.mjs --check|--dry-run|--github-matrix|--windows-github-matrix|--full-test-matrix",
      "",
      "Validates or prints the shared prodex-app library CI shard plan.",
      "--github-matrix emits the push/PR app matrix without runtime-owned filters.",
      "--windows-github-matrix emits five grouped Windows partitions to avoid duplicate compilation.",
      "--full-test-matrix emits the scheduled full-test matrix including workspace coverage.",
    ].join("\n") + "\n",
  );
}

function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }
  const selectedModes = [
    args.check,
    args.dryRun,
    args.githubMatrix,
    args.windowsGithubMatrix,
    args.fullTestMatrix,
  ].filter(Boolean).length;
  if (selectedModes !== 1) {
    throw new Error(
      "choose exactly one of --check, --dry-run, --github-matrix, --windows-github-matrix, or --full-test-matrix",
    );
  }

  const issues = [...validateShards(), ...validateShards(PRODEX_APP_FULL_TEST_SHARDS)];
  if (issues.length > 0) {
    throw new Error(issues.join("\n"));
  }
  if (args.check) {
    process.stdout.write(`prodex-app-test-shards: ${PRODEX_APP_LIB_SHARDS.length} app shard(s), one cache writer\n`);
    return;
  }
  if (args.githubMatrix) {
    process.stdout.write(`${JSON.stringify(ciGithubMatrix())}\n`);
    return;
  }
  if (args.fullTestMatrix) {
    process.stdout.write(`${JSON.stringify(githubMatrix({ includeWorkspace: true }))}\n`);
    return;
  }
  if (args.windowsGithubMatrix) {
    process.stdout.write(`${JSON.stringify(windowsGithubMatrix())}\n`);
    return;
  }

  const shards = args.dryRun ? [WORKSPACE_SHARD, ...PRODEX_APP_FULL_TEST_SHARDS] : [];
  process.stdout.write(`dry-run: ${shards.length} full-test shard(s)\n`);
  for (const shard of shards) {
    process.stdout.write(`  ${shard.suite}: ${stepCommand(shard)}\n`);
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    main();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    process.stderr.write(`prodex-app-test-shards: ${message}\n`);
    process.exitCode = 1;
  }
}
