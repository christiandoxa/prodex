#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { repoRoot } from "../npm/common.mjs";
import { assertReleaseAdvance, mergedVersionTagsInReleaseOrder } from "./release-lineage.mjs";

try {
  const argv = process.argv.slice(2);
  if (argv.length !== 4 || argv[0] !== "--version" || argv[2] !== "--head"
      || !/^[0-9a-f]{40}$/u.test(argv[3])) {
    throw new Error("usage: release-version-order.mjs --version X.Y.Z --head EXACT_SHA");
  }
  const previous = mergedVersionTagsInReleaseOrder(repoRoot, argv[3]).at(-1);
  const baseline = JSON.parse(fs.readFileSync(path.join(repoRoot, "scripts/compat/upstream-baseline.json"), "utf8"));
  assertReleaseAdvance(argv[1], previous, baseline.codex?.latestRelease?.tag_name);
  process.stdout.write(`release order verified: ${previous?.tag ?? "initial"} -> ${argv[1]}\n`);
} catch (error) {
  process.stderr.write(`release-version-order: ${error.message}\n`);
  process.exitCode = 1;
}
