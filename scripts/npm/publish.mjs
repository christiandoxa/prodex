#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { spawn } from "node:child_process";

function parseArgs(argv) {
  const args = { root: null, dryRun: false, provenance: false };
  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--root") {
      index += 1;
      if (!argv[index]) {
        throw new Error("--root requires a value");
      }
      args.root = path.resolve(argv[index]);
      continue;
    }
    if (value === "--dry-run") {
      args.dryRun = true;
      continue;
    }
    if (value === "--provenance") {
      args.provenance = true;
      continue;
    }
    if (value === "--help" || value === "-h") {
      args.help = true;
      continue;
    }
    throw new Error(`unknown argument: ${value}`);
  }

  if (!args.help && !args.root) {
    throw new Error("--root is required");
  }

  return args;
}

function runCommand(command, args, cwd, extraEnv = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd,
      env: {
        ...process.env,
        ...extraEnv,
      },
      stdio: "inherit",
    });

    child.on("error", reject);
    child.on("close", (code, signal) => {
      if (signal) {
        reject(new Error(`${command} exited with signal ${signal}`));
        return;
      }
      if (code !== 0) {
        reject(new Error(`${command} exited with code ${code}`));
        return;
      }
      resolve();
    });
  });
}

function runCommandCapture(command, args, cwd, extraEnv = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd,
      env: {
        ...process.env,
        ...extraEnv,
      },
      stdio: ["ignore", "pipe", "pipe"],
    });

    let stdout = "";
    let stderr = "";
    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    child.on("error", reject);
    child.on("close", (code, signal) => {
      if (signal) {
        reject(new Error(`${command} exited with signal ${signal}`));
        return;
      }
      resolve({ code, stdout, stderr });
    });
  });
}

function packageSpec(name, version) {
  return `${name}@${version}`;
}

function npmViewPackageNotFound(result) {
  const output = `${result.stdout}\n${result.stderr}`;
  return /\bE404\b/.test(output) || /\b404 Not Found\b/i.test(output);
}

async function npmPackageVersionExists(name, version, cwd) {
  const result = await runCommandCapture(
    "npm",
    ["view", packageSpec(name, version), "version"],
    cwd,
  );
  if (result.code === 0) {
    return result.stdout.trim() === version;
  }
  if (npmViewPackageNotFound(result)) {
    return false;
  }
  throw new Error(
    [
      `npm view ${packageSpec(name, version)} exited with code ${result.code}`,
      result.stderr.trim(),
      result.stdout.trim(),
    ]
      .filter(Boolean)
      .join("\n"),
  );
}

async function listPackageDirectories(root) {
  const packagesDir = path.join(root, "packages");
  const entries = await fs.readdir(packagesDir, { withFileTypes: true });
  const directories = entries
    .filter((entry) => entry.isDirectory())
    .map((entry) => path.join(packagesDir, entry.name));

  const metadata = await Promise.all(
    directories.map(async (dir) => {
      const packageJson = JSON.parse(await fs.readFile(path.join(dir, "package.json"), "utf8"));
      return { dir, name: packageJson.name };
    }),
  );

  const mainPackage = metadata.filter((entry) => entry.name === "@christiandoxa/prodex");
  const platformPackages = metadata.filter((entry) => entry.name !== "@christiandoxa/prodex");
  platformPackages.sort((left, right) => left.name.localeCompare(right.name));
  return [...platformPackages, ...mainPackage].map((entry) => entry.dir);
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    process.stdout.write(
      [
        "Usage: node scripts/npm/publish.mjs --root <staging-dir> [--dry-run] [--provenance]",
        "",
        "Publishes staged prodex npm packages in dependency-safe order.",
      ].join("\n") + "\n",
    );
    return;
  }

  const packageDirs = await listPackageDirectories(args.root);
  for (const dir of packageDirs) {
    const packageJson = JSON.parse(await fs.readFile(path.join(dir, "package.json"), "utf8"));
    const publishArgs = ["publish", "--access", "public"];
    if (args.provenance) {
      publishArgs.push("--provenance");
    }
    if (args.dryRun) {
      publishArgs.push("--dry-run");
    }

    process.stdout.write(`publishing ${packageJson.name}@${packageJson.version}\n`);
    if (
      !args.dryRun &&
      (await npmPackageVersionExists(packageJson.name, packageJson.version, dir))
    ) {
      process.stdout.write(
        `skipping ${packageJson.name}@${packageJson.version}; already published\n`,
      );
      continue;
    }
    await runCommand("npm", publishArgs, dir, {
      npm_config_foreground_scripts: "true",
    });
  }
}

await main();
