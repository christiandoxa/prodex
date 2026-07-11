#!/usr/bin/env node
import { git } from "./guard-common.mjs";
import { GENERATED_METADATA_CHECK_PATHS } from "./test-impact-manifest.mjs";
import { repoRoot } from "../npm/common.mjs";

function parseArgs(argv) {
  const args = { paths: GENERATED_METADATA_CHECK_PATHS };
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

function printHelp() {
  process.stdout.write(
    [
      "Usage: node scripts/ci/generated-metadata-clean.mjs",
      "",
      "Fails when generated release/version metadata has uncommitted sync drift.",
      "The checked path set is owned by scripts/ci/test-impact-manifest.mjs.",
    ].join("\n") + "\n",
  );
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }

  await git(["diff", "--exit-code", "--", ...args.paths], { cwd: repoRoot });
  process.stdout.write(`generated-metadata-clean: ok (${args.paths.join(", ")})\n`);
}

try {
  await main();
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`generated-metadata-clean: ${message}\n`);
  process.exitCode = 1;
}
