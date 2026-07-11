import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { promisify } from "node:util";
import {
  parseCargoPackageName,
  parseWorkspaceMembers,
  runtimeTestMetadataFromManifest,
  workspaceMetadataFromCargoMetadata,
  workspaceMetadataFromCargoToml,
} from "./workspace-metadata.mjs";

const execFileAsync = promisify(execFile);
const SCRIPT_PATH = new URL("./workspace-metadata.mjs", import.meta.url).pathname;

async function writeFile(filePath, contents = "") {
  await fs.mkdir(path.dirname(filePath), { recursive: true });
  await fs.writeFile(filePath, contents, "utf8");
}

test("parses basic Cargo.toml package and workspace members", () => {
  const manifest = [
    "[package]",
    'name = "prodex-fixture" # keep comment outside string',
    "",
    "[workspace]",
    "members = [",
    '  ".",',
    '  "crates/*",',
    "]",
  ].join("\n");

  assert.equal(parseCargoPackageName(manifest), "prodex-fixture");
  assert.deepEqual(parseWorkspaceMembers(manifest), [".", "crates/*"]);
});

test("builds workspace metadata from cargo metadata JSON", () => {
  const metadata = {
    workspace_root: "/repo",
    workspace_members: ["path+file:///repo#prodex@0.1.0", "path+file:///repo/crates/prodex-core#0.1.0"],
    packages: [
      {
        id: "path+file:///repo#prodex@0.1.0",
        name: "prodex",
        manifest_path: "/repo/Cargo.toml",
        targets: [
          { kind: ["bin"], src_path: "/repo/src/main.rs" },
          { kind: ["test"], src_path: "/repo/tests/smoke.rs" },
          { kind: ["custom-build"], src_path: "/repo/build.rs" },
        ],
      },
      {
        id: "path+file:///repo/crates/prodex-core#0.1.0",
        name: "prodex-core",
        manifest_path: "/repo/crates/prodex-core/Cargo.toml",
        targets: [
          { kind: ["lib"], src_path: "/repo/crates/prodex-core/src/lib.rs" },
          { kind: ["bench"], src_path: "/repo/crates/prodex-core/benches/core.rs" },
        ],
      },
      {
        id: "registry+https://example.invalid#serde@1.0.0",
        name: "serde",
        manifest_path: "/cargo/registry/serde/Cargo.toml",
        targets: [{ kind: ["lib"], src_path: "/cargo/registry/serde/src/lib.rs" }],
      },
    ],
  };

  const result = workspaceMetadataFromCargoMetadata(metadata);

  assert.equal(result.source, "cargo-metadata");
  assert.deepEqual(result.packageNames, ["prodex", "prodex-core"]);
  assert.deepEqual(result.crateDirs, [".", "crates/prodex-core"]);
  assert.deepEqual(result.pathGroups.cargoManifests.exact, [
    "Cargo.toml",
    "crates/prodex-core/Cargo.toml",
  ]);
  assert.deepEqual(result.pathGroups.rustSources, {
    exact: ["build.rs"],
    prefixes: ["crates/prodex-core/src/", "src/"],
  });
  assert.deepEqual(result.pathGroups.rustTests, {
    prefixes: ["crates/prodex-core/benches/", "tests/"],
  });
  assert.deepEqual(result.pathGroups.rustAll, {
    exact: ["Cargo.lock", "Cargo.toml", "build.rs", "crates/prodex-core/Cargo.toml"],
    prefixes: ["crates/prodex-core/benches/", "crates/prodex-core/src/", "src/", "tests/"],
  });
});

test("falls back to Cargo.toml workspace members and filesystem path groups", async () => {
  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-workspace-metadata-"));

  try {
    await writeFile(
      path.join(tempDir, "Cargo.toml"),
      [
        "[package]",
        'name = "prodex-fixture"',
        "",
        "[workspace]",
        "members = [",
        '  ".",',
        '  "crates/*",',
        "]",
      ].join("\n"),
    );
    await writeFile(path.join(tempDir, "src/main.rs"), "fn main() {}\n");
    await writeFile(path.join(tempDir, "tests/root.rs"), "#[test] fn root() {}\n");
    await writeFile(
      path.join(tempDir, "crates/prodex-core/Cargo.toml"),
      ['[package]', 'name = "prodex-core"'].join("\n"),
    );
    await writeFile(path.join(tempDir, "crates/prodex-core/src/lib.rs"), "");
    await writeFile(path.join(tempDir, "crates/prodex-core/benches/core.rs"), "");
    await fs.mkdir(path.join(tempDir, "crates/not-a-crate"), { recursive: true });

    const result = await workspaceMetadataFromCargoToml(tempDir);

    assert.equal(result.source, "cargo-toml");
    assert.deepEqual(result.packageNames, ["prodex-fixture", "prodex-core"]);
    assert.deepEqual(result.crateDirs, [".", "crates/prodex-core"]);
    assert.deepEqual(result.pathGroups.rustSources.prefixes, ["crates/prodex-core/src/", "src/"]);
    assert.deepEqual(result.pathGroups.rustTests.prefixes, [
      "crates/prodex-core/benches/",
      "tests/",
    ]);
    assert.deepEqual(
      result.packages.find((pkg) => pkg.name === "prodex-core").targets.map((target) => target.path),
      ["crates/prodex-core/benches/core.rs", "crates/prodex-core/src/lib.rs"],
    );
  } finally {
    await fs.rm(tempDir, { recursive: true, force: true });
  }
});

test("derives runtime test metadata from manifest-shaped data", () => {
  const result = runtimeTestMetadataFromManifest({
    RUNTIME_TEST_TAGS: {
      serial: "runtime:serial",
      parallelSafe: "runtime:parallel-safe",
    },
    RUNTIME_SMOKE_TESTS: [
      {
        label: "header-preservation",
        package: "prodex-runtime-proxy",
        filter: "request_header_skip_list_preserves_codex_metadata_headers",
      },
    ],
    RUNTIME_CI_TEST_CASES: [
      {
        package: "prodex-app",
        filter: "main_internal_tests::runtime_proxy_",
        tags: ["runtime:parallel-safe"],
      },
      {
        name: "runtime_proxy_env_sensitive_fixture",
        tags: ["runtime:serial"],
      },
    ],
    RUNTIME_CI_WORKFLOW_SHARDS: [
      {
        suite: "root",
        label: "root proxy helpers",
        filters: [{ label: "broker", filter: "main_internal_tests::info_and_broker::" }],
      },
    ],
  });

  assert.deepEqual(result.packageNames, ["prodex-app", "prodex-runtime-proxy"]);
  assert.deepEqual(result.tags, ["runtime:parallel-safe", "runtime:serial"]);
  assert.deepEqual(result.counts, {
    smokeTests: 1,
    ciTestCases: 2,
    workflowShards: 1,
  });
  assert.deepEqual(result.workflowShards, [
    {
      suite: "root",
      label: "root proxy helpers",
      filterCount: 1,
    },
  ]);
  assert.deepEqual(result.filters, [
    "main_internal_tests::info_and_broker::",
    "main_internal_tests::runtime_proxy_",
    "request_header_skip_list_preserves_codex_metadata_headers",
    "runtime_proxy_env_sensitive_fixture",
  ]);
});

test("CLI can read Cargo.toml fallback metadata without cargo", async () => {
  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-workspace-metadata-cli-"));

  try {
    await writeFile(
      path.join(tempDir, "Cargo.toml"),
      ["[package]", 'name = "prodex-fixture"', "", "[workspace]", 'members = ["."]'].join("\n"),
    );
    await writeFile(path.join(tempDir, "src/main.rs"), "fn main() {}\n");

    const { stdout } = await execFileAsync(process.execPath, [
      SCRIPT_PATH,
      "--source",
      "cargo-toml",
      "--repo-root",
      tempDir,
      "--json",
    ]);
    const result = JSON.parse(stdout);

    assert.equal(result.workspace.source, "cargo-toml");
    assert.deepEqual(result.workspace.packageNames, ["prodex-fixture"]);
    assert.deepEqual(result.workspace.pathGroups.rustSources.prefixes, ["src/"]);
  } finally {
    await fs.rm(tempDir, { recursive: true, force: true });
  }
});
