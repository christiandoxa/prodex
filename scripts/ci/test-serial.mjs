#!/usr/bin/env node
import {
  cargoFeatureArgs,
  cargoTestStep,
  loadRuntimeManifest,
  manifestArray,
  parsePositiveInteger,
  runStepsSerial,
  skipArgs,
} from "./main-internal-test-runner.mjs";

const VALID_SUITES = new Set(["core", "runtime", "runtime-smoke", "stress", "all"]);

function parseArgs(argv) {
  const args = {
    suite: "all",
    dryRun: false,
    timings: false,
    timingsJson: false,
    timingsLimit: 10,
    allFeatures: false,
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--suite") {
      index += 1;
      if (!argv[index]) {
        throw new Error("--suite requires a value");
      }
      args.suite = argv[index];
      continue;
    }
    if (value === "--dry-run") {
      args.dryRun = true;
      continue;
    }
    if (value === "--all-features") {
      args.allFeatures = true;
      continue;
    }
    if (value === "--timings") {
      args.timings = true;
      continue;
    }
    if (value === "--timings-json") {
      args.timings = true;
      args.timingsJson = true;
      continue;
    }
    if (value === "--timings-limit") {
      index += 1;
      if (!argv[index]) {
        throw new Error("--timings-limit requires a value");
      }
      args.timingsLimit = parsePositiveInteger(argv[index], "--timings-limit");
      continue;
    }
    if (value === "--help" || value === "-h") {
      args.help = true;
      continue;
    }
    throw new Error(`unknown argument: ${value}`);
  }

  if (!args.help && !VALID_SUITES.has(args.suite)) {
    throw new Error(`invalid --suite value: ${args.suite}. Expected one of: ${Array.from(VALID_SUITES).join(", ")}`);
  }

  return args;
}

function printHelp() {
  process.stdout.write(
    [
      "Usage: node scripts/ci/test-serial.mjs [--suite core|runtime|runtime-smoke|stress|all] [--all-features] [--timings] [--timings-json] [--timings-limit <n>] [--dry-run]",
      "",
      "Runs local serial/quarantine cargo shards with --test-threads=1.",
      "",
      "Suites:",
      "  core    profile command internals and main_internal non-runtime-proxy tests",
      "  runtime runtime/global-state unit tests plus runtime proxy broad shard",
      "  runtime-smoke curated local runtime invariant checks",
      "  stress  manifest-driven serialized and continuation-heavy runtime shards",
      "  all     core, runtime, and stress",
      "",
      "Options:",
      "  --all-features run cargo test shards with --all-features",
      "  --timings      print a slowest-step duration summary after the selected suite",
      "  --timings-json include a single-line JSON timing payload in the summary",
      "",
      "The runtime manifest is imported when available. Missing exports degrade to broader serial coverage or skipped stress re-runs with warnings.",
    ].join("\n") + "\n",
  );
}

function coreSteps(stressSkipTests, options) {
  return [
    cargoTestStep("serial:profile-commands", "profile_commands_internal_tests::", [], options),
    cargoTestStep("serial:main-internal-core", "main_internal_tests::", [
      "--skip",
      "runtime_proxy_",
      ...skipArgs(stressSkipTests),
    ], options),
  ];
}

function runtimeSteps(stressSkipTests, options) {
  return [
    cargoTestStep("serial:runtime-proxy-units", "runtime_proxy::", [], options),
    cargoTestStep("serial:runtime-broker-units", "runtime_broker::", [], options),
    cargoTestStep("serial:runtime-core-shared-units", "runtime_core_shared::", [], options),
    cargoTestStep("serial:main-internal-runtime-proxy-broad", "main_internal_tests::runtime_proxy_", [
      ...skipArgs(stressSkipTests),
    ], options),
  ];
}

function runtimeSmokeSteps(smokeTests, options) {
  if (smokeTests.length === 0) {
    throw new Error("runtime smoke suite requires RUNTIME_SMOKE_TESTS manifest entries");
  }

  return smokeTests.map((testCase) => {
    if (!testCase || typeof testCase !== "object" || Array.isArray(testCase)) {
      throw new Error("runtime smoke manifest entries must be objects");
    }
    if (!testCase.filter || typeof testCase.filter !== "string") {
      throw new Error("runtime smoke manifest entries need a string filter");
    }
    if (testCase.package !== undefined && typeof testCase.package !== "string") {
      throw new Error("runtime smoke manifest package must be a string when provided");
    }
    const label = testCase.label ?? testCase.filter;
    if (testCase.package) {
      return {
        label: `serial:runtime-smoke:${label}`,
        command: "cargo",
        args: [
          "test",
          "-p",
          testCase.package,
          ...cargoFeatureArgs(options),
          "--lib",
          testCase.filter,
          "--",
          "--test-threads=1",
        ],
        failOnZeroTests: true,
      };
    }
    return cargoTestStep(`serial:runtime-smoke:${label}`, testCase.filter, [], options);
  });
}

function stressSteps(serializedTests, continuationTests, options) {
  const serializedSteps = serializedTests.map((testName) => ({
    ...cargoTestStep(`serial:stress:${testName}`, testName, [], options),
    attempts: 2,
    retryDelayMs: 5000,
  }));
  const continuationSteps = [];
  for (let iteration = 1; iteration <= 2; iteration += 1) {
    for (const testName of continuationTests) {
      continuationSteps.push(cargoTestStep(`serial:continuation:${iteration}:${testName}`, testName, [], options));
    }
  }
  return [...serializedSteps, ...continuationSteps];
}

function selectSteps({ suite, core, runtime, runtimeSmoke, stress }) {
  if (suite === "core") {
    return core;
  }
  if (suite === "runtime") {
    return runtime;
  }
  if (suite === "runtime-smoke") {
    return runtimeSmoke;
  }
  if (suite === "stress") {
    return stress;
  }
  return [...core, ...runtime, ...stress];
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }

  const manifest = await loadRuntimeManifest();
  const stressSkipTests = manifestArray(manifest, "RUNTIME_STRESS_SKIP_TESTS", []);
  const serializedTests = manifestArray(manifest, "RUNTIME_STRESS_SERIALIZED_TESTS", []);
  const continuationTests = manifestArray(manifest, "RUNTIME_STRESS_CONTINUATION_TESTS", []);
  const smokeTests = manifestArray(manifest, "RUNTIME_SMOKE_TESTS", []);

  const steps = selectSteps({
    suite: args.suite,
    core: coreSteps(stressSkipTests, args),
    runtime: runtimeSteps(stressSkipTests, args),
    runtimeSmoke: args.suite === "runtime-smoke" ? runtimeSmokeSteps(smokeTests, args) : [],
    stress: stressSteps(serializedTests, continuationTests, args),
  });

  if (steps.length === 0) {
    process.stdout.write(`test-serial: no steps for suite ${args.suite}\n`);
    return;
  }

  await runStepsSerial(steps, {
    dryRun: args.dryRun,
    timingSummary: args.timings
      ? { label: `test-serial:${args.suite}`, limit: args.timingsLimit, json: args.timingsJson }
      : null,
  });
}

try {
  await main();
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`test-serial: ${message}\n`);
  process.exitCode = 1;
}
