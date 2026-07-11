import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { promisify } from "node:util";

const execFileAsync = promisify(execFile);
const SCRIPT_PATH = new URL("./publish.mjs", import.meta.url).pathname;

async function writePackage(root, dirName, name, version) {
  const dir = path.join(root, "packages", dirName);
  await fs.mkdir(dir, { recursive: true });
  await fs.writeFile(
    path.join(dir, "package.json"),
    `${JSON.stringify({ name, version }, null, 2)}\n`,
  );
}

async function writeFakeNpm(binDir) {
  const npmPath = path.join(binDir, "npm");
  await fs.mkdir(binDir, { recursive: true });
  await fs.writeFile(
    npmPath,
    `#!/usr/bin/env node
const fs = require("node:fs");
const path = require("node:path");

const args = process.argv.slice(2);
const published = new Set((process.env.PUBLISHED_SPECS || "").split(",").filter(Boolean));
const logPath = process.env.NPM_CALL_LOG;

if (args[0] === "view" && args[2] === "version") {
  const spec = args[1];
  if (published.has(spec)) {
    const version = spec.slice(spec.lastIndexOf("@") + 1);
    console.log(version);
    process.exit(0);
  }
  console.error("npm ERR! code E404");
  console.error("npm ERR! 404 Not Found");
  process.exit(1);
}

if (args[0] === "publish") {
  const packageJson = JSON.parse(fs.readFileSync(path.join(process.cwd(), "package.json"), "utf8"));
  fs.appendFileSync(logPath, \`publish \${packageJson.name}@\${packageJson.version} \${args.slice(1).join(" ")}\\n\`);
  process.exit(0);
}

console.error(\`unexpected npm args: \${args.join(" ")}\`);
process.exit(2);
`,
  );
  await fs.chmod(npmPath, 0o755);
  return npmPath;
}

test("publish skips package versions that already exist in npm", async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-publish-"));
  try {
    const staging = path.join(root, "release");
    const binDir = path.join(root, "bin");
    const logPath = path.join(root, "npm-calls.log");
    await writePackage(
      staging,
      "linux-x64",
      "@christiandoxa/prodex-linux-x64",
      "1.2.3",
    );
    await writePackage(
      staging,
      "win32-x64",
      "@christiandoxa/prodex-win32-x64",
      "1.2.3",
    );
    await writeFakeNpm(binDir);

    const { stdout } = await execFileAsync(
      process.execPath,
      [SCRIPT_PATH, "--root", staging, "--provenance"],
      {
        cwd: root,
        env: {
          ...process.env,
          PATH: `${binDir}${path.delimiter}${process.env.PATH}`,
          NPM_CALL_LOG: logPath,
          PUBLISHED_SPECS: "@christiandoxa/prodex-linux-x64@1.2.3",
        },
      },
    );

    assert.match(
      stdout,
      /skipping @christiandoxa\/prodex-linux-x64@1\.2\.3; already published/,
    );
    assert.match(stdout, /publishing @christiandoxa\/prodex-win32-x64@1\.2\.3/);
    const log = await fs.readFile(logPath, "utf8");
    assert.equal(
      log,
      "publish @christiandoxa/prodex-win32-x64@1.2.3 --access public --provenance\n",
    );
  } finally {
    await fs.rm(root, { recursive: true, force: true });
  }
});
