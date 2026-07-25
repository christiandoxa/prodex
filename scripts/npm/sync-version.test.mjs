import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import test from "node:test";

const execFileAsync = promisify(execFile);
const script = new URL("./sync-version.mjs", import.meta.url).pathname;

test("sync-version updates workspace package-lock metadata", async (t) => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-sync-version-"));
  t.after(() => fs.rm(root, { recursive: true, force: true }));
  await fs.mkdir(path.join(root, "npm", "prodex"), { recursive: true });
  await fs.writeFile(path.join(root, "Cargo.toml"), '[package]\nversion = "0.347.0"\n');
  await fs.writeFile(
    path.join(root, "npm", "prodex", "package.json"),
    JSON.stringify({
      name: "@christiandoxa/prodex",
      version: "0.346.0",
      optionalDependencies: { "@christiandoxa/prodex-linux-x64": "0.346.0" },
    }),
  );
  await fs.writeFile(
    path.join(root, "package-lock.json"),
    JSON.stringify({
      lockfileVersion: 3,
      packages: {
        "npm/prodex": {
          name: "@christiandoxa/prodex",
          version: "0.346.0",
          optionalDependencies: { "@christiandoxa/prodex-linux-x64": "0.346.0" },
        },
        "npm/platforms/linux-x64": {
          name: "@christiandoxa/prodex-linux-x64",
          version: "0.346.0",
        },
      },
    }),
  );

  await execFileAsync(process.execPath, [script, "--root", path.join(root, "npm")], {
    env: { ...process.env, PRODEX_REPO_ROOT: root },
  });

  const lock = JSON.parse(await fs.readFile(path.join(root, "package-lock.json"), "utf8"));
  assert.equal(lock.packages["npm/prodex"].version, "0.347.0");
  assert.equal(lock.packages["npm/platforms/linux-x64"].version, "0.347.0");
  assert.equal(
    lock.packages["npm/prodex"].optionalDependencies["@christiandoxa/prodex-linux-x64"],
    "0.347.0",
  );
});
