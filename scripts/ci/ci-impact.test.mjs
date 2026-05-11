import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { promisify } from "node:util";
import { classifyChangedPaths, normalizeChangedPath } from "./ci-impact.mjs";
import { buildSteps } from "./changed-tests.mjs";
import {
  CI_IMPACT_GROUPS,
  PATH_GROUP_NAMES,
  ciImpactCategory,
  pathMatchesGroup,
} from "./test-impact-manifest.mjs";
import { workspacePackagesFromCargoMetadata } from "./workspace-metadata.mjs";

const execFileAsync = promisify(execFile);
const SCRIPT_PATH = new URL("./ci-impact.mjs", import.meta.url).pathname;

test("normalizes changed paths", () => {
  assert.equal(normalizeChangedPath("./docs\\runtime-policy.md"), "docs/runtime-policy.md");
});

test("classifies docs and npm release tooling as lightweight", () => {
  const result = classifyChangedPaths([
    "README.md",
    "QUICKSTART.md",
    "docs/runtime-policy.md",
    "package.json",
    "package-lock.json",
    "npm/prodex/package.json",
    "scripts/npm/release-run.mjs",
    "scripts/ci/release-metadata-only-guard.mjs",
    "scripts/ci/version-metadata-release-guard.mjs",
    "scripts/ci/test-impact-manifest.mjs",
  ]);

  assert.equal(result.heavy, false);
  assert.equal(result.reason, "only lightweight docs/npm/release metadata paths changed");
  assert.deepEqual(result.unknownPaths, []);
});

test("ci-impact uses canonical manifest impact groups", () => {
  assert.equal(CI_IMPACT_GROUPS.heavy, PATH_GROUP_NAMES.ciImpactHeavy);
  assert.equal(CI_IMPACT_GROUPS.light, PATH_GROUP_NAMES.ciImpactLight);

  assert.equal(pathMatchesGroup("src/main.rs", CI_IMPACT_GROUPS.heavy), true);
  assert.equal(ciImpactCategory("src/main.rs"), "heavy");
  assert.equal(classifyChangedPaths(["src/main.rs"]).heavyPaths[0], "src/main.rs");

  assert.equal(pathMatchesGroup("README.md", CI_IMPACT_GROUPS.light), true);
  assert.equal(ciImpactCategory("README.md"), "light");
  assert.equal(classifyChangedPaths(["README.md"]).heavy, false);
});

test("classifies Rust, workflow, runtime CI, and stress paths as heavy", () => {
  for (const filePath of [
    "src/main.rs",
    "crates/prodex-app/src/lib.rs",
    "tests/smoke.rs",
    "benches/runtime.rs",
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    ".cargo/config.toml",
    ".github/workflows/ci.yml",
    "scripts/ci/runtime-stress.mjs",
    "scripts/ci/runtime-proxy-shard.mjs",
    "scripts/ci/runtime-test-manifest.mjs",
  ]) {
    const result = classifyChangedPaths([filePath]);
    assert.equal(result.heavy, true, filePath);
    assert.equal(result.heavyPaths[0], filePath);
  }
});

test("defaults empty and unknown path sets to heavy", () => {
  assert.equal(classifyChangedPaths([]).heavy, true);

  const unknownPath = "scripts/ci/benchmark-calibration.mjs";
  assert.equal(ciImpactCategory(unknownPath), "unknown");

  const unknown = classifyChangedPaths([unknownPath]);
  assert.equal(unknown.heavy, true);
  assert.equal(unknown.unknownPaths[0], unknownPath);
  assert.match(unknown.reason, /unknown path/);
});

test("changed-tests uses canonical manifest groups for focused checks", async () => {
  const filePath = "scripts/ci/size-guard.mjs";
  assert.equal(pathMatchesGroup(filePath, PATH_GROUP_NAMES.sizeGuardRelevant), true);

  const labels = (await buildSteps([filePath])).map((step) => step.label);
  assert.deepEqual(labels, [
    "node-check:scripts/ci/size-guard.mjs",
    "size-guard",
    "size-guard-fixtures",
    "allow-attribute-guard",
    "env-mutation-guard",
  ]);
});

test("changed-tests keeps uncategorized node scripts conservative", async () => {
  const filePath = "scripts/ci/benchmark-calibration.mjs";
  assert.equal(pathMatchesGroup(filePath, PATH_GROUP_NAMES.nodeScript), true);
  assert.equal(ciImpactCategory(filePath), "unknown");

  const labels = (await buildSteps([filePath])).map((step) => step.label);
  assert.deepEqual(labels, ["node-check:scripts/ci/benchmark-calibration.mjs"]);
});

test("workspace metadata helper derives package ownership from cargo metadata", () => {
  const packages = workspacePackagesFromCargoMetadata({
    workspace_root: "/repo",
    workspace_members: ["path+file:///repo#prodex-runtime-proxy@0.0.0"],
    packages: [
      {
        id: "path+file:///repo#prodex-runtime-proxy@0.0.0",
        manifest_path: "/repo/crates/prodex-runtime-proxy/Cargo.toml",
        name: "prodex-runtime-proxy",
        targets: [
          {
            kind: ["lib"],
            name: "runtime_proxy",
            src_path: "/repo/crates/prodex-runtime-proxy/src/lib.rs",
          },
        ],
      },
    ],
  });
  assert.equal(packages[0].name, "prodex-runtime-proxy");
  assert.equal(packages[0].crateDir, "crates/prodex-runtime-proxy");
  assert.equal(packages[0].targets[0].srcPath, "crates/prodex-runtime-proxy/src/lib.rs");
});

test("heavy path wins over lightweight paths", () => {
  const result = classifyChangedPaths(["README.md", "crates/prodex-core/src/lib.rs"]);
  assert.equal(result.heavy, true);
  assert.equal(result.heavyPaths[0], "crates/prodex-core/src/lib.rs");
});

test("CLI emits JSON for explicit paths", async () => {
  const { stdout } = await execFileAsync(process.execPath, [SCRIPT_PATH, "--path", "README.md", "--json"]);
  const result = JSON.parse(stdout);

  assert.equal(result.heavy, false);
  assert.equal(result.reason, "only lightweight docs/npm/release metadata paths changed");
  assert.deepEqual(result.paths, ["README.md"]);
});

test("CLI reads git diff paths from base and head", async () => {
  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-ci-impact-git-"));

  try {
    await execFileAsync("git", ["init"], { cwd: tempDir });
    await execFileAsync("git", ["config", "user.email", "prodex@example.invalid"], { cwd: tempDir });
    await execFileAsync("git", ["config", "user.name", "Prodex CI"], { cwd: tempDir });
    await fs.writeFile(path.join(tempDir, "README.md"), "initial\n", "utf8");
    await execFileAsync("git", ["add", "README.md"], { cwd: tempDir });
    await execFileAsync("git", ["commit", "-m", "initial"], { cwd: tempDir });
    const { stdout: baseStdout } = await execFileAsync("git", ["rev-parse", "HEAD"], { cwd: tempDir });
    const base = baseStdout.trim();

    await fs.writeFile(path.join(tempDir, "README.md"), "updated\n", "utf8");
    await execFileAsync("git", ["commit", "-am", "docs"], { cwd: tempDir });
    const { stdout: headStdout } = await execFileAsync("git", ["rev-parse", "HEAD"], { cwd: tempDir });
    const head = headStdout.trim();

    const { stdout } = await execFileAsync(process.execPath, [SCRIPT_PATH, "--base", base, "--head", head, "--json"], {
      cwd: tempDir,
    });
    const result = JSON.parse(stdout);

    assert.equal(result.heavy, false);
    assert.deepEqual(result.paths, ["README.md"]);
  } finally {
    await fs.rm(tempDir, { recursive: true, force: true });
  }
});

test("CLI writes GitHub outputs", async () => {
  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-ci-impact-"));
  const outputPath = path.join(tempDir, "github-output.txt");

  try {
    const { stdout } = await execFileAsync(
      process.execPath,
      [SCRIPT_PATH, "--path", "src/main.rs", "--github-output"],
      {
        env: { ...process.env, GITHUB_OUTPUT: outputPath },
      },
    );

    assert.match(stdout, /^heavy=true\nreason=heavy path matched: src\/main\.rs\n$/);
    assert.equal(await fs.readFile(outputPath, "utf8"), "heavy=true\nreason=heavy path matched: src/main.rs\n");
  } finally {
    await fs.rm(tempDir, { recursive: true, force: true });
  }
});
