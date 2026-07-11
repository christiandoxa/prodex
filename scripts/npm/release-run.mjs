#!/usr/bin/env node
import { pathToFileURL } from "node:url";
import { main } from "./release-run-lib.mjs";

export * from "./release-run-lib.mjs";

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  try {
    await main();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    process.stderr.write(`release-run: ${message}\n`);
    process.exitCode = 1;
  }
}
