import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, existsSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";

const script = resolve("scripts/ci/ensure-llvm-archive-tools.sh");
const bash = spawnSync("sh", ["-c", "command -v bash"], { encoding: "utf8" }).stdout.trim();
function fixture(run) {
  const root = mkdtempSync(join(tmpdir(), "prodex-llvm-tools-"));
  const bin = join(root, "path");
  const search = join(root, "installed");
  mkdirSync(bin); mkdirSync(search);
  const output = join(root, "github-path");
  const calls = join(root, "sudo-calls");
  const executable = (directory, name, contents = "#!/bin/sh\nexit 0\n") =>
    writeFileSync(join(directory, name), contents, { mode: 0o700 });
  const execute = (extra = {}) => spawnSync(bash, [script, search], {
    encoding: "utf8", env: { PATH: bin, GITHUB_PATH: output, ...extra }, timeout: 5_000,
  });
  try { run({ root, bin, search, output, calls, executable, execute }); }
  finally { rmSync(root, { recursive: true, force: true }); }
}
test("LLVM already on PATH avoids package installation", () => fixture((f) => {
  for (const name of ["llvm-ar", "llvm-nm", "llvm-lib"]) f.executable(f.bin, name);
  const result = f.execute();
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /already available/);
  assert.equal(existsSync(f.output), false);
}));
test("complete versioned LLVM installation is exposed without sudo", () => fixture((f) => {
  const incomplete = join(f.search, "llvm-17", "bin");
  const directory = join(f.search, "llvm-18", "bin");
  mkdirSync(incomplete, { recursive: true }); mkdirSync(directory, { recursive: true });
  f.executable(incomplete, "llvm-nm");
  for (const name of ["llvm-ar", "llvm-nm", "llvm-lib"]) f.executable(directory, name);
  const result = f.execute();
  assert.equal(result.status, 0, result.stderr);
  assert.equal(readFileSync(f.output, "utf8"), directory + "\n");
  assert.match(result.stdout, /Reusing installed/);
}));
test("missing tools retain checked apt fallback and never accept an incomplete install", () => fixture((f) => {
  f.executable(f.bin, "sudo", `#!/bin/sh
printf '%s\n' "$*" >> '${f.calls}'
exit 0
`);
  const result = f.execute();
  assert.notEqual(result.status, 0, "successful apt exit alone does not prove required binaries exist");
  assert.equal(readFileSync(f.calls, "utf8"), "apt-get update\napt-get install -y llvm\n");
}));
test("failure to expose versioned tools fails closed instead of pretending setup succeeded", () => fixture((f) => {
  const directory = join(f.search, "llvm-18", "bin"); mkdirSync(directory, { recursive: true });
  for (const name of ["llvm-ar", "llvm-nm", "llvm-lib"]) f.executable(directory, name);
  const result = f.execute({ GITHUB_PATH: "" });
  assert.equal(result.status, 2);
  assert.match(result.stderr, /GITHUB_PATH is required/);
}));
