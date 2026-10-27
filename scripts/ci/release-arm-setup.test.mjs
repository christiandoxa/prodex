import assert from "node:assert/strict";
import { readFileSync, mkdtempSync, writeFileSync, mkdirSync, chmodSync, existsSync, rmSync } from "node:fs";
import { spawnSync } from "node:child_process";
import os from "node:os";
import path from "node:path";
import test from "node:test";
const workflow = readFileSync(".github/workflows/release-target.yml", "utf8");
const step = workflow.match(/      - name: Install Linux ARM cross and emulation tools\n([\s\S]*?)\n      - name: Build prodex/)[1];
const script = step.slice(step.indexOf("        run: |\n") + "        run: |\n".length)
  .split("\n").map((line) => line.startsWith("          ") ? line.slice(10) : line).join("\n");

test("ARM package setup uses authenticated official sources without touching runner mirror configuration", () => {
  assert.match(step, /timeout-minutes: 5/);
  assert.match(script, /https:\/\/archive.ubuntu.com\/ubuntu/);
  assert.match(script, /https:\/\/security.ubuntu.com\/ubuntu/);
  assert.match(script, /Signed-By: \/usr\/share\/keyrings\/ubuntu-archive-keyring.gpg/);
  assert.match(script, /Dir::Etc::sourceparts=-/);
  assert.match(script, /Acquire::Retries=2/);
  assert.match(script, /Acquire::https::Timeout=20/);
  assert.match(script, /install -y gcc-aarch64-linux-gnu qemu-user/);
  assert.doesNotMatch(script, /--allow-unauthenticated|trusted=yes|sed -i|--force-yes|\|\| true/);
});

test("ARM package setup is bounded and fail-closed, with temporary source cleanup", { skip: process.platform !== "linux" }, (t) => {
  const root = mkdtempSync(path.join(os.tmpdir(), "prodex-arm-setup-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const bin = path.join(root, "bin"); mkdirSync(bin);
  const sudo = `#!/bin/bash
set -eu
printf '%s\\n' "$*" >> "$APT_CALLS"
for argument in "$@"; do
  case "$argument" in
    Dir::Etc::sourcelist=*) source_file="${'$'}{argument#*=}"; cp "$source_file" "$SOURCE_CAPTURE"; printf '%s' "$source_file" > "$SOURCE_PATH" ;;
  esac
done
if [[ "${'$'}{FAIL_UPDATE:-}" == 1 && " $* " == *' update '* ]]; then exit 23; fi
`;
  writeFileSync(path.join(bin, "sudo"), sudo); chmodSync(path.join(bin, "sudo"), 0o700);
  for (const name of ["aarch64-linux-gnu-gcc", "qemu-aarch64"]) {
    writeFileSync(path.join(bin, name), "#!/bin/sh\necho synthetic-tool-version\n");
    chmodSync(path.join(bin, name), 0o700);
  }
  const env = { ...process.env, PATH: bin + path.delimiter + process.env.PATH,
    APT_CALLS: path.join(root, "calls"), SOURCE_CAPTURE: path.join(root, "sources"), SOURCE_PATH: path.join(root, "source-path") };
  const success = spawnSync("bash", ["-c", script], { encoding: "utf8", env });
  assert.equal(success.status, 0, success.stderr);
  const calls = readFileSync(env.APT_CALLS, "utf8").trim().split("\n");
  assert.equal(calls.length, 2);
  assert.match(calls[0], / update$/);
  assert.match(calls[1], /install -y gcc-aarch64-linux-gnu qemu-user$/);
  assert.match(readFileSync(env.SOURCE_CAPTURE, "utf8"), /Signed-By: \/usr\/share\/keyrings\/ubuntu-archive-keyring.gpg/);
  assert.equal(existsSync(readFileSync(env.SOURCE_PATH, "utf8")), false, "temporary apt source leaked");
  writeFileSync(env.APT_CALLS, "");
  const failure = spawnSync("bash", ["-c", script], { encoding: "utf8", env: { ...env, FAIL_UPDATE: "1" } });
  assert.equal(failure.status, 23, failure.stderr);
  assert.equal(readFileSync(env.APT_CALLS, "utf8").trim().split("\n").length, 1, "install ran after failed update");
  assert.equal(existsSync(readFileSync(env.SOURCE_PATH, "utf8")), false);
});
