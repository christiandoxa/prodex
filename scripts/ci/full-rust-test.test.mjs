import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import test from "node:test";
import {
  RUNTIME_CI_BROAD_SHARD_FILTERS,
  RUNTIME_CI_WORKFLOW_SHARDS,
} from "./runtime-test-manifest.mjs";

test("full Rust runner includes the explicitly disabled prodex-app lib target", () => {
  const result = spawnSync(
    process.execPath,
    ["scripts/ci/full-rust-test.mjs", "--dry-run", "--jobs", "4"],
    { cwd: process.cwd(), encoding: "utf8" },
  );

  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /prebuild:prodex-bin: cargo build --locked --bin prodex/);
  assert.match(
    result.stdout,
    /prodex-app:non-main-internal: cargo test --locked -q -p prodex-app --lib -- --test-threads=1 --skip main_internal_tests::/,
  );
  assert.match(
    result.stdout,
    /prodex-app:runtime-proxy-selection: cargo test --locked -q -p prodex-app --lib main_internal_tests::runtime_proxy_selection_and_pressure:: -- --test-threads=1/,
  );
  assert.match(result.stdout, /dry-run: 8 parallel step\(s\), jobs=4/);
  assert.match(result.stdout, /dry-run: 1 parallel step\(s\), jobs=4/);
  assert.ok(
    result.stdout.indexOf("prodex-app:runtime-proxy-selection:") <
      result.stdout.lastIndexOf("auto-rotate-shards:"),
    "auto-rotate should run after the heavy prodex-app partition",
  );
  assert.doesNotMatch(result.stdout, /full-rust-test:prodex-app/);

  const platformResult = spawnSync(
    process.execPath,
    ["scripts/ci/full-rust-test.mjs", "--dry-run", "--no-prodex-app-lib"],
    { cwd: process.cwd(), encoding: "utf8" },
  );
  assert.equal(platformResult.status, 0, platformResult.stderr);
  assert.doesNotMatch(platformResult.stdout, /prodex-app.*lib/);
  assert.ok(
    platformResult.stdout.indexOf("workspace:parallel-safe:") <
      platformResult.stdout.lastIndexOf("auto-rotate-shards:"),
    "auto-rotate should follow workspace tests when prodex-app lib is disabled",
  );
});

test("full Rust runner locks every direct cargo test command", () => {
  const result = spawnSync(process.execPath, ["scripts/ci/full-rust-test.mjs", "--dry-run"], {
    cwd: process.cwd(),
    encoding: "utf8",
  });

  assert.equal(result.status, 0, result.stderr);
  const cargoTestLines = result.stdout.split("\n").filter((line) => line.includes(": cargo test "));
  assert.ok(cargoTestLines.length > 0);
  assert.ok(cargoTestLines.every((line) => line.includes("cargo test --locked ")));
  const workspaceLine = cargoTestLines.find((line) => line.includes("workspace:parallel-safe:"));
  assert.match(
    workspaceLine,
    /--skip ping::ping_openai_sends_ping_to_each_ready_openai_profile/,
  );
});

test("no-prodex-app mode excludes prodex-app from workspace execution", () => {
  const result = spawnSync(
    process.execPath,
    ["scripts/ci/full-rust-test.mjs", "--dry-run", "--no-prodex-app-lib"],
    { cwd: process.cwd(), encoding: "utf8" },
  );

  assert.equal(result.status, 0, result.stderr);
  const workspaceCommand = result.stdout
    .split("\n")
    .find((line) => line.includes("workspace:parallel-safe: cargo test "));
  assert.ok(workspaceCommand, "workspace test command missing");
  assert.match(workspaceCommand, /--workspace --exclude prodex-app -- --test-threads/);
  assert.doesNotMatch(workspaceCommand, /--all-features/);
});

test("scheduled full suite runs disjoint workspace and prodex-app partitions in parallel", () => {
  const workflow = readFileSync(".github/workflows/full-test.yml", "utf8");
  const archive = workflow.match(/\n  mojo-runtime-linux-archive:\n([\s\S]*?)\n  full_test_shards:/)?.[1];

  assert.ok(archive, "full-test Mojo runtime archive job missing");
  assert.match(archive, /PRODEX_MOJO_REQUIRED=1/);
  assert.match(workflow, /needs: \[full_test_shards, mojo-runtime-linux-archive\]/);
  assert.match(workflow, /name: Download runtime Mojo archive/);
  assert.match(workflow, /name: Full tests \(\$\{\{ matrix\.label \}\}\)/);
  assert.match(workflow, /full_test_shards:/);
  assert.match(workflow, /--full-test-matrix/);
  assert.match(workflow, /matrix: \$\{\{ fromJSON\(needs\.full_test_shards\.outputs\.matrix\) \}\}/);
  assert.match(workflow, /--timings-json \\\n\s+--no-prodex-app-lib/);
  assert.match(workflow, /matrix\.skip_filters/);
  assert.match(workflow, /--test-threads=1/);
  assert.match(workflow, /Test temp-backed state with a symlinked TMPDIR[\s\S]*?if: matrix\.suite == 'remainder'/);
  assert.match(workflow, /filter_status=\$\?/);
  assert.doesNotMatch(workflow, /if \[ "\$\{status\}" -ne 0 \]; then\n\s+break/);
});

test("fresh benchmark compiles required Mojo runtime with the pinned toolchain", () => {
  const workflow = readFileSync(".github/workflows/bench-calibration.yml", "utf8");

  assert.match(workflow, /MOJO_VERSION: "1\.1\.0"/);
  assert.match(workflow, /Install pinned Mojo toolchain[\s\S]*?mojo==\$\{MOJO_VERSION\}/);
  assert.match(workflow, /"PRODEX_MOJO_REQUIRED=1"/);
  assert.match(workflow, /"PRODEX_MOJO_VERSION=\$\{MOJO_VERSION\}"/);
});

test("native builds link strict runtime archives while the real Mojo lane owns all-feature lint", () => {
  const ci = readFileSync(".github/workflows/ci.yml", "utf8");
  const full = readFileSync(".github/workflows/full-test.yml", "utf8");
  const mojoJob = ci.match(/\n  real-mojo:\n([\s\S]*?)\n  ci-duration-telemetry:/);
  const linuxArchive = ci.match(/\n  mojo-runtime-linux-archive:\n([\s\S]*?)\n  mojo-runtime-platform-archives:/);

  assert.ok(mojoJob, "real-mojo job missing");
  assert.ok(linuxArchive, "Linux runtime archive job missing");
  assert.doesNotMatch(ci.replace(mojoJob[0], ""), /--all-features/);
  assert.doesNotMatch(full, /--all-features/);
  assert.match(linuxArchive[0], /PRODEX_MOJO_REQUIRED=1/);
  assert.match(linuxArchive[0], /cargo build --release --locked --target .* -p prodex-mojo-core/);
  assert.match(ci, /PRODEX_MOJO_ARCHIVE: \$\{\{ github\.workspace \}\}\/\.prodex-ci\/mojo-runtime\//);
  assert.match(mojoJob[0], /name: Real Mojo \/ parity/);
  assert.match(mojoJob[0], /Install pinned Mojo toolchain/);
  assert.match(mojoJob[0], /cargo clippy .* --all-features -- -D warnings/);
  assert.match(mojoJob[0], /cargo build --locked --features mojo-core --bin prodex/);
});

test("slow independent CI phases fan out without dropping their gates", () => {
  const workflow = readFileSync(".github/workflows/ci.yml", "utf8");
  const supplyChain = workflow.match(/\n  supply-chain:\n([\s\S]*?)\n  release-sync:/)?.[1];
  const mojo = workflow.match(/\n  real-mojo:\n([\s\S]*?)\n  ci-duration-telemetry:/)?.[1];

  assert.ok(supplyChain, "supply-chain job missing");
  assert.ok(mojo, "real-mojo job missing");
  assert.match(supplyChain, /Install cargo supply-chain tools in parallel/);
  for (const command of [
    "run_install cargo-audit install_tool cargo-audit cargo-audit --locked --version 0.22.1",
    "run_install cargo-deny install_tool cargo-deny cargo-deny --locked --version 0.19.0",
    "run_install cargo-machete install_tool cargo-machete cargo-machete --locked --version 0.9.2",
  ]) {
    assert.ok(supplyChain.includes(command));
  }
  for (const command of ["cargo audit", "cargo deny check advisories bans licenses sources", "cargo machete --with-metadata"]) {
    assert.ok(supplyChain.includes(command));
  }
  assert.match(supplyChain, /wait "\$\{pids\[index\]\}"/);
  assert.match(supplyChain, /CARGO_TARGET_DIR=.*cargo install --root/);
  assert.match(mojo, /Run remaining Mojo parity tests in parallel/);
  for (const command of [
    "run_test provider-core cargo test --locked -q -p prodex-provider-core --features mojo -- --test-threads=1",
    "run_test runtime-tuning cargo test --locked -q -p prodex-runtime-tuning --features mojo -- --test-threads=1",
    "run_test context cargo test --locked -q -p prodex-context --features mojo -- --test-threads=1",
    "run_test runtime-policy cargo test --locked -q -p prodex-runtime-policy --features mojo -- --test-threads=1",
    "run_test gateway-constraints cargo test --locked -q -p prodex-app --features mojo-core --lib resolved_gateway_request_constraints -- --test-threads=1",
    "run_test smart-context-rehydrate cargo test --locked -q -p prodex-app --features mojo-core --lib smart_context_auto_rehydrate_plan_defers_over_budget_refs -- --test-threads=1",
    "run_test mojo-core cargo test --locked -q -p prodex-mojo-core --features mojo-core -- --test-threads=1",
  ]) {
    assert.ok(mojo.includes(command));
  }
  assert.match(mojo, /wait "\$\{pids\[index\]\}"/);
});

test("push CI reuses the disjoint prodex-app library partitions", () => {
  const workflow = readFileSync(".github/workflows/ci.yml", "utf8");
  const job = workflow.match(/\n  prodex-app-lib:\n([\s\S]*?)\n  fuzz-build:/)?.[1];
  const telemetry = workflow.match(/\n  ci-duration-telemetry:\n([\s\S]*)/)?.[1];

  assert.ok(job, "prodex-app-lib job missing");
  assert.ok(telemetry, "ci-duration-telemetry job missing");
  assert.match(job, /matrix: \$\{\{ fromJSON\(needs\.changes\.outputs\.prodex_app_matrix\) \}\}/);
  assert.doesNotMatch(job, /include:/);
  assert.match(job, /CARGO_INCREMENTAL: "0"/);
  assert.match(job, /CARGO_PROFILE_TEST_DEBUG: "0"/);
  assert.match(job, /save-if: \$\{\{ matrix\.save_cache \}\}/);
  assert.match(job, /PRODEX_APP_FILTER/);
  assert.doesNotMatch(job, /Test temp-backed state with a symlinked TMPDIR/);
  assert.match(telemetry, /- prodex-app-lib/);
  assert.doesNotMatch(workflow, /\n  redis-integration:/);
  assert.doesNotMatch(workflow, /\n  backup-restore-drill:/);
});

test("direct targeted workflow lanes reject zero-test matches", () => {
  const workflow = readFileSync(".github/workflows/ci.yml", "utf8");
  const releaseWorkflow = readFileSync(".github/workflows/standalone-release.yml", "utf8");
  for (const [jobName, nextJob, stepName] of [
    ["windows-workspace", "windows-prodex-app", "Run Windows foundation member tests"],
    ["windows-workspace", "windows-prodex-app", "Run Windows runtime member tests"],
    ["macos-workspace", "process-guard", "Run native macOS broker recovery tests"],
    ["macos-workspace", "process-guard", "Run native macOS Kiro selector and resume tests"],
    ["profile-commands-internal", "main-internal-core", "Run profile command internal tests"],
  ]) {
    const job = workflow.match(new RegExp(`\\n  ${jobName}:\\n([\\s\\S]*?)\\n  ${nextJob}:`))?.[1];
    assert.ok(job, `${jobName} job missing`);
    const stepOffset = job.indexOf(`- name: ${stepName}`);
    assert.ok(stepOffset >= 0, `${jobName}/${stepName} step missing`);
    const step = job.slice(stepOffset);
    assert.match(step, /grep -Fq 'running 0 tests'/, `${jobName}/${stepName} lacks zero-test guard`);
  }

  const releaseBuild = releaseWorkflow.match(/\n  build:\n([\s\S]*?)\n  attest-binaries:/)?.[1];
  assert.ok(releaseBuild, "standalone release build job missing");
  assert.doesNotMatch(releaseBuild, /Test native desktop launcher/);
  assert.doesNotMatch(releaseBuild, /runtime_desktop::tests::/);
  assert.match(releaseBuild, /- name: Build prodex/);
  assert.match(releaseBuild, /cargo build --locked/);

});

test("release hygiene does not inherit an unavailable Rust compiler wrapper", () => {
  const workflow = readFileSync(".github/workflows/ci.yml", "utf8");
  const job = workflow.match(/\n  process-guard:\n([\s\S]*?)\n  supply-chain:/)?.[1];
  const step = job?.match(/- name: Enforce release hygiene([\s\S]*?)- name: Run independent static guards in parallel/)?.[1];

  assert.ok(job, "process-guard job missing");
  assert.ok(step, "release hygiene step missing");
  assert.match(step, /env:\n\s+RUSTC_WRAPPER: ""/);
});

test("release Mojo install activates the compiler in its current step", () => {
  const workflow = readFileSync(".github/workflows/standalone-release.yml", "utf8");
  const mojoJob = workflow.match(
    /\n  build-mojo-archives:[\s\S]*?\n  build:\n/,
  )?.[0];
  const installStep = mojoJob?.match(
    /- name: Install pinned Mojo compiler([\s\S]*?)- name: Install LLVM archive tools/,
  )?.[1];

  assert.ok(installStep, "release Mojo install step missing");
  assert.match(
    installStep,
    /export PATH="\$\{GITHUB_WORKSPACE\}\/\.venv\/bin:\$\{PATH\}"[\s\S]*mojo --version/,
  );
});

test("release validates the Kiro pin before build fan-out", () => {
  const workflow = readFileSync(".github/workflows/standalone-release.yml", "utf8");
  const verifyCi = workflow.match(/\n  verify-ci:\n([\s\S]*?)\n  build:/)?.[1];
  const build = workflow.match(/\n  build:\n([\s\S]*?)\n  artifact-smoke:/)?.[1];

  assert.ok(verifyCi, "release CI verification job missing");
  assert.ok(build, "release build job missing");
  assert.match(build, /timeout-minutes:\s*180/);
  assert.match(workflow, /KIRO_CLI_VERSION:\s*"2\.24\.0"/);
  assert.match(verifyCi, /Verify pinned Kiro CLI release/);
  assert.match(verifyCi, /manifest_version[\s\S]*KIRO_CLI_VERSION/);
  assert.match(build, /needs:\s*[\s\S]*?- verify-ci/);
});

test("release builds Prodex only and leaves Codex external", () => {
  const workflow = readFileSync(".github/workflows/standalone-release.yml", "utf8");
  const build = workflow.match(/\n  build:\n([\s\S]*?)\n  artifact-smoke:/)?.[1];

  assert.ok(build, "release build job missing");
  assert.doesNotMatch(build, /Codex|codex/);
  assert.match(build, /cross build --locked/);
  assert.match(build, /cargo build --locked/);
  assert.match(build, /Prepare binary artifact/);
});

test("release verifies optional-tool freshness on the exact release SHA", () => {
  const workflow = readFileSync(".github/workflows/standalone-release.yml", "utf8");
  const verifyCi = workflow.match(/\n  verify-ci:\n([\s\S]*?)\n  build:/)?.[1];

  assert.ok(verifyCi, "release CI verification job missing");
  assert.match(verifyCi, /Verify optional-tool freshness at release cut/);
  assert.match(verifyCi, /--checkpoint B --release-sha/);
  assert.match(verifyCi, /RELEASE_SHA: \$\{\{ steps\.target\.outputs\.target_sha \}\}/);
});

test("release validates curated notes before creating the tag", () => {
  const workflow = readFileSync(".github/workflows/standalone-release.yml", "utf8");
  const notes = workflow.indexOf("- name: Validate curated release notes before tagging");
  const tag = workflow.indexOf("- name: Create release tag");
  assert.ok(notes >= 0, "pre-tag release-note validation is missing");
  assert.ok(tag > notes, "release notes must be validated before tag creation");
});

test("release waits for every asset provenance check before tagging", () => {
  const workflow = readFileSync(".github/workflows/standalone-release.yml", "utf8");
  const provenance = workflow.indexOf("- name: Verify release provenance");
  const tag = workflow.indexOf("- name: Create release tag");

  assert.ok(provenance >= 0, "release provenance verification is missing");
  assert.ok(tag > provenance, "release provenance must precede tag creation");
  const step = workflow.slice(provenance, tag);
  assert.match(step, /gh attestation verify "\$\{asset\}"/);
  assert.match(step, /pids=\(\)/);
  assert.match(step, /wait "\$\{pids\[index\]\}"/);
  assert.match(step, /exit "\$\{status\}"/);
});

test("runtime proxy matrix is generated before fan-out without a runner barrier", () => {
  const workflow = readFileSync(".github/workflows/ci.yml", "utf8");
  const changes = workflow.match(/\n  changes:\n([\s\S]*?)\n  fmt:/)?.[1];
  const runtimeProxy = workflow.match(/\n  main-internal-runtime-proxy:\n([\s\S]*?)\n  runtime-proxy-bench-smoke:/)?.[1];

  assert.ok(changes, "changes job missing");
  assert.ok(runtimeProxy, "main-internal-runtime-proxy job missing");
  assert.match(changes, /runtime_proxy_matrix: \$\{\{ steps\.runtime-matrix\.outputs\.matrix \}\}/);
  assert.match(changes, /node scripts\/ci\/runtime-proxy-ci-matrix\.mjs --github-matrix/);
  assert.match(runtimeProxy, /needs: \[changes, mojo-runtime-linux-archive\]/);
  assert.match(runtimeProxy, /fromJSON\(needs\.changes\.outputs\.runtime_proxy_matrix\)/);
  assert.doesNotMatch(workflow, /\n  runtime-proxy-shard-matrix:/);
});

test("runtime proxy benchmark remains independently gated with one cache writer", () => {
  const workflow = readFileSync(".github/workflows/ci.yml", "utf8");
  const runtimeProxy = workflow.match(
    /\n  runtime-proxy-bench-smoke:\n([\s\S]*?)\n  runtime-load-smoke:/,
  )?.[1];
  const telemetry = workflow.match(/\n  ci-duration-telemetry:\n([\s\S]*)/)?.[1];

  assert.ok(runtimeProxy, "runtime proxy benchmark job missing");
  assert.ok(telemetry, "CI duration telemetry job missing");
  assert.match(runtimeProxy, /PRODEX_RUNTIME_PROXY_BENCH_CHECK/);
  assert.match(runtimeProxy, /runtime_proxy_hot_paths/);
  assert.doesNotMatch(runtimeProxy, /governance_hot_paths/);
  assert.equal(
    runtimeProxy.match(/save-if: \$\{\{ github\.job == 'runtime-proxy-bench-smoke' \}\}/g)?.length,
    1,
  );
  assert.match(telemetry, /- runtime-proxy-bench-smoke/);
  assert.doesNotMatch(workflow, /\n  runtime-governance-bench-smoke:/);
  assert.doesNotMatch(telemetry, /- runtime-governance-bench-smoke/);
});

test("runtime proxy logical suites pack without losing filters", () => {
  const result = spawnSync(process.execPath, ["scripts/ci/runtime-proxy-ci-matrix.mjs", "--github-matrix"], {
    cwd: process.cwd(),
    encoding: "utf8",
  });

  assert.equal(result.status, 0, result.stderr);
  const matrix = JSON.parse(result.stdout);
  const filters = matrix.include.flatMap((entry) => entry.filters.split("\n"));
  const expectedFilters = RUNTIME_CI_BROAD_SHARD_FILTERS.map(
    ({ label, filter }) => `${label}|${filter}`,
  );
  assert.equal(matrix.include.length, Math.ceil(RUNTIME_CI_WORKFLOW_SHARDS.length / 2));
  const midpoint = Math.ceil(RUNTIME_CI_WORKFLOW_SHARDS.length / 2);
  const tail = RUNTIME_CI_WORKFLOW_SHARDS.slice(midpoint).reverse();
  const expectedSuites = RUNTIME_CI_WORKFLOW_SHARDS.slice(0, midpoint).map((shard, index) =>
    [shard, tail[index]]
      .filter(Boolean)
      .map((candidate) => candidate.suite)
      .join("+"),
  );
  assert.deepEqual(
    matrix.include.map((entry) => entry.suite),
    expectedSuites,
  );
  assert.ok(matrix.include.every((entry) => entry.filters.trim() !== ""));
  assert.equal(matrix.include.filter((entry) => entry.save_cache).length, 1);
  assert.deepEqual(new Set(filters), new Set(expectedFilters));
  const admissionCorePack = matrix.include.find((entry) =>
    entry.filters.includes(
      "|main_internal_tests::runtime_proxy_selection_and_pressure::admission::compact::",
    ),
  );
  const admissionAffinityPack = matrix.include.find((entry) =>
    entry.filters.includes(
      "|main_internal_tests::runtime_proxy_selection_and_pressure::admission::response_affinity::",
    ),
  );
  assert.ok(admissionCorePack, "admission core filter missing from runtime matrix");
  assert.ok(admissionAffinityPack, "admission affinity filter missing from runtime matrix");
  assert.notEqual(admissionCorePack.suite, admissionAffinityPack.suite);
});

test("push CI keeps runtime quarantine while broad stress stays scheduled", () => {
  const matrix = (eventName) => {
    const result = spawnSync(
      process.execPath,
      [
        "scripts/ci/runtime-proxy-ci-matrix.mjs",
        "--github-stress-matrix",
        "--event-name",
        eventName,
      ],
      { cwd: process.cwd(), encoding: "utf8" },
    );
    assert.equal(result.status, 0, result.stderr);
    return JSON.parse(result.stdout).include;
  };

  const push = matrix("push");
  const schedule = matrix("schedule");
  assert.deepEqual(new Set(push.map((entry) => entry.suite)), new Set(["serialized", "continuation"]));
  assert.equal(push.length, 4);
  assert.equal(schedule.length, 9);
  assert.equal(push.filter((entry) => entry.save_cache).length, 1);
  assert.equal(schedule.filter((entry) => entry.save_cache).length, 1);

  const invalid = spawnSync(
    process.execPath,
    ["scripts/ci/runtime-proxy-ci-matrix.mjs", "--github-stress-matrix"],
    { cwd: process.cwd(), encoding: "utf8" },
  );
  assert.notEqual(invalid.status, 0);
  assert.match(invalid.stderr, /unsupported CI event name: missing/);

  const workflow = readFileSync(".github/workflows/ci.yml", "utf8");
  assert.match(workflow, /runtime_stress_matrix: \$\{\{ steps\.runtime-matrix\.outputs\.stress_matrix \}\}/);
  assert.match(workflow, /fromJSON\(needs\.changes\.outputs\.runtime_stress_matrix\)/);
});

test("scheduled CI delegates duplicate Ubuntu suites to the daily full test", () => {
  const workflow = readFileSync(".github/workflows/ci.yml", "utf8");
  for (const [jobName, nextJob] of [
    ["prodex-app-lib", "fuzz-build"],
    ["auto-rotate", "profile-commands-internal"],
    ["profile-commands-internal", "main-internal-core"],
    ["main-internal-core", "env-sensitive-parallel-guard"],
    ["main-internal-runtime-proxy", "runtime-proxy-bench-smoke"],
  ]) {
    const job = workflow.match(new RegExp(`\\n  ${jobName}:\\n([\\s\\S]*?)\\n  ${nextJob}:`))?.[1];
    assert.ok(job, `${jobName} job missing`);
    assert.match(job, /github\.event_name != 'schedule'/);
  }

  const processGuard = workflow.match(/\n  process-guard:\n([\s\S]*?)\n  supply-chain:/)?.[1];
  assert.match(processGuard, /Run independent static guards in parallel[\s\S]*?scripts\/ci\/static-guards-parallel\.mjs/);
});

test("large CI matrices use one cache writer and retain failure diagnostics", () => {
  const workflow = readFileSync(".github/workflows/ci.yml", "utf8");
  const core = workflow.match(/\n  main-internal-core:\n([\s\S]*?)\n  env-sensitive-parallel-guard:/)?.[1];
  const runtime = workflow.match(/\n  main-internal-runtime-proxy:\n([\s\S]*?)\n  runtime-proxy-bench-smoke:/)?.[1];
  const stress = workflow.match(/\n  runtime-stress:\n([\s\S]*?)\n  ci-duration-telemetry:/)?.[1];
  const processGuard = workflow.match(/\n  process-guard:\n([\s\S]*?)\n  supply-chain:/)?.[1];

  assert.equal(core?.match(/save_cache: true/g)?.length, 1);
  assert.match(core, /save-if: \$\{\{ matrix\.save_cache \}\}/);
  assert.match(runtime, /save-if: \$\{\{ matrix\.save_cache \}\}/);
  assert.match(stress, /fromJSON\(needs\.changes\.outputs\.runtime_stress_matrix\)/);
  assert.match(stress, /save-if: \$\{\{ matrix\.save_cache \}\}/);
  assert.match(processGuard, /save-if: \$\{\{ matrix\.lane == 'static' \}\}/);
  assert.match(runtime, /status=0[\s\S]*?status=1[\s\S]*?exit "\$\{status\}"/);
});
