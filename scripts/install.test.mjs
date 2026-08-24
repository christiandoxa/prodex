import assert from "node:assert/strict";
import crypto from "node:crypto";
import fs from "node:fs/promises";
import http from "node:http";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { openaiCodexDependencySpecifier } from "./npm/common.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const installerPath = path.join(repoRoot, "install.sh");
const windowsInstallerPath = path.join(repoRoot, "install.ps1");
const version = "1.2.3";

function targetForHost() {
  const arch = process.arch === "arm64" ? "aarch64" : "x86_64";
  if (process.platform === "darwin") return `${arch}-apple-darwin`;
  if (process.platform === "linux") return `${arch}-unknown-linux-gnu`;
  return null;
}

function run(command, args, options = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { ...options, stdio: ["ignore", "pipe", "pipe"] });
    let stdout = "";
    let stderr = "";
    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk) => (stdout += chunk));
    child.stderr.on("data", (chunk) => (stderr += chunk));
    child.on("error", reject);
    child.on("close", (code, signal) => resolve({ code, signal, stdout, stderr }));
  });
}

async function fixture(t, { validChecksum = true, manifestImplementation = null, compilerRequired = false } = {}) {
  const target = targetForHost();
  if (!target) {
    t.skip(`installer fixture unsupported on ${process.platform}`);
    return null;
  }
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-installer-test-"));
  const home = path.join(root, "home");
  const binDir = path.join(home, ".local", "bin");
  const fakeBin = path.join(root, "fake-bin");
  const managerLog = path.join(root, "manager.log");
  await fs.mkdir(fakeBin, { recursive: true });
  const asset = `prodex-${target}`;
  const binary = Buffer.from(
    manifestImplementation === null
      ? `#!/bin/sh\nprintf 'prodex ${version}\\n'\n`
      : `#!/bin/sh\nif [ "$1" = "doctor" ]; then\nprintf '%s\\n' '{'\nprintf '%s\\n' '  "implementation": "${manifestImplementation}",'\nprintf '%s\\n' '  "fallback": false,'\nprintf '%s\\n' '  "compiler_required": ${compilerRequired},'\nprintf '%s\\n' '  "self_test": "passed"'\nprintf '%s\\n' '}'\nelse\nprintf 'prodex ${version}\\n'\nfi\n`,
  );
  const manifest =
    manifestImplementation === null
      ? null
      : [
          "schema_version\t1",
          `version\t${version}`,
          `commit\t${"0".repeat(40)}`,
          "target\tasset\timplementation\tmojo_version\tmojo_features\truntime_bundle\tminimum_glibc",
          `${target}\t${asset}\t${manifestImplementation}\t${manifestImplementation === "mojo-compiled-in" ? "1.0.0" : ""}\t${manifestImplementation === "mojo-compiled-in" ? "mojo-core" : ""}\tfalse\t`,
          "",
        ].join("\n");
  const digest = validChecksum
    ? crypto.createHash("sha256").update(binary).digest("hex")
    : "0".repeat(64);
  const manifestDigest = manifest === null ? null : crypto.createHash("sha256").update(manifest).digest("hex");
  const server = http.createServer((request, response) => {
    if (request.url === "/release/SHA256SUMS") {
      response.end(`${digest}  ${asset}\n${manifestDigest ?? ""}${manifestDigest === null ? "" : `  release-manifest.tsv\n`}`);
    } else if (request.url === "/release/release-manifest.tsv" && manifest !== null) {
      response.end(manifest);
    } else if (request.url === `/release/${asset}`) {
      response.end(binary);
    } else {
      response.writeHead(404).end();
    }
  });
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  t.after(async () => {
    await new Promise((resolve) => server.close(resolve));
    await fs.rm(root, { recursive: true, force: true });
  });
  const { port } = server.address();
  const env = {
    ...process.env,
    HOME: home,
    PATH: `${fakeBin}:/usr/bin:/bin`,
    PRODEX_INSTALL_DIR: binDir,
    PRODEX_NON_INTERACTIVE: "1",
    PRODEX_NO_PATH_UPDATE: "1",
    PRODEX_RELEASE_BASE_URL: `http://127.0.0.1:${port}/release`,
    TEST_MANAGER_LOG: managerLog,
    npm_package_name: "",
  };
  return { root, binDir, fakeBin, managerLog, env };
}

async function runInstaller(fixtureState, extraEnv = {}) {
  return run("sh", [installerPath, "--release", version], {
    cwd: repoRoot,
    env: { ...fixtureState.env, ...extraEnv },
  });
}

test("install.sh has valid POSIX shell syntax", { skip: process.platform === "win32" }, async () => {
  const result = await run("sh", ["-n", installerPath], { cwd: repoRoot });
  assert.equal(result.code, 0, result.stderr);
});

test("Unix and Windows Codex migration pins stay synchronized", async () => {
  const [unixInstaller, windowsInstaller] = await Promise.all([
    fs.readFile(installerPath, "utf8"),
    fs.readFile(windowsInstallerPath, "utf8"),
  ]);
  const unixVersion = unixInstaller.match(/^CODEX_NPM_VERSION="([^"]+)"$/m)?.[1];
  const windowsVersion = windowsInstaller.match(/^\$CodexNpmVersion = "([^"]+)"$/m)?.[1];

  assert.equal(unixVersion, windowsVersion);
  assert.equal(unixVersion, openaiCodexDependencySpecifier);
});

test("install.ps1 verifies Windows release assets", async () => {
  const source = await fs.readFile(windowsInstallerPath, "utf8");
  assert.match(source, /prodex-\$Target\.exe/);
  assert.match(source, /Get-FileHash[^\n]+SHA256/);
  assert.match(source, /System\.Security\.Cryptography\.SHA256/);
  assert.match(source, /x86_64-pc-windows-msvc/);
  assert.match(source, /aarch64-pc-windows-msvc/);
  assert.match(source, /New-Item -ItemType Junction/);
  assert.match(source, new RegExp(`\\$CodexNpmVersion = "${openaiCodexDependencySpecifier}"`));
  assert.match(source, /"@openai\/codex@\$CodexNpmVersion"/);
  assert.doesNotMatch(source, /@openai\/codex@latest\b/);
});

test("install.ps1 prefers the OS architecture under emulation", async () => {
  const source = await fs.readFile(windowsInstallerPath, "utf8");
  const osArchitectureProbe = source.indexOf('GetProperty("OSArchitecture")');
  const processArchitectureFallback = source.indexOf("$arch = $env:PROCESSOR_ARCHITEW6432");
  assert.ok(osArchitectureProbe >= 0, "installer should probe OSArchitecture");
  assert.ok(
    processArchitectureFallback > osArchitectureProbe,
    "process architecture environment variables must remain a fallback",
  );
});

test("install.ps1 installs the native Windows binary", { skip: process.platform !== "win32" }, async (t) => {
  const cargoToml = await fs.readFile(path.join(repoRoot, "Cargo.toml"), "utf8");
  const currentVersion = cargoToml.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  assert.ok(currentVersion, "Cargo.toml package version should exist");

  const target = process.arch === "arm64" ? "aarch64-pc-windows-msvc" : "x86_64-pc-windows-msvc";
  const sourceBinary = path.join(repoRoot, "target", "debug", "prodex.exe");
  await fs.access(sourceBinary);

  const root = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-windows-installer-test-"));
  const releaseDir = path.join(root, "release");
  const binDir = path.join(root, "bin");
  const asset = `prodex-${target}.exe`;
  await fs.mkdir(releaseDir, { recursive: true });
  const binary = await fs.readFile(sourceBinary);
  await fs.writeFile(path.join(releaseDir, asset), binary);
  await fs.writeFile(
    path.join(releaseDir, "SHA256SUMS"),
    `${crypto.createHash("sha256").update(binary).digest("hex")}  ${asset}\n`,
  );
  t.after(() => fs.rm(root, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 }));

  const result = await run(
    "powershell.exe",
    ["-NoLogo", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", windowsInstallerPath, "-Release", currentVersion],
    {
      cwd: repoRoot,
      env: {
        ...process.env,
        PRODEX_INSTALL_DIR: binDir,
        PRODEX_NON_INTERACTIVE: "1",
        PRODEX_NO_PATH_UPDATE: "1",
        PRODEX_RELEASE_BASE_URL: releaseDir,
        PRODEX_RUNNING_EXE: "",
        npm_package_name: "",
      },
    },
  );
  assert.equal(result.code, 0, result.stderr);
  const installed = path.join(binDir, "prodex.exe");
  assert.equal((await run(installed, ["--version"])).stdout.trim(), `prodex ${currentVersion}`);
});

test("installer verifies and installs the host release binary", async (t) => {
  const state = await fixture(t);
  if (!state) return;
  const result = await runInstaller(state);
  assert.equal(result.code, 0, result.stderr);
  const installed = path.join(state.binDir, "prodex");
  assert.equal((await run(installed, ["--version"])).stdout, `prodex ${version}\n`);
});

test("installer selects and verifies a compiled-in Mojo release from the manifest", async (t) => {
  const state = await fixture(t, { manifestImplementation: "mojo-compiled-in" });
  if (!state) return;
  const result = await runInstaller(state);
  assert.equal(result.code, 0, result.stderr);
  assert.match(result.stdout, /Release implementation: mojo-compiled-in/);
  const installed = path.join(state.binDir, "prodex");
  assert.equal((await run(installed, ["--version"])).stdout, `prodex ${version}\n`);
});

test("installer rejects a Mojo release that requires a compiler at runtime", async (t) => {
  const state = await fixture(t, { manifestImplementation: "mojo-compiled-in", compilerRequired: true });
  if (!state) return;
  const result = await runInstaller(state);
  assert.notEqual(result.code, 0);
  assert.match(result.stderr, /unexpectedly requires the Mojo compiler/);
  await assert.rejects(fs.access(path.join(state.binDir, "prodex")));
});

test("installer require-Mojo mode rejects a Rust compatibility release", async (t) => {
  const state = await fixture(t, { manifestImplementation: "rust" });
  if (!state) return;
  const result = await runInstaller(state, { PRODEX_INSTALL_REQUIRE_MOJO: "1" });
  assert.notEqual(result.code, 0);
  assert.match(result.stderr, /no Mojo-enabled artifact/);
  await assert.rejects(fs.access(path.join(state.binDir, "prodex")));
});

test("installer rejects a release binary with the wrong checksum", async (t) => {
  const state = await fixture(t, { validChecksum: false });
  if (!state) return;
  const result = await runInstaller(state);
  assert.notEqual(result.code, 0);
  assert.match(result.stderr, /checksum did not match/);
  await assert.rejects(fs.access(path.join(state.binDir, "prodex")));
});

test("updater migrates npm Prodex and preserves Codex", async (t) => {
  const state = await fixture(t);
  if (!state) return;
  const npm = path.join(state.fakeBin, "npm");
  await fs.writeFile(npm, "#!/bin/sh\nprintf '%s\\n' \"$*\" >>\"$TEST_MANAGER_LOG\"\n", {
    mode: 0o755,
  });
  const result = await runInstaller(state, {
    PRODEX_MIGRATE: "1",
    PRODEX_RUNNING_EXE:
      "/home/test-user/lib/node_modules/@christiandoxa/prodex-linux-x64/vendor/prodex",
    npm_package_name: "@christiandoxa/prodex",
  });
  assert.equal(result.code, 0, result.stderr);
  assert.deepEqual((await fs.readFile(state.managerLog, "utf8")).trim().split("\n"), [
    `install -g @openai/codex@${openaiCodexDependencySpecifier}`,
    "uninstall -g @christiandoxa/prodex",
  ]);
});

test("updater migrates cargo-installed Prodex", async (t) => {
  const state = await fixture(t);
  if (!state) return;
  const cargo = path.join(state.fakeBin, "cargo");
  await fs.writeFile(
    cargo,
    [
      "#!/bin/sh",
      'if [ "$1 $2" = "install --list" ]; then',
      "  echo 'prodex v0.9.0:'",
      "  exit 0",
      "fi",
      "printf '%s\\n' \"$*\" >>\"$TEST_MANAGER_LOG\"",
      "",
    ].join("\n"),
    { mode: 0o755 },
  );
  const result = await runInstaller(state, {
    PRODEX_MIGRATE: "1",
    PRODEX_RUNNING_EXE: "/home/test-user/.cargo/bin/prodex",
  });
  assert.equal(result.code, 0, result.stderr);
  assert.equal((await fs.readFile(state.managerLog, "utf8")).trim(), "uninstall prodex");
});
