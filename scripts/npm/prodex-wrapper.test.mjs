import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";
import {
  copyRepoFile,
  ensureDir,
  mainPackageManifest,
  platformPackages,
  platformPackageManifest,
  writeJsonFile,
} from "./common.mjs";

function packageInstallDir(root, packageName) {
  return path.join(root, "node_modules", ...packageName.split("/"));
}

function cleanEnv(overrides = {}) {
  const env = { ...process.env, ...overrides };
  for (const [key, value] of Object.entries(env)) {
    if (value === undefined) {
      delete env[key];
    }
  }
  return env;
}

function hostOpenAiCodexPlatformSpec() {
  const specs = {
    linux: {
      x64: {
        packageName: "@openai/codex-linux-x64",
        targetTriple: "x86_64-unknown-linux-musl",
        binaryFileName: "codex",
      },
      arm64: {
        packageName: "@openai/codex-linux-arm64",
        targetTriple: "aarch64-unknown-linux-musl",
        binaryFileName: "codex",
      },
    },
    darwin: {
      x64: {
        packageName: "@openai/codex-darwin-x64",
        targetTriple: "x86_64-apple-darwin",
        binaryFileName: "codex",
      },
      arm64: {
        packageName: "@openai/codex-darwin-arm64",
        targetTriple: "aarch64-apple-darwin",
        binaryFileName: "codex",
      },
    },
    win32: {
      x64: {
        packageName: "@openai/codex-win32-x64",
        targetTriple: "x86_64-pc-windows-msvc",
        binaryFileName: "codex.exe",
      },
      arm64: {
        packageName: "@openai/codex-win32-arm64",
        targetTriple: "aarch64-pc-windows-msvc",
        binaryFileName: "codex.exe",
      },
    },
  };
  return specs[process.platform]?.[process.arch] ?? null;
}

async function writeExecutable(filePath, contents) {
  await ensureDir(path.dirname(filePath));
  await fs.writeFile(filePath, contents);
  await fs.chmod(filePath, 0o755);
}

async function stageCodexShimInstall() {
  if (process.platform === "win32") {
    return null;
  }
  const spec = hostOpenAiCodexPlatformSpec();
  if (!spec) {
    return null;
  }

  const root = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-codex-shim-test-"));
  const mainPackageDir = packageInstallDir(root, "@christiandoxa/prodex");
  await ensureDir(path.join(mainPackageDir, "lib"));
  await copyRepoFile(
    "npm/prodex/lib/codex-shim.cjs",
    path.join(mainPackageDir, "lib", "codex-shim.cjs"),
  );

  const openAiCodexDir = packageInstallDir(root, "@openai/codex");
  await ensureDir(path.join(openAiCodexDir, "bin"));
  await writeJsonFile(path.join(openAiCodexDir, "package.json"), {
    name: "@openai/codex",
    version: "0.0.0-test",
    bin: {
      codex: "bin/codex.js",
    },
  });
  await writeExecutable(
    path.join(openAiCodexDir, "bin", "codex.js"),
    ["#!/usr/bin/env node", "throw new Error('codex js fallback should not run');", ""].join(
      "\n",
    ),
  );

  const platformPackageDir = packageInstallDir(root, spec.packageName);
  const nativeBinaryPath = path.join(
    platformPackageDir,
    "vendor",
    spec.targetTriple,
    "bin",
    spec.binaryFileName,
  );
  await writeJsonFile(path.join(platformPackageDir, "package.json"), {
    name: spec.packageName,
    version: "0.0.0-test",
  });
  await writeExecutable(
    nativeBinaryPath,
    [
      "#!/usr/bin/env node",
      "console.log(JSON.stringify({",
      "  argv: process.argv.slice(2),",
      "  managedByNpm: process.env.CODEX_MANAGED_BY_NPM || null,",
      "  managedPackageRoot: process.env.CODEX_MANAGED_PACKAGE_ROOT || null,",
      "}));",
      "",
    ].join("\n"),
  );

  return {
    root,
    shimPath: path.join(mainPackageDir, "lib", "codex-shim.cjs"),
    nativeBinaryPath,
    platformPackageDir,
    openAiCodexDir,
  };
}

async function stageWrapperInstall(version) {
  if (process.platform === "win32") {
    return null;
  }

  const spec = platformPackages.find(
    (entry) => entry.os === process.platform && entry.cpu === process.arch,
  );
  assert.ok(spec, `unsupported test platform ${process.platform} ${process.arch}`);

  const root = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-wrapper-test-"));
  const mainPackageDir = packageInstallDir(root, "@christiandoxa/prodex");
  await ensureDir(path.join(mainPackageDir, "lib"));
  await copyRepoFile("npm/prodex/prodex", path.join(mainPackageDir, "prodex"));
  await copyRepoFile(
    "npm/prodex/lib/codex-shim.cjs",
    path.join(mainPackageDir, "lib", "codex-shim.cjs"),
  );
  await fs.chmod(path.join(mainPackageDir, "prodex"), 0o755);
  await fs.chmod(path.join(mainPackageDir, "lib", "codex-shim.cjs"), 0o755);
  await writeJsonFile(path.join(mainPackageDir, "package.json"), mainPackageManifest(version));

  const platformPackageDir = packageInstallDir(root, spec.packageName);
  await ensureDir(path.join(platformPackageDir, "vendor"));
  await writeJsonFile(
    path.join(platformPackageDir, "package.json"),
    platformPackageManifest(spec, version),
  );
  await writeExecutable(
    path.join(platformPackageDir, "vendor", spec.binaryFileName),
    [
      "#!/usr/bin/env node",
      "console.log(JSON.stringify({",
      "  codexBin: process.env.PRODEX_CODEX_BIN || null,",
      "  pathEntries: (process.env.PATH || '').split(require('node:path').delimiter),",
      "}));",
      "",
    ].join("\n"),
  );

  const externalDir = path.join(root, "external-bin");
  const externalCodex = path.join(externalDir, "codex");
  await writeExecutable(
    externalCodex,
    ["#!/bin/sh", "echo external codex should not be executed", "exit 77", ""].join("\n"),
  );

  return {
    root,
    launcherPath: path.join(mainPackageDir, "prodex"),
    externalDir,
    externalCodex,
  };
}

function runWrapper(install, env) {
  const result = spawnSync(process.execPath, [install.launcherPath, "--probe"], {
    encoding: "utf8",
    env,
  });
  assert.equal(result.status, 0, result.stderr || result.stdout);
  return JSON.parse(result.stdout);
}

test("prodex npm wrapper uses bundled Codex shim by default", async (t) => {
  const install = await stageWrapperInstall("0.0.0-test");
  if (!install) {
    t.skip("wrapper fake native binary test is only implemented for POSIX runners");
    return;
  }
  t.after(() => fs.rm(install.root, { recursive: true, force: true }));

  const output = runWrapper(
    install,
    cleanEnv({
      PATH: `${install.externalDir}${path.delimiter}${process.env.PATH || ""}`,
      PRODEX_CODEX_BIN: undefined,
      PRODEX_CODEX_RESOLUTION: undefined,
    }),
  );

  assert.equal(output.codexBin, "codex");
  assert.match(output.pathEntries[0], /prodex-codex-/);
  assert.notEqual(output.pathEntries[0], install.externalDir);
});

test("prodex npm wrapper can opt into external Codex explicitly", async (t) => {
  const install = await stageWrapperInstall("0.0.0-test");
  if (!install) {
    t.skip("wrapper fake native binary test is only implemented for POSIX runners");
    return;
  }
  t.after(() => fs.rm(install.root, { recursive: true, force: true }));

  const explicitBin = runWrapper(
    install,
    cleanEnv({
      PATH: `${install.externalDir}${path.delimiter}${process.env.PATH || ""}`,
      PRODEX_CODEX_BIN: install.externalCodex,
      PRODEX_CODEX_RESOLUTION: undefined,
    }),
  );
  assert.equal(explicitBin.codexBin, install.externalCodex);
  assert.equal(explicitBin.pathEntries[0], install.externalDir);

  const explicitMode = runWrapper(
    install,
    cleanEnv({
      PATH: `${install.externalDir}${path.delimiter}${process.env.PATH || ""}`,
      PRODEX_CODEX_BIN: undefined,
      PRODEX_CODEX_RESOLUTION: "external",
    }),
  );
  assert.equal(explicitMode.codexBin, install.externalCodex);
  assert.equal(explicitMode.pathEntries[0], install.externalDir);
});

test("prodex Codex shim prefers direct native Codex package", async (t) => {
  const install = await stageCodexShimInstall();
  if (!install) {
    t.skip("Codex shim native package test is only implemented for POSIX runners");
    return;
  }
  t.after(() => fs.rm(install.root, { recursive: true, force: true }));

  const result = spawnSync(process.execPath, [install.shimPath, "run", "--probe"], {
    encoding: "utf8",
    env: cleanEnv({}),
  });
  assert.equal(result.status, 0, result.stderr || result.stdout);
  const output = JSON.parse(result.stdout);
  assert.deepEqual(output.argv, ["run", "--probe"]);
  assert.equal(output.managedByNpm, "1");
  assert.equal(output.managedPackageRoot, await fs.realpath(install.openAiCodexDir));
});

test("prodex Codex shim fails fast when bundled native package is missing and no external codex exists", async (t) => {
  const install = await stageCodexShimInstall();
  if (!install) {
    t.skip("Codex shim native package test is only implemented for POSIX runners");
    return;
  }
  t.after(() => fs.rm(install.root, { recursive: true, force: true }));
  await fs.rm(install.platformPackageDir, { recursive: true, force: true });

  const result = spawnSync(process.execPath, [install.shimPath, "--version"], {
    encoding: "utf8",
    env: cleanEnv({ PATH: "" }),
  });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /Unable to locate bundled Codex native package/);
  assert.ok(result.stderr.includes("does not fall back to @openai/codex/bin/codex.js"));
  assert.equal(result.stdout, "");
});

test("prodex Codex shim repairs non-executable native Codex binary", async (t) => {
  const install = await stageCodexShimInstall();
  if (!install) {
    t.skip("Codex shim native package test is only implemented for POSIX runners");
    return;
  }
  t.after(() => fs.rm(install.root, { recursive: true, force: true }));
  await fs.chmod(install.nativeBinaryPath, 0o644);

  const result = spawnSync(process.execPath, [install.shimPath, "--version"], {
    encoding: "utf8",
    env: cleanEnv({}),
  });
  assert.equal(result.status, 0, result.stderr || result.stdout);
  const mode = (await fs.stat(install.nativeBinaryPath)).mode;
  assert.ok(mode & 0o111);
});

test("codex shim falls back to external codex when bundled native binary is unusable", async (t) => {
  if (process.platform === "win32") {
    t.skip("POSIX shell fixture only");
    return;
  }

  const platformSpecs = {
    linux: {
      x64: ["@openai/codex-linux-x64", "x86_64-unknown-linux-musl", "codex"],
      arm64: ["@openai/codex-linux-arm64", "aarch64-unknown-linux-musl", "codex"],
    },
    darwin: {
      x64: ["@openai/codex-darwin-x64", "x86_64-apple-darwin", "codex"],
      arm64: ["@openai/codex-darwin-arm64", "aarch64-apple-darwin", "codex"],
    },
  };
  const spec = platformSpecs[process.platform]?.[process.arch];
  if (!spec) {
    t.skip(`unsupported fixture platform ${process.platform}/${process.arch}`);
    return;
  }

  const [packageName, targetTriple, binaryFileName] = spec;
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-codex-shim-fallback-"));
  t.after(() => fs.rm(root, { recursive: true, force: true }));

  const shimPath = path.join(root, "codex-shim.cjs");
  await fs.copyFile(path.join("npm", "prodex", "lib", "codex-shim.cjs"), shimPath);
  await fs.chmod(shimPath, 0o755);

  const packageRoot = path.join(root, "node_modules", ...packageName.split("/"));
  await fs.mkdir(packageRoot, { recursive: true });
  await fs.writeFile(
    path.join(packageRoot, "package.json"),
    JSON.stringify({ name: packageName, version: "0.0.0-test" }),
  );

  // Simulate a corrupt optional native package: the expected native binary path
  // exists but is not a regular executable file.
  await fs.mkdir(path.join(packageRoot, "vendor", targetTriple, "bin", binaryFileName), {
    recursive: true,
  });

  const shimDir = path.join(root, "shim-bin");
  await fs.mkdir(shimDir);
  await fs.writeFile(
    path.join(shimDir, "codex"),
    `#!/bin/sh\nexec node ${JSON.stringify(shimPath)} "$@"\n`,
    { mode: 0o755 },
  );

  const brokenExternalDir = path.join(root, "broken-external-bin");
  await fs.mkdir(brokenExternalDir);
  await fs.writeFile(path.join(brokenExternalDir, "codex"), "not executable\n", {
    mode: 0o644,
  });

  const externalDir = path.join(root, "external-bin");
  await fs.mkdir(externalDir);
  await fs.writeFile(
    path.join(externalDir, "codex"),
    '#!/bin/sh\necho "external codex $@"\n',
    { mode: 0o755 },
  );

  const result = spawnSync(process.execPath, [shimPath, "--version"], {
    encoding: "utf8",
    env: {
      ...process.env,
      PATH: [shimDir, brokenExternalDir, externalDir].join(path.delimiter),
    },
  });

  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stderr, /falling back to external codex from PATH/);
  assert.match(result.stderr, /broken-external-bin.*not executable/);
  assert.match(result.stdout, /external codex --version/);
});

test("codex shim resolves nvm-style global codex symlink after skipping Prodex shim", async (t) => {
  if (process.platform === "win32") {
    t.skip("nvm POSIX layout fixture only");
    return;
  }

  const platformSpecs = {
    linux: {
      x64: ["@openai/codex-linux-x64", "x86_64-unknown-linux-musl", "codex"],
      arm64: ["@openai/codex-linux-arm64", "aarch64-unknown-linux-musl", "codex"],
    },
    darwin: {
      x64: ["@openai/codex-darwin-x64", "x86_64-apple-darwin", "codex"],
      arm64: ["@openai/codex-darwin-arm64", "aarch64-apple-darwin", "codex"],
    },
  };
  const spec = platformSpecs[process.platform]?.[process.arch];
  if (!spec) {
    t.skip(`unsupported fixture platform ${process.platform}/${process.arch}`);
    return;
  }

  const [packageName, targetTriple, binaryFileName] = spec;
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-codex-shim-nvm-"));
  t.after(() => fs.rm(root, { recursive: true, force: true }));

  const shimPath = path.join(root, "codex-shim.cjs");
  await fs.copyFile(path.join("npm", "prodex", "lib", "codex-shim.cjs"), shimPath);
  await fs.chmod(shimPath, 0o755);

  const packageRoot = path.join(root, "node_modules", ...packageName.split("/"));
  await fs.mkdir(packageRoot, { recursive: true });
  await fs.writeFile(
    path.join(packageRoot, "package.json"),
    JSON.stringify({ name: packageName, version: "0.0.0-test" }),
  );
  await fs.mkdir(path.join(packageRoot, "vendor", targetTriple, "bin", binaryFileName), {
    recursive: true,
  });

  const shimDir = path.join(root, "prodex-shim-bin");
  await fs.mkdir(shimDir);
  await fs.writeFile(
    path.join(shimDir, "codex"),
    `#!/bin/sh\nexec node ${JSON.stringify(shimPath)} "$@"\n`,
    { mode: 0o755 },
  );

  const nvmVersionRoot = path.join(root, ".nvm", "versions", "node", "v20.20.0");
  const nvmBin = path.join(nvmVersionRoot, "bin");
  const nvmCodexPackageBin = path.join(
    nvmVersionRoot,
    "lib",
    "node_modules",
    "@openai",
    "codex",
    "bin",
  );
  await fs.mkdir(nvmBin, { recursive: true });
  await fs.mkdir(nvmCodexPackageBin, { recursive: true });
  const nvmCodexJs = path.join(nvmCodexPackageBin, "codex.js");
  await fs.writeFile(
    nvmCodexJs,
    '#!/usr/bin/env node\nconsole.log("nvm codex " + process.argv.slice(2).join(" "));\n',
    { mode: 0o755 },
  );
  await fs.symlink(process.execPath, path.join(nvmBin, "node"));
  await fs.symlink("../lib/node_modules/@openai/codex/bin/codex.js", path.join(nvmBin, "codex"));

  const result = spawnSync(process.execPath, [shimPath, "--version"], {
    encoding: "utf8",
    env: {
      ...process.env,
      PATH: [shimDir, nvmBin].join(path.delimiter),
    },
  });

  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stderr, /falling back to external codex from PATH/);
  assert.match(result.stderr, /Prodex Codex shim/);
  assert.match(result.stderr, /\.nvm\/versions\/node\/v20\.20\.0\/bin\/codex/);
  assert.match(result.stdout, /nvm codex --version/);
});

test("codex shim can opt into npm installing external codex when none is found", async (t) => {
  if (process.platform === "win32") {
    t.skip("npm auto-install fixture is only implemented for POSIX runners");
    return;
  }

  const root = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-codex-shim-autoinstall-"));
  t.after(() => fs.rm(root, { recursive: true, force: true }));

  const shimPath = path.join(root, "codex-shim.cjs");
  await fs.copyFile(path.join("npm", "prodex", "lib", "codex-shim.cjs"), shimPath);
  await fs.chmod(shimPath, 0o755);

  const binDir = path.join(root, "bin");
  await ensureDir(binDir);
  const fakeNpm = path.join(binDir, "npm");
  await writeExecutable(
    fakeNpm,
    [
      `#!${process.execPath}`,
      "const fs = require('node:fs');",
      "const path = require('node:path');",
      "const expected = ['install', '-g', '@openai/codex@latest'];",
      "if (JSON.stringify(process.argv.slice(2)) !== JSON.stringify(expected)) {",
      "  console.error('unexpected npm args: ' + process.argv.slice(2).join(' '));",
      "  process.exit(17);",
      "}",
      "const codex = path.join(process.env.PRODEX_TEST_NPM_INSTALL_BIN, 'codex');",
      "fs.writeFileSync(codex, '#!/bin/sh\\necho npm-installed codex \"$@\"\\n');",
      "fs.chmodSync(codex, 0o755);",
      "",
    ].join("\n"),
  );

  const result = spawnSync(process.execPath, [shimPath, "--version"], {
    encoding: "utf8",
    env: cleanEnv({
      PATH: binDir,
      PRODEX_CODEX_AUTO_INSTALL: "1",
      PRODEX_TEST_NPM_INSTALL_BIN: binDir,
    }),
  });

  assert.equal(result.status, 0, result.stderr || result.stdout);
  assert.match(result.stderr, /attempting npm install because PRODEX_CODEX_AUTO_INSTALL=1/);
  assert.match(result.stderr, /falling back to external codex from PATH/);
  assert.match(result.stdout, /npm-installed codex --version/);
});
