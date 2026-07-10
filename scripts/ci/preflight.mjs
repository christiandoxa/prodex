#!/usr/bin/env node
import {
  defaultJobCount,
  parsePositiveInteger,
  runStepsSerial,
} from "./main-internal-test-runner.mjs";
import { fileURLToPath } from "node:url";

const modulePath = fileURLToPath(import.meta.url);

export function parseArgs(argv) {
  const args = {
    churnCheck: process.env.PRODEX_PREFLIGHT_CHURN_REPORT_ONLY !== "1",
    churnIgnoreBefore: process.env.PRODEX_PREFLIGHT_CHURN_IGNORE_BEFORE || null,
    churnRange: process.env.PRODEX_PREFLIGHT_CHURN_RANGE || null,
    dryRun: false,
    jobs: defaultJobCount(),
    fastTests: true,
    serial: false,
    storagePostgresProof:
      process.env.PRODEX_PREFLIGHT_STORAGE_POSTGRES_PROOF === "1",
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--dry-run") {
      args.dryRun = true;
      continue;
    }
    if (value === "--churn-check") {
      args.churnCheck = true;
      continue;
    }
    if (value === "--no-churn-check") {
      args.churnCheck = false;
      continue;
    }
    if (value === "--churn-report-only") {
      args.churnCheck = false;
      continue;
    }
    if (value === "--churn-ignore-before") {
      index += 1;
      if (!argv[index]) {
        throw new Error(`${value} requires a value`);
      }
      args.churnIgnoreBefore = argv[index];
      continue;
    }
    if (value === "--churn-range") {
      index += 1;
      if (!argv[index]) {
        throw new Error(`${value} requires a value`);
      }
      args.churnRange = argv[index];
      continue;
    }
    if (value === "--jobs" || value === "-j") {
      index += 1;
      if (!argv[index]) {
        throw new Error(`${value} requires a value`);
      }
      args.jobs = parsePositiveInteger(argv[index], value);
      continue;
    }
    if (value === "--no-tests") {
      args.fastTests = false;
      continue;
    }
    if (value === "--serial") {
      args.serial = true;
      continue;
    }
    if (value === "--storage-postgres-proof") {
      args.storagePostgresProof = true;
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
      "Usage: node scripts/ci/preflight.mjs [--jobs <n>] [--no-tests] [--serial] [--storage-postgres-proof] [--churn-check|--churn-report-only] [--churn-range <range>] [--churn-ignore-before <rev>] [--dry-run]",
      "",
      "Runs the practical local preflight gate before pushing.",
      "",
      "Default checks:",
      "  - release hygiene, Rust size/allow/super-wildcard guards, crate boundaries, runtime hot-path, churn hygiene, manifest-owned version sync",
      "  - docs lint, upstream baseline, runtime manifest, fmt, cargo check",
      "  - cargo clippy --locked --all-targets --all-features -- -D warnings",
      "  - npm run test:fast -- --tests-only --no-prebuild",
      "",
      "Churn hygiene fails locally and in CI by default. Use --churn-report-only or PRODEX_PREFLIGHT_CHURN_REPORT_ONLY=1 for exploratory dry runs.",
      "",
      "Options:",
      "  --jobs <n>        set test:fast child process parallelism",
      "  --serial          also run npm run test:serial -- --suite all",
      "  --no-tests        skip test:fast; useful when only checking metadata/clippy",
      "  --storage-postgres-proof  also run npm run ci:storage-postgres-proof (or set PRODEX_PREFLIGHT_STORAGE_POSTGRES_PROOF=1)",
      "  --churn-check        fail when churn hygiene thresholds are exceeded; default unless report-only env is set",
      "  --churn-report-only  force churn hygiene report-only mode",
      "  --no-churn-check     deprecated alias for --churn-report-only",
      "  --churn-range <range>  pass an explicit git range to churn hygiene",
      "  --churn-ignore-before <rev>  pass a reviewed historical baseline to churn hygiene; latest-tag is supported",
      "  --dry-run         print the command plan without running it",
    ].join("\n") + "\n",
  );
}

export function preflightSteps(args) {
  const churnArgs = ["scripts/ci/churn-hygiene.mjs", ...(args.churnCheck ? ["--check"] : ["--report-only"])];
  if (args.churnRange) {
    churnArgs.push("--range", args.churnRange);
  }
  if (args.churnIgnoreBefore) {
    churnArgs.push("--ignore-before", args.churnIgnoreBefore);
  }
  const steps = [
    {
      label: "release-hygiene",
      command: "node",
      args: ["scripts/ci/release-hygiene.mjs"],
    },
    {
      label: "size-guard",
      command: "node",
      args: ["scripts/ci/size-guard.mjs"],
    },
    {
      label: "allow-attribute-guard",
      command: "node",
      args: ["scripts/ci/allow-attribute-guard.mjs"],
    },
    {
      label: "super-wildcard-guard",
      command: "node",
      args: ["scripts/ci/super-wildcard-guard.mjs"],
    },
    {
      label: "env-mutation-guard",
      command: "node",
      args: ["scripts/ci/env-mutation-guard.mjs"],
    },
    {
      label: "enterprise-docs-guard-self-test",
      command: "node",
      args: ["scripts/ci/enterprise-docs-guard.mjs", "--self-test"],
    },
    {
      label: "enterprise-docs-guard",
      command: "node",
      args: ["scripts/ci/enterprise-docs-guard.mjs"],
    },
    {
      label: "enterprise-id-boundary-guard-self-test",
      command: "node",
      args: ["scripts/ci/enterprise-id-boundary-guard.mjs", "--self-test"],
    },
    {
      label: "enterprise-id-boundary-guard",
      command: "node",
      args: ["scripts/ci/enterprise-id-boundary-guard.mjs"],
    },
    {
      label: "enterprise-binaries-guard-self-test",
      command: "node",
      args: ["scripts/ci/enterprise-binaries-guard.mjs", "--self-test"],
    },
    {
      label: "enterprise-binaries-guard",
      command: "node",
      args: ["scripts/ci/enterprise-binaries-guard.mjs"],
    },
    {
      label: "crate-boundary-guard-self-test",
      command: "node",
      args: ["scripts/ci/crate-boundary-guard.mjs", "--self-test"],
    },
    {
      label: "crate-boundary-guard",
      command: "node",
      args: ["scripts/ci/crate-boundary-guard.mjs"],
    },
    {
      label: "domain-boundary-guard-self-test",
      command: "node",
      args: ["scripts/ci/domain-boundary-guard.mjs", "--self-test"],
    },
    {
      label: "domain-boundary-guard",
      command: "node",
      args: ["scripts/ci/domain-boundary-guard.mjs"],
    },
    {
      label: "application-boundary-guard-self-test",
      command: "node",
      args: ["scripts/ci/application-boundary-guard.mjs", "--self-test"],
    },
    {
      label: "application-boundary-guard",
      command: "node",
      args: ["scripts/ci/application-boundary-guard.mjs"],
    },
    {
      label: "control-plane-boundary-guard-self-test",
      command: "node",
      args: ["scripts/ci/control-plane-boundary-guard.mjs", "--self-test"],
    },
    {
      label: "control-plane-boundary-guard",
      command: "node",
      args: ["scripts/ci/control-plane-boundary-guard.mjs"],
    },
    {
      label: "auth-boundary-guard-self-test",
      command: "node",
      args: ["scripts/ci/auth-boundary-guard.mjs", "--self-test"],
    },
    {
      label: "auth-boundary-guard",
      command: "node",
      args: ["scripts/ci/auth-boundary-guard.mjs"],
    },
    {
      label: "config-boundary-guard-self-test",
      command: "node",
      args: ["scripts/ci/config-boundary-guard.mjs", "--self-test"],
    },
    {
      label: "config-boundary-guard",
      command: "node",
      args: ["scripts/ci/config-boundary-guard.mjs"],
    },
    {
      label: "observability-boundary-guard-self-test",
      command: "node",
      args: ["scripts/ci/observability-boundary-guard.mjs", "--self-test"],
    },
    {
      label: "observability-boundary-guard",
      command: "node",
      args: ["scripts/ci/observability-boundary-guard.mjs"],
    },
    {
      label: "provider-spi-boundary-guard-self-test",
      command: "node",
      args: ["scripts/ci/provider-spi-boundary-guard.mjs", "--self-test"],
    },
    {
      label: "provider-spi-boundary-guard",
      command: "node",
      args: ["scripts/ci/provider-spi-boundary-guard.mjs"],
    },
    {
      label: "storage-boundary-guard-self-test",
      command: "node",
      args: ["scripts/ci/storage-boundary-guard.mjs", "--self-test"],
    },
    {
      label: "storage-boundary-guard",
      command: "node",
      args: ["scripts/ci/storage-boundary-guard.mjs"],
    },
    {
      label: "storage-postgres-boundary-guard-self-test",
      command: "node",
      args: ["scripts/ci/storage-postgres-boundary-guard.mjs", "--self-test"],
    },
    {
      label: "storage-postgres-boundary-guard",
      command: "node",
      args: ["scripts/ci/storage-postgres-boundary-guard.mjs"],
    },
    {
      label: "storage-redis-boundary-guard-self-test",
      command: "node",
      args: ["scripts/ci/storage-redis-boundary-guard.mjs", "--self-test"],
    },
    {
      label: "storage-redis-boundary-guard",
      command: "node",
      args: ["scripts/ci/storage-redis-boundary-guard.mjs"],
    },
    {
      label: "storage-sqlite-boundary-guard-self-test",
      command: "node",
      args: ["scripts/ci/storage-sqlite-boundary-guard.mjs", "--self-test"],
    },
    {
      label: "storage-sqlite-boundary-guard",
      command: "node",
      args: ["scripts/ci/storage-sqlite-boundary-guard.mjs"],
    },
    {
      label: "gateway-core-boundary-guard-self-test",
      command: "node",
      args: ["scripts/ci/gateway-core-boundary-guard.mjs", "--self-test"],
    },
    {
      label: "gateway-core-boundary-guard",
      command: "node",
      args: ["scripts/ci/gateway-core-boundary-guard.mjs"],
    },
    {
      label: "gateway-http-boundary-guard-self-test",
      command: "node",
      args: ["scripts/ci/gateway-http-boundary-guard.mjs", "--self-test"],
    },
    {
      label: "gateway-http-boundary-guard",
      command: "node",
      args: ["scripts/ci/gateway-http-boundary-guard.mjs"],
    },
    {
      label: "deployment-security-guard-self-test",
      command: "node",
      args: ["scripts/ci/deployment-security-guard.mjs", "--self-test"],
    },
    {
      label: "deployment-security-guard",
      command: "node",
      args: ["scripts/ci/deployment-security-guard.mjs"],
    },
    {
      label: "runtime-hotpath-guard-self-test",
      command: "node",
      args: ["scripts/ci/runtime-hotpath-guard.mjs", "--self-test"],
    },
    {
      label: "runtime-hotpath-guard",
      command: "node",
      args: ["scripts/ci/runtime-hotpath-guard.mjs"],
    },
    {
      label: args.churnCheck ? "churn-hygiene-check" : "churn-hygiene-report",
      command: "node",
      args: churnArgs,
    },
    {
      label: "metadata-preflight",
      command: "node",
      args: ["scripts/npm/release-prepare.mjs", "--no-cargo-test", "--ci-changelog-check"],
    },
    {
      label: "generated-metadata-clean",
      command: "node",
      args: ["scripts/ci/generated-metadata-clean.mjs"],
    },
    {
      label: "clippy",
      command: "cargo",
      args: ["clippy", "--locked", "--all-targets", "--all-features", "--", "-D", "warnings"],
    },
  ];

  if (args.storagePostgresProof) {
    steps.push({
      label: "storage-postgres-proof",
      command: "npm",
      args: ["run", "ci:storage-postgres-proof"],
    });
  }

  if (args.fastTests) {
    steps.push({
      label: "test-fast",
      command: "node",
      args: ["scripts/ci/test-fast.mjs", "--tests-only", "--no-prebuild", "--jobs", String(args.jobs)],
    });
  }

  if (args.serial) {
    steps.push({
      label: "test-serial",
      command: "node",
      args: ["scripts/ci/test-serial.mjs", "--suite", "all"],
    });
  }

  return steps;
}

export async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }

  await runStepsSerial(preflightSteps(args), { dryRun: args.dryRun });
}

if (process.argv[1] && process.argv[1] === modulePath) {
  try {
    await main();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    process.stderr.write(`preflight: ${message}\n`);
    process.exitCode = 1;
  }
}
