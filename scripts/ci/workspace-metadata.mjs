#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { spawn } from "node:child_process";
import { fileURLToPath } from "node:url";
import { normalizeGitPath } from "./guard-common.mjs";
import { repoRoot } from "../npm/common.mjs";

let workspacePackageCache = null;

function stripTomlComment(line) {
  let inString = false;
  let escaped = false;
  for (let index = 0; index < line.length; index += 1) {
    const character = line[index];
    if (character === '"' && !escaped) {
      inString = !inString;
    }
    if (!inString && character === "#") {
      return line.slice(0, index);
    }
    escaped = character === "\\" && !escaped;
    if (character !== "\\") {
      escaped = false;
    }
  }
  return line;
}

function sortedUnique(values) {
  return [...new Set(values.filter(Boolean).map(normalizeGitPath))].sort();
}

function relativeWorkspacePath(filePath, workspaceRoot) {
  const absolutePath = path.isAbsolute(filePath) ? filePath : path.join(workspaceRoot, filePath);
  return normalizeGitPath(path.relative(workspaceRoot, absolutePath)) || ".";
}

function pathSpec({ exact = [], prefixes = [] }) {
  const spec = {};
  const sortedExact = sortedUnique(exact);
  const sortedPrefixes = sortedUnique(prefixes);
  if (sortedExact.length > 0) {
    spec.exact = sortedExact;
  }
  if (sortedPrefixes.length > 0) {
    spec.prefixes = sortedPrefixes;
  }
  return spec;
}

function classifyRustTargetPath(relativePath, kinds) {
  const normalized = normalizeGitPath(relativePath);
  const kindSet = new Set(kinds ?? []);
  if (kindSet.has("test") || kindSet.has("bench")) {
    return "test";
  }
  if (normalized === "build.rs" || normalized.endsWith("/build.rs")) {
    return "source-exact";
  }
  return "source";
}

function rustPrefixForPath(relativePath) {
  const normalized = normalizeGitPath(relativePath);
  if (normalized.endsWith("/src/lib.rs") || normalized.endsWith("/src/main.rs")) {
    return normalized.slice(0, normalized.lastIndexOf("/") + 1);
  }
  if (normalized.startsWith("src/")) {
    return "src/";
  }
  if (normalized.startsWith("tests/")) {
    return "tests/";
  }
  if (normalized.startsWith("benches/")) {
    return "benches/";
  }
  const srcIndex = normalized.indexOf("/src/");
  if (srcIndex >= 0) {
    return normalized.slice(0, srcIndex + "/src/".length);
  }
  const testsIndex = normalized.indexOf("/tests/");
  if (testsIndex >= 0) {
    return normalized.slice(0, testsIndex + "/tests/".length);
  }
  const benchesIndex = normalized.indexOf("/benches/");
  if (benchesIndex >= 0) {
    return normalized.slice(0, benchesIndex + "/benches/".length);
  }
  return null;
}

function pathGroupsForPackages(packages) {
  const cargoManifests = packages.map((pkg) => pkg.manifestPath);
  const rustSourceExact = [];
  const rustSourcePrefixes = [];
  const rustTestExact = [];
  const rustTestPrefixes = [];

  for (const pkg of packages) {
    for (const target of pkg.targets ?? []) {
      if (!target.path) {
        continue;
      }
      const classification = classifyRustTargetPath(target.path, target.kind);
      if (classification === "source-exact") {
        rustSourceExact.push(target.path);
        continue;
      }
      const prefix = rustPrefixForPath(target.path);
      if (classification === "test") {
        if (prefix) {
          rustTestPrefixes.push(prefix);
        } else {
          rustTestExact.push(target.path);
        }
        continue;
      }
      if (prefix) {
        rustSourcePrefixes.push(prefix);
      } else {
        rustSourceExact.push(target.path);
      }
    }
  }

  return {
    cargoManifests: pathSpec({ exact: cargoManifests }),
    rustSources: pathSpec({ exact: rustSourceExact, prefixes: rustSourcePrefixes }),
    rustTests: pathSpec({ exact: rustTestExact, prefixes: rustTestPrefixes }),
    rustAll: pathSpec({
      exact: ["Cargo.lock", ...cargoManifests, ...rustSourceExact, ...rustTestExact],
      prefixes: [...rustSourcePrefixes, ...rustTestPrefixes],
    }),
  };
}

export function parseCargoPackageName(contents) {
  const lines = contents.split(/\r?\n/);
  let inPackage = false;
  for (const rawLine of lines) {
    const line = stripTomlComment(rawLine).trim();
    const section = line.match(/^\[([^\]]+)\]$/)?.[1]?.trim();
    if (section) {
      inPackage = section === "package";
      continue;
    }
    if (!inPackage) {
      continue;
    }
    const match = line.match(/^name\s*=\s*"([^"]+)"/);
    if (match) {
      return match[1];
    }
  }
  return null;
}

export function parseWorkspaceMembers(contents) {
  const lines = contents.split(/\r?\n/);
  let inWorkspace = false;
  let collectingMembers = false;
  let buffer = "";
  for (const rawLine of lines) {
    const line = stripTomlComment(rawLine).trim();
    const section = line.match(/^\[([^\]]+)\]$/)?.[1]?.trim();
    if (section) {
      inWorkspace = section === "workspace";
      collectingMembers = false;
      buffer = "";
      continue;
    }
    if (!inWorkspace) {
      continue;
    }
    if (!collectingMembers && /^members\s*=/.test(line)) {
      collectingMembers = true;
      buffer = line.slice(line.indexOf("=") + 1);
    } else if (collectingMembers) {
      buffer += `\n${line}`;
    }
    if (collectingMembers && buffer.includes("]")) {
      return [...buffer.matchAll(/"([^"]+)"/g)].map((match) => normalizeGitPath(match[1]));
    }
  }
  return [];
}

function packageFromCargoMetadata(pkg, workspaceRoot) {
  const manifestPath = relativeWorkspacePath(pkg.manifest_path, workspaceRoot);
  const crateDir = relativeWorkspacePath(path.dirname(pkg.manifest_path), workspaceRoot);
  return {
    crateDir,
    manifestPath,
    name: pkg.name,
    targets: (pkg.targets ?? [])
      .filter((target) => target.src_path)
      .map((target) => {
        const relativePath = relativeWorkspacePath(target.src_path, workspaceRoot);
        return {
          kind: target.kind ?? [],
          name: target.name ?? null,
          path: relativePath,
          srcPath: relativePath,
        };
      })
      .sort((left, right) => left.path.localeCompare(right.path)),
  };
}

export function workspacePackagesFromCargoMetadata(metadata) {
  if (!metadata || !Array.isArray(metadata.packages)) {
    throw new Error("cargo metadata JSON must include a packages array");
  }
  const workspaceRoot = metadata.workspace_root ?? repoRoot;
  const workspaceMembers = new Set(metadata.workspace_members ?? metadata.packages.map((pkg) => pkg.id));
  return metadata.packages
    .filter((pkg) => workspaceMembers.has(pkg.id))
    .map((pkg) => packageFromCargoMetadata(pkg, workspaceRoot))
    .sort((left, right) => left.crateDir.localeCompare(right.crateDir));
}

export function workspaceMetadataFromCargoMetadata(metadata) {
  const packages = workspacePackagesFromCargoMetadata(metadata);
  return {
    source: "cargo-metadata",
    packages,
    packageNames: packages.map((pkg) => pkg.name),
    crateDirs: packages.map((pkg) => pkg.crateDir),
    pathGroups: pathGroupsForPackages(packages),
  };
}

async function readOptionalFile(filePath) {
  try {
    return await fs.readFile(filePath, "utf8");
  } catch (error) {
    if (error?.code === "ENOENT") {
      return null;
    }
    throw error;
  }
}

async function existingCargoMemberDirs(root, members) {
  const dirs = [];
  for (const member of members.length > 0 ? members : ["."]) {
    if (member.endsWith("/*")) {
      const base = member.slice(0, -"/*".length);
      const entries = await fs.readdir(path.join(root, base), { withFileTypes: true }).catch(() => []);
      for (const entry of entries) {
        if (!entry.isDirectory()) {
          continue;
        }
        const crateDir = normalizeGitPath(path.join(base, entry.name));
        if (await readOptionalFile(path.join(root, crateDir, "Cargo.toml"))) {
          dirs.push(crateDir);
        }
      }
      continue;
    }
    if (await readOptionalFile(path.join(root, member, "Cargo.toml"))) {
      dirs.push(normalizeGitPath(member) || ".");
    }
  }
  return sortedUnique(dirs);
}

async function fallbackTargetsForCrate(root, crateDir) {
  const candidates = [
    { path: path.join(crateDir, "build.rs"), kind: ["custom-build"] },
    { path: path.join(crateDir, "src/lib.rs"), kind: ["lib"] },
    { path: path.join(crateDir, "src/main.rs"), kind: ["bin"] },
  ];
  const targets = [];
  for (const candidate of candidates) {
    const relativePath = normalizeGitPath(candidate.path);
    if ((await readOptionalFile(path.join(root, relativePath))) !== null) {
      targets.push({ kind: candidate.kind, name: null, path: relativePath, srcPath: relativePath });
    }
  }
  for (const dirName of ["tests", "benches"]) {
    const absoluteDir = path.join(root, crateDir, dirName);
    const entries = await fs.readdir(absoluteDir, { withFileTypes: true }).catch(() => []);
    for (const entry of entries) {
      if (entry.isFile() && entry.name.endsWith(".rs")) {
        const relativePath = normalizeGitPath(path.join(crateDir, dirName, entry.name));
        targets.push({
          kind: [dirName === "tests" ? "test" : "bench"],
          name: null,
          path: relativePath,
          srcPath: relativePath,
        });
      }
    }
  }
  return targets.sort((left, right) => left.path.localeCompare(right.path));
}

export async function workspaceMetadataFromCargoToml(root = repoRoot) {
  const rootManifest = await fs.readFile(path.join(root, "Cargo.toml"), "utf8");
  const memberDirs = await existingCargoMemberDirs(root, parseWorkspaceMembers(rootManifest));
  const packages = [];
  for (const crateDir of memberDirs) {
    const manifestPath = crateDir === "." ? "Cargo.toml" : `${crateDir}/Cargo.toml`;
    const manifest = await fs.readFile(path.join(root, manifestPath), "utf8");
    const name = parseCargoPackageName(manifest);
    if (!name) {
      throw new Error(`missing package.name in ${manifestPath}`);
    }
    packages.push({
      crateDir,
      manifestPath,
      name,
      targets: await fallbackTargetsForCrate(root, crateDir === "." ? "" : crateDir),
    });
  }
  return {
    source: "cargo-toml",
    packages,
    packageNames: packages.map((pkg) => pkg.name),
    crateDirs: packages.map((pkg) => pkg.crateDir),
    pathGroups: pathGroupsForPackages(packages),
  };
}

async function runCargoMetadata(root = repoRoot) {
  return await new Promise((resolve, reject) => {
    const child = spawn("cargo", ["metadata", "--format-version", "1", "--no-deps"], {
      cwd: root,
      stdio: ["ignore", "pipe", "pipe"],
    });
    const stdout = [];
    const stderr = [];
    child.stdout.on("data", (chunk) => stdout.push(chunk));
    child.stderr.on("data", (chunk) => stderr.push(chunk));
    child.on("error", reject);
    child.on("close", (code) => {
      if (code === 0) {
        resolve(JSON.parse(Buffer.concat(stdout).toString("utf8")));
        return;
      }
      reject(new Error(Buffer.concat(stderr).toString("utf8").trim() || `cargo metadata exited ${code}`));
    });
  });
}

export function runtimeTestMetadataFromManifest(manifest) {
  const packageNames = new Set();
  const filters = new Set();
  for (const testCase of manifest.RUNTIME_SMOKE_TESTS ?? []) {
    if (testCase.package) {
      packageNames.add(testCase.package);
    }
    if (testCase.filter) {
      filters.add(testCase.filter);
    }
  }
  for (const testCase of manifest.RUNTIME_CI_TEST_CASES ?? []) {
    if (testCase.package) {
      packageNames.add(testCase.package);
    }
    if (testCase.filter) {
      filters.add(testCase.filter);
    }
    if (testCase.name) {
      filters.add(testCase.name);
    }
  }
  const workflowShards = (manifest.RUNTIME_CI_WORKFLOW_SHARDS ?? []).map((shard) => {
    for (const filter of shard.filters ?? []) {
      if (filter.filter) {
        filters.add(filter.filter);
      }
    }
    return {
      suite: shard.suite,
      label: shard.label,
      filterCount: (shard.filters ?? []).length,
    };
  });
  return {
    packageNames: [...packageNames].sort(),
    tags: Object.values(manifest.RUNTIME_TEST_TAGS ?? {}).sort(),
    counts: {
      smokeTests: (manifest.RUNTIME_SMOKE_TESTS ?? []).length,
      ciTestCases: (manifest.RUNTIME_CI_TEST_CASES ?? []).length,
      workflowShards: workflowShards.length,
    },
    workflowShards,
    filters: [...filters].sort(),
  };
}

export async function readRuntimeTestMetadata() {
  return runtimeTestMetadataFromManifest(await import("./runtime-test-manifest.mjs"));
}

export async function workspaceMetadata({ source = "auto", root = repoRoot } = {}) {
  if (source === "cargo-metadata" || source === "auto") {
    try {
      return workspaceMetadataFromCargoMetadata(await runCargoMetadata(root));
    } catch (error) {
      if (source === "cargo-metadata") {
        throw error;
      }
    }
  }
  return await workspaceMetadataFromCargoToml(root);
}

export async function workspacePackages() {
  if (workspacePackageCache) {
    return workspacePackageCache;
  }
  workspacePackageCache = (await workspaceMetadata()).packages;
  return workspacePackageCache;
}

export async function workspacePackageNameForCrateDir(crateDir) {
  const normalizedCrateDir = normalizeGitPath(crateDir) || ".";
  const packageInfo = (await workspacePackages()).find((pkg) => pkg.crateDir === normalizedCrateDir);
  if (!packageInfo) {
    throw new Error(`unknown crate dir: ${normalizedCrateDir}`);
  }
  return packageInfo.name;
}

function parseArgs(argv) {
  const args = { json: false, includeRuntimeTests: false, repoRoot, source: "auto" };
  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--source") {
      index += 1;
      args.source = argv[index];
      continue;
    }
    if (value === "--repo-root") {
      index += 1;
      args.repoRoot = argv[index];
      continue;
    }
    if (value === "--json") {
      args.json = true;
      continue;
    }
    if (value === "--include-runtime-tests") {
      args.includeRuntimeTests = true;
      continue;
    }
    if (value === "--help" || value === "-h") {
      args.help = true;
      continue;
    }
    throw new Error(`unknown argument: ${value}`);
  }
  return args;
}

function printHelp() {
  process.stdout.write("Usage: node scripts/ci/workspace-metadata.mjs [--source auto|cargo-metadata|cargo-toml] [--repo-root <path>] [--json]\n\n" +
      "Derives workspace package names, crate directories, and Rust path groups from cargo metadata.\n" +
      "Falls back to Cargo.toml workspace members when requested or when cargo metadata is unavailable in auto mode.\n" +
      "Use --include-runtime-tests with --json to include metadata derived from runtime-test-manifest.mjs.\n");
}

function printPackages(packages) {
  for (const pkg of packages) {
    process.stdout.write(`${pkg.name}\t${pkg.crateDir}\n`);
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.stdout.on("error", (error) => {
    if (error?.code !== "EPIPE") {
      throw error;
    }
  });
  try {
    const args = parseArgs(process.argv);
    if (args.help) {
      printHelp();
    } else {
      const workspace = await workspaceMetadata({ source: args.source, root: args.repoRoot });
      if (args.json) {
        const result = { workspace };
        if (args.includeRuntimeTests) {
          result.runtimeTests = await readRuntimeTestMetadata();
        }
        process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
      } else {
        printPackages(workspace.packages);
      }
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    process.stderr.write(`workspace-metadata: ${message}\n`);
    process.exitCode = 1;
  }
}
