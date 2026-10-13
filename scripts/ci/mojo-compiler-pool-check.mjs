#!/usr/bin/env node
// Build-script policy tests run outside Cargo's normal crate test suites.
// Compile the actual build.rs test harness: no fake source-level assertions.
import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = mkdtempSync(join(tmpdir(), "prodex-mojo-compiler-pool-check-"));
const buildScript = fileURLToPath(new URL("../../crates/prodex-mojo-core/build.rs", import.meta.url));
const binary = join(root, process.platform === "win32" ? "mojo-pool-tests.exe" : "mojo-pool-tests");

function checked(command, args) {
  const result = spawnSync(command, args, {
    encoding: "utf8",
    timeout: 30_000,
  });
  if (result.stdout) process.stdout.write(result.stdout);
  if (result.stderr) process.stderr.write(result.stderr);
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(command + " failed: exit=" + result.status + " signal=" + result.signal);
  }
}

try {
  checked("rustc", ["--edition=2024", "--test", buildScript, "-o", binary]);
  checked(binary, ["--test-threads=1"]);
} finally {
  rmSync(root, { recursive: true, force: true });
}
