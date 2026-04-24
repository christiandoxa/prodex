#!/usr/bin/env node
import {
  cargoTestStep,
  loadRuntimeManifest,
  manifestArray,
  runStepsSerial,
  skipArgs,
} from "./main-internal-test-runner.mjs";

const VALID_SUITES = new Set(["core", "runtime", "stress", "all"]);

function parseArgs(argv) {
  const args = {
    suite: "all",
    dryRun: false,
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
      "Usage: node scripts/ci/test-serial.mjs [--suite core|runtime|stress|all] [--dry-run]",
      "",
      "Runs local serial/quarantine cargo shards with --test-threads=1.",
      "",
      "Suites:",
      "  core    profile command internals and main_internal non-runtime-proxy tests",
      "  runtime runtime/global-state unit tests plus runtime proxy broad shard",
      "  stress  manifest-driven serialized and continuation-heavy runtime shards",
      "  all     core, runtime, and stress",
      "",
      "The runtime manifest is imported when available. Missing exports degrade to broader serial coverage or skipped stress re-runs with warnings.",
    ].join("\n") + "\n",
  );
}

function coreSteps(stressSkipTests) {
  return [
    cargoTestStep("serial:profile-commands", "profile_commands_internal_tests::"),
    cargoTestStep("serial:main-internal-core", "main_internal_tests::", [
      "--skip",
      "runtime_proxy_",
      ...skipArgs(stressSkipTests),
    ]),
  ];
}

function runtimeSteps(stressSkipTests) {
  return [
    cargoTestStep("serial:runtime-proxy-units", "runtime_proxy::"),
    cargoTestStep("serial:runtime-broker-units", "runtime_broker::"),
    cargoTestStep("serial:runtime-core-shared-units", "runtime_core_shared::"),
    cargoTestStep("serial:main-internal-runtime-proxy-broad", "main_internal_tests::runtime_proxy_", [
      ...skipArgs(stressSkipTests),
    ]),
  ];
}

function stressSteps(serializedTests, continuationTests) {
  const serializedSteps = serializedTests.map((testName) => ({
    ...cargoTestStep(`serial:stress:${testName}`, testName),
    attempts: 2,
    retryDelayMs: 5000,
  }));
  const continuationSteps = [];
  for (let iteration = 1; iteration <= 2; iteration += 1) {
    for (const testName of continuationTests) {
      continuationSteps.push(cargoTestStep(`serial:continuation:${iteration}:${testName}`, testName));
    }
  }
  return [...serializedSteps, ...continuationSteps];
}

function selectSteps({ suite, core, runtime, stress }) {
  if (suite === "core") {
    return core;
  }
  if (suite === "runtime") {
    return runtime;
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

  const steps = selectSteps({
    suite: args.suite,
    core: coreSteps(stressSkipTests),
    runtime: runtimeSteps(stressSkipTests),
    stress: stressSteps(serializedTests, continuationTests),
  });

  if (steps.length === 0) {
    process.stdout.write(`test-serial: no steps for suite ${args.suite}\n`);
    return;
  }

  await runStepsSerial(steps, { dryRun: args.dryRun });
}

try {
  await main();
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`test-serial: ${message}\n`);
  process.exitCode = 1;
}
