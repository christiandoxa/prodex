#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..", "..");

const LOW_LEVEL_CRATES = Object.freeze([
  "prodex-audit-log",
  "prodex-bench-support",
  "prodex-cli",
  "prodex-codex-config",
  "prodex-core",
  "prodex-profile-export",
  "prodex-profile-identity",
  "prodex-provider-core",
  "prodex-proxy-config",
  "prodex-redaction",
  "prodex-runtime-anthropic",
  "prodex-runtime-cookies",
  "prodex-runtime-policy",
  "prodex-runtime-state",
  "prodex-runtime-tuning",
  "prodex-secret-store",
  "prodex-shared-codex-fs",
  "prodex-state",
]);

const LOW_LEVEL_FORBIDDEN = Object.freeze([
  "prodex-app",
  "prodex-app-reports",
  "prodex-runtime-doctor",
  "prodex-runtime-launch",
  "prodex-runtime-proxy",
  "prodex-terminal-ui",
]);

const RULES = Object.freeze([
  {
    id: "low-level-crates-stay-below-orchestration",
    from: ({ name }) => LOW_LEVEL_CRATES.includes(name),
    to: new Set(LOW_LEVEL_FORBIDDEN),
    reason: "low-level helpers should stay reusable and side-effect-light.",
    fix: "Put rendering/orchestration in prodex-app or a report crate; keep pure data/helpers low-level.",
  },
  {
    id: "app-reports-do-not-depend-on-app",
    from: ({ name }) => name === "prodex-app-reports",
    to: new Set(["prodex-app", "prodex-runtime-launch"]),
    reason: "report helpers must be reusable by app-owned screens without importing app orchestration.",
    fix: "Pass plain data into report helpers instead of depending on command handlers.",
  },
  {
    id: "terminal-ui-stays-generic",
    from: ({ name }) => name === "prodex-terminal-ui",
    to: new Set([
      "prodex-app",
      "prodex-app-reports",
      "prodex-runtime-doctor",
      "prodex-runtime-launch",
      "prodex-runtime-proxy",
      "prodex-runtime-quota",
    ]),
    reason: "terminal-ui must remain generic layout/printing code.",
    fix: "Keep domain-specific rendering in report crates or prodex-app; pass strings/layout data into terminal-ui.",
  },
  {
    id: "runtime-proxy-stays-hot-path-helper",
    from: ({ name }) => name === "prodex-runtime-proxy",
    to: new Set([
      "prodex-app",
      "prodex-app-reports",
      "prodex-runtime-doctor",
      "prodex-runtime-launch",
      "prodex-runtime-quota",
      "prodex-terminal-ui",
    ]),
    reason: "runtime-proxy is hot-path boundary/helper logic, not app orchestration or terminal rendering.",
    fix: "Move pure classifiers/DTOs into prodex-runtime-proxy; keep launch, doctor, quota, and rendering outside it.",
  },
  {
    id: "focused-crates-do-not-depend-on-app",
    from: ({ name }) => name !== "prodex" && name !== "prodex-app",
    to: new Set(["prodex-app"]),
    reason: "prodex-app is orchestration glue; focused crates must not depend upward on it.",
    fix: "Move shared types/helpers into a focused crate, then call them from prodex-app.",
  },
]);

function parseArgs(argv) {
  const args = { selfTest: false };
  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--self-test") {
      args.selfTest = true;
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
  process.stdout.write(
    [
      "Usage: node scripts/ci/crate-boundary-guard.mjs [--self-test]",
      "",
      "Parses workspace Cargo manifests and fails on direct crate dependency edges that point upward",
      "from helper/render/runtime-boundary crates into app orchestration or hot-path-incompatible layers.",
    ].join("\n") + "\n",
  );
}

function stripComment(line) {
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

function parseSection(line) {
  const match = line.match(/^\s*\[+\s*([^\]]+?)\s*\]+\s*$/);
  return match ? match[1].trim() : null;
}

function dependencySection(section) {
  if (section === "workspace.dependencies") {
    return false;
  }
  return /(^|\.)((dev-)?dependencies|build-dependencies)$/.test(section);
}

function parseKeyValue(line) {
  const match = line.match(/^\s*([A-Za-z0-9_.-]+)\s*=\s*(.+?)\s*$/);
  if (!match) {
    return null;
  }
  return { key: match[1], value: match[2].trim() };
}

function parseString(value) {
  const match = value.match(/^"([^"]*)"/);
  return match ? match[1] : null;
}

function parseInlineTable(value) {
  const trimmed = value.trim();
  if (!trimmed.startsWith("{") || !trimmed.endsWith("}")) {
    return null;
  }

  const fields = new Map();
  const body = trimmed.slice(1, -1);
  const pattern = /([A-Za-z0-9_.-]+)\s*=\s*("([^"]*)"|true|false|[A-Za-z0-9_.-]+)/g;
  let match;
  while ((match = pattern.exec(body)) !== null) {
    const rawValue = match[2];
    fields.set(match[1], rawValue.startsWith('"') ? match[3] : rawValue);
  }
  return fields;
}

function parsePackageName(contents) {
  let section = "";
  for (const rawLine of contents.split(/\r?\n/)) {
    const line = stripComment(rawLine).trim();
    if (!line) {
      continue;
    }
    const parsedSection = parseSection(line);
    if (parsedSection !== null) {
      section = parsedSection;
      continue;
    }
    if (section !== "package") {
      continue;
    }
    const pair = parseKeyValue(line);
    if (pair?.key === "name") {
      return parseString(pair.value);
    }
  }
  return null;
}

function parseWorkspaceMembers(contents) {
  const members = [];
  let section = "";
  let inMembers = false;

  for (const rawLine of contents.split(/\r?\n/)) {
    const line = stripComment(rawLine).trim();
    if (!line) {
      continue;
    }

    const parsedSection = parseSection(line);
    if (parsedSection !== null) {
      section = parsedSection;
      inMembers = false;
      continue;
    }

    if (section !== "workspace") {
      continue;
    }

    if (!inMembers && /^members\s*=/.test(line)) {
      inMembers = true;
    }

    if (inMembers) {
      for (const match of line.matchAll(/"([^"]+)"/g)) {
        members.push(match[1]);
      }
      if (line.includes("]")) {
        inMembers = false;
      }
    }
  }

  return members;
}

function parseWorkspaceDependencyPackages(contents) {
  const packagesByAlias = new Map();
  let section = "";

  for (const rawLine of contents.split(/\r?\n/)) {
    const line = stripComment(rawLine).trim();
    if (!line) {
      continue;
    }
    const parsedSection = parseSection(line);
    if (parsedSection !== null) {
      section = parsedSection;
      continue;
    }
    if (section !== "workspace.dependencies") {
      continue;
    }

    const pair = parseKeyValue(line);
    if (!pair) {
      continue;
    }

    const inline = parseInlineTable(pair.value);
    packagesByAlias.set(pair.key, inline?.get("package") ?? pair.key);
  }

  return packagesByAlias;
}

function parseManifestDependencies(contents) {
  const deps = [];
  let section = "";

  for (const rawLine of contents.split(/\r?\n/)) {
    const line = stripComment(rawLine).trim();
    if (!line) {
      continue;
    }

    const parsedSection = parseSection(line);
    if (parsedSection !== null) {
      section = parsedSection;
      continue;
    }

    if (!dependencySection(section)) {
      continue;
    }

    const pair = parseKeyValue(line);
    if (!pair) {
      continue;
    }

    const inline = parseInlineTable(pair.value);
    deps.push({
      alias: pair.key,
      packageName: inline?.get("package") ?? null,
      path: inline?.get("path") ?? null,
      workspace: inline?.get("workspace") === "true",
      section,
    });
  }

  return deps;
}

function inheritsWorkspaceLints(contents) {
  let section = "";
  for (const rawLine of contents.split(/\r?\n/)) {
    const line = stripComment(rawLine).trim();
    if (!line) {
      continue;
    }
    const parsedSection = parseSection(line);
    if (parsedSection !== null) {
      section = parsedSection;
      continue;
    }
    const pair = parseKeyValue(line);
    if (section === "lints" && pair?.key === "workspace") {
      return pair.value === "true";
    }
  }
  return false;
}

function normalizeRelativePath(baseDir, relativePath) {
  return path.relative(repoRoot, path.resolve(baseDir, relativePath)).replaceAll("\\", "/") || ".";
}

async function readWorkspace(rootManifestPath) {
  const rootContents = await fs.readFile(rootManifestPath, "utf8");
  const workspaceDeps = parseWorkspaceDependencyPackages(rootContents);
  const memberPaths = parseWorkspaceMembers(rootContents);
  const packageByMemberPath = new Map();
  const packages = [];

  for (const memberPath of memberPaths) {
    const manifestPath = path.join(repoRoot, memberPath, "Cargo.toml");
    const contents = await fs.readFile(manifestPath, "utf8");
    const name = parsePackageName(contents);
    if (!name) {
      throw new Error(`missing [package].name in ${path.relative(repoRoot, manifestPath)}`);
    }
    const normalizedMemberPath = memberPath.replaceAll("\\", "/");
    packageByMemberPath.set(normalizedMemberPath, name);
    packages.push({
      name,
      memberPath: normalizedMemberPath,
      manifestPath: path.relative(repoRoot, manifestPath).replaceAll("\\", "/"),
      manifestDir: path.dirname(manifestPath),
      contents,
    });
  }

  const workspacePackageNames = new Set(packages.map((pkg) => pkg.name));
  const edges = [];

  for (const pkg of packages) {
    for (const dep of parseManifestDependencies(pkg.contents)) {
      let depPackageName = dep.packageName;
      if (!depPackageName && dep.workspace) {
        depPackageName = workspaceDeps.get(dep.alias) ?? dep.alias;
      }
      if (!depPackageName && dep.path) {
        depPackageName = packageByMemberPath.get(normalizeRelativePath(pkg.manifestDir, dep.path)) ?? dep.alias;
      }
      depPackageName ??= dep.alias;

      if (!workspacePackageNames.has(depPackageName)) {
        continue;
      }

      edges.push({
        from: pkg.name,
        to: depPackageName,
        alias: dep.alias,
        section: dep.section,
        manifestPath: pkg.manifestPath,
      });
    }
  }

  return { packages, edges };
}

function findViolations(workspace) {
  const packagesByName = new Map(workspace.packages.map((pkg) => [pkg.name, pkg]));
  const violations = [];

  for (const edge of workspace.edges) {
    const fromPackage = packagesByName.get(edge.from);
    for (const rule of RULES) {
      if (!rule.from(fromPackage) || !rule.to.has(edge.to)) {
        continue;
      }
      violations.push({ ...edge, rule });
      break;
    }
  }

  return violations;
}

function printViolations(violations) {
  process.stderr.write(
    `crate-boundary-guard: found ${violations.length} forbidden workspace dependency edge(s)\n`,
  );
  for (const violation of violations) {
    process.stderr.write(
      [
        `- ${violation.from} -> ${violation.to} via ${violation.alias} (${violation.section})`,
        `  manifest: ${violation.manifestPath}`,
        `  rule: ${violation.rule.id}`,
        `  why: ${violation.rule.reason}`,
        `  fix: ${violation.rule.fix}`,
      ].join("\n") + "\n",
    );
  }
}

async function runGuard() {
  const workspace = await readWorkspace(path.join(repoRoot, "Cargo.toml"));
  const violations = findViolations(workspace);
  if (violations.length > 0) {
    printViolations(violations);
    process.exitCode = 1;
    return;
  }
  const missingWorkspaceLints = workspace.packages.filter(
    (pkg) => !inheritsWorkspaceLints(pkg.contents),
  );
  if (missingWorkspaceLints.length > 0) {
    for (const pkg of missingWorkspaceLints) {
      process.stderr.write(
        `${pkg.manifestPath}: workspace crates must inherit [workspace.lints] with [lints] workspace = true\n`,
      );
    }
    process.exitCode = 1;
    return;
  }

  process.stdout.write(
    `crate-boundary-guard: OK (${workspace.packages.length} packages, ${workspace.edges.length} workspace edges checked)\n`,
  );
}

function assertEqual(actual, expected, label) {
  const actualJson = JSON.stringify(actual);
  const expectedJson = JSON.stringify(expected);
  if (actualJson !== expectedJson) {
    throw new Error(`${label}: expected ${expectedJson}, got ${actualJson}`);
  }
}

function runSelfTest() {
  const rootManifest = [
    "[workspace]",
    'members = [".", "crates/prodex-app", "crates/prodex-terminal-ui"]',
    "",
    "[workspace.dependencies]",
    'app_alias = { package = "prodex-app", path = "crates/prodex-app" }',
    'runtime_proxy = { package = "prodex-runtime-proxy", path = "crates/prodex-runtime-proxy" }',
  ].join("\n");
  const terminalManifest = [
    "[package]",
    'name = "prodex-terminal-ui"',
    "",
    "[lints]",
    "workspace = true",
    "",
    "[dependencies]",
    "app_alias = { workspace = true }",
    'local_runtime = { package = "prodex-runtime-proxy", path = "../prodex-runtime-proxy" }',
  ].join("\n");

  assertEqual(parseWorkspaceMembers(rootManifest), [".", "crates/prodex-app", "crates/prodex-terminal-ui"], "members");
  assertEqual(
    [...parseWorkspaceDependencyPackages(rootManifest).entries()],
    [
      ["app_alias", "prodex-app"],
      ["runtime_proxy", "prodex-runtime-proxy"],
    ],
    "workspace deps",
  );
  assertEqual(parsePackageName(terminalManifest), "prodex-terminal-ui", "package name");
  assertEqual(inheritsWorkspaceLints(terminalManifest), true, "workspace lints");
  assertEqual(
    inheritsWorkspaceLints('[package]\nname = "missing-lints"'),
    false,
    "missing workspace lints",
  );
  assertEqual(
    parseManifestDependencies(terminalManifest).map((dep) => ({
      alias: dep.alias,
      packageName: dep.packageName,
      path: dep.path,
      workspace: dep.workspace,
    })),
    [
      { alias: "app_alias", packageName: null, path: null, workspace: true },
      {
        alias: "local_runtime",
        packageName: "prodex-runtime-proxy",
        path: "../prodex-runtime-proxy",
        workspace: false,
      },
    ],
    "dependencies",
  );

  const violations = findViolations({
    packages: [
      { name: "prodex-app" },
      { name: "prodex-provider-core" },
      { name: "prodex-terminal-ui" },
      { name: "prodex-runtime-proxy" },
    ],
    edges: [
      {
        from: "prodex-provider-core",
        to: "prodex-app",
        alias: "prodex_app",
        section: "dependencies",
        manifestPath: "crates/prodex-provider-core/Cargo.toml",
      },
      {
        from: "prodex-terminal-ui",
        to: "prodex-app",
        alias: "app_alias",
        section: "dependencies",
        manifestPath: "crates/prodex-terminal-ui/Cargo.toml",
      },
      {
        from: "prodex-runtime-proxy",
        to: "prodex-terminal-ui",
        alias: "terminal_ui",
        section: "dependencies",
        manifestPath: "crates/prodex-runtime-proxy/Cargo.toml",
      },
    ],
  });
  assertEqual(
    violations.map((violation) => violation.rule.id),
    [
      "low-level-crates-stay-below-orchestration",
      "terminal-ui-stays-generic",
      "runtime-proxy-stays-hot-path-helper",
    ],
    "violations",
  );

  process.stdout.write("crate-boundary-guard: self-test OK\n");
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }
  if (args.selfTest) {
    runSelfTest();
    return;
  }
  await runGuard();
}

main().catch((error) => {
  process.stderr.write(`crate-boundary-guard: ${error.message}\n`);
  process.exitCode = 1;
});
