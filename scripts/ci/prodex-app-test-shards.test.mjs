import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import test from "node:test";
import {
  ciGithubMatrix,
  githubMatrix,
  PRODEX_APP_LIB_FILTERS,
  PRODEX_APP_FULL_TEST_SHARDS,
  PRODEX_APP_LIB_SHARDS,
  validateShards,
  windowsGithubMatrix,
} from "./prodex-app-test-shards.mjs";

function runPlanner(...args) {
  return spawnSync(process.execPath, ["scripts/ci/prodex-app-test-shards.mjs", ...args], {
    cwd: process.cwd(),
    encoding: "utf8",
  });
}

test("prodex-app shard manifest is disjoint and remainder-complete", () => {
  assert.deepEqual(validateShards(), []);
  assert.deepEqual(validateShards(PRODEX_APP_FULL_TEST_SHARDS), []);
  assert.equal(PRODEX_APP_LIB_SHARDS.length, 11);
  assert.equal(
    PRODEX_APP_FULL_TEST_SHARDS.filter((shard) => shard.filters?.length > 1).length,
    0,
  );
  assert.deepEqual(
    PRODEX_APP_LIB_SHARDS.filter((shard) => shard.filters).flatMap((shard) => shard.filters),
    PRODEX_APP_LIB_FILTERS,
  );
  assert.deepEqual(PRODEX_APP_LIB_SHARDS.at(-1).skipFilters, PRODEX_APP_LIB_FILTERS);
  assert.equal(PRODEX_APP_LIB_SHARDS.find((shard) => shard.suite === "admission-core").filters.length, 8);
  assert.equal(PRODEX_APP_LIB_SHARDS.find((shard) => shard.suite === "admission-affinity").filters.length, 5);

  const appMatrix = githubMatrix();
  const ciMatrix = ciGithubMatrix();
  const fullMatrix = githubMatrix({ includeWorkspace: true });
  const windowsMatrix = windowsGithubMatrix();
  assert.equal(appMatrix.include.length, 11);
  assert.equal(ciMatrix.include.length, 8);
  assert.equal(fullMatrix.include.length, PRODEX_APP_FULL_TEST_SHARDS.length + 4);
  assert.equal(windowsMatrix.include.length, 13);
  assert.equal(appMatrix.include.filter((entry) => entry.save_cache).length, 1);
  assert.equal(ciMatrix.include.filter((entry) => entry.save_cache).length, 1);
  assert.equal(fullMatrix.include.filter((entry) => entry.save_cache).length, 1);
  assert.equal(windowsMatrix.include.filter((entry) => entry.save_cache).length, 1);
  assert.equal(fullMatrix.include[0].suite, "workspace");
  for (const [name, matrix] of [
    ["app", appMatrix],
    ["CI", ciMatrix],
    ["full", fullMatrix],
    ["Windows", windowsMatrix],
  ]) {
    for (const entry of matrix.include) {
      const hasFilters = entry.filters.trim() !== "";
      const hasSkipFilters = entry.skip_filters.trim() !== "";
      assert.equal(
        hasFilters || hasSkipFilters || entry.suite === "workspace",
        true,
        `${name} matrix has an empty shard: ${entry.suite}`,
      );
    }
  }
  const ciFilters = ciMatrix.include.flatMap((entry) => entry.filters.split("\n")).filter(Boolean);
  assert.deepEqual(
    new Set(ciFilters),
    new Set(PRODEX_APP_LIB_FILTERS.filter((filter) => !filter.startsWith("main_internal_tests::"))),
  );
  assert.deepEqual(
    ciMatrix.include.at(-1).skip_filters.split("\n"),
    [
      ...PRODEX_APP_LIB_FILTERS.filter((filter) => !filter.startsWith("main_internal_tests::")),
      "main_internal_tests::",
      "profile_commands_internal_tests::",
    ],
  );
  for (const entry of ciMatrix.include) {
    const skips = entry.skip_filters.split("\n");
    assert.ok(skips.includes("main_internal_tests::"));
    assert.ok(skips.includes("profile_commands_internal_tests::"));
  }
  assert.equal(
    ciMatrix.include.find((entry) => entry.suite === "launch-providers").filters.split("\n").length,
    8,
  );
  const fullFilters = fullMatrix.include
    .slice(1, PRODEX_APP_FULL_TEST_SHARDS.length)
    .flatMap((entry) => entry.filters.split("\n"))
    .filter(Boolean);
  assert.deepEqual(new Set(fullFilters), new Set(PRODEX_APP_LIB_FILTERS));
  assert.equal(
    fullMatrix.include.filter((entry) => entry.suite.startsWith("launch-providers-")).length,
    8,
  );
  const windowsBaseFilters = windowsMatrix.include
    .slice(0, 6)
    .flatMap((entry) => entry.filters.split("\n"))
    .filter(Boolean);
  assert.equal(windowsBaseFilters.length, PRODEX_APP_LIB_FILTERS.length);
  assert.deepEqual(new Set(windowsBaseFilters), new Set(PRODEX_APP_LIB_FILTERS));

  const extra = windowsMatrix.include.slice(6, 12);
  assert.deepEqual(extra.map((entry) => entry.suite), [
    "remainder-continuations-http",
    "remainder-continuations-websocket",
    "remainder-continuations-other",
    "remainder-selection-recovery",
    "remainder-selection",
    "remainder-foundation",
  ]);
  const extraFilters = extra.flatMap((entry) => entry.filters.split("\n")).filter(Boolean);
  assert.equal(extraFilters.length, 15);
  assert.equal(extra.find((entry) => entry.suite === "remainder-selection").skip_filters.split("\n").length, 21);
  assert.equal(extra.find((entry) => entry.suite === "remainder-selection-recovery").filters.split("\n").length, 5);
  assert.deepEqual(
    extra.find((entry) => entry.suite === "remainder-continuations-other").skip_filters.split("\n"),
    [
      "main_internal_tests::runtime_proxy_continuations::http_followups::",
      "main_internal_tests::runtime_proxy_continuations::websocket_",
    ],
  );
  assert.deepEqual(
    windowsMatrix.include.at(-1).skip_filters.split("\n"),
    [...PRODEX_APP_LIB_FILTERS, ...extraFilters],
  );
});

test("Windows critical-path partitions preserve targeted and recovery ownership without overlap", () => {
  const entries = windowsGithubMatrix().include;
  assert.equal(entries.length, 13);
  assert.equal(entries.filter((entry) => entry.save_cache).length, 1);
  assert.deepEqual(entries.slice(3, 6).map((entry) => entry.suite), [
    "selection-admission",
    "provider-launches",
    "runtime-proxy",
  ]);
  const targetedFilters = entries.slice(0, 6).flatMap((entry) =>
    entry.filters.split("\n").filter(Boolean),
  );
  assert.deepEqual(new Set(targetedFilters), new Set(PRODEX_APP_LIB_FILTERS));
  assert.equal(targetedFilters.length, PRODEX_APP_LIB_FILTERS.length);

  const recovery = entries.find((entry) => entry.suite === "remainder-selection-recovery");
  const remaining = entries.find((entry) => entry.suite === "remainder-selection");
  assert.ok(recovery && remaining);
  const recoveryFilters = recovery.filters.split("\n").filter(Boolean);
  assert.equal(recoveryFilters.length, 5);
  const selectionSkips = remaining.skip_filters.split("\n").filter(Boolean);
  assert.deepEqual(selectionSkips.slice(-5), recoveryFilters);
  assert.ok(entries.at(-1).skip_filters.includes(recoveryFilters[0]));

  const examples = [
    ["main_internal_tests::runtime_proxy_selection_and_pressure::admission::rotation_matrix::valid", "remainder-selection-recovery"],
    ["main_internal_tests::runtime_proxy_selection_and_pressure::admission::hard_affinity_replay::valid", "remainder-selection-recovery"],
    ["main_internal_tests::runtime_proxy_selection_and_pressure::admission::responses_missing_content_type::valid", "remainder-selection-recovery"],
    ["main_internal_tests::runtime_proxy_selection_and_pressure::admission::responses_overload_recovery::valid", "remainder-selection-recovery"],
    ["main_internal_tests::runtime_proxy_selection_and_pressure::admission::standard_session_recovery::valid", "remainder-selection-recovery"],
    ["main_internal_tests::runtime_proxy_selection_and_pressure::admission::unknown_new_case::valid", "remainder-selection"],
    ["main_internal_tests::runtime_proxy_selection_and_pressure::persistence::session_affinity::valid", "remainder-selection"],
    ["main_internal_tests::runtime_proxy_selection_and_pressure::doctor::state_collect::valid", "remainder-selection"],
    ["main_internal_tests::runtime_proxy_selection_and_pressure::selection::profile_selection::valid", "selection-admission"],
    ["main_internal_tests::runtime_proxy_selection_and_pressure::admission::guards::valid", "gateway-support-affinity"],
    ["runtime_launch::proxy_startup::deepseek::valid", "provider-launches"],
    ["runtime_proxy::tests::valid", "runtime-proxy"],
    ["app_commands::tests::valid", "commands-gemini"],
    ["runtime_kiro_acp::tests::valid", "gateway-support-affinity"],
    ["future_new_module::valid", "remainder"],
  ];
  for (const [testName, expected] of examples) {
    const owners = entries.filter((entry) => {
      const includes = entry.filters.split("\n").filter(Boolean);
      const skips = entry.skip_filters.split("\n").filter(Boolean);
      return (!includes.length || includes.some((f) => testName.includes(f)))
        && !skips.some((f) => testName.includes(f));
    });
    assert.deepEqual(owners.map((entry) => entry.suite), [expected], testName);
  }
});

test("Windows matrix help documents the active partition count", () => {
  const help = runPlanner("--help");
  assert.equal(help.status, 0, help.stderr);
  assert.match(help.stdout, /--windows-github-matrix emits thirteen disjoint Windows partitions/);
});

test("shard planner dry-run and matrix output are compile-free", () => {
  const check = runPlanner("--check");
  assert.equal(check.status, 0, check.stderr);
  assert.match(check.stdout, /11 app shard\(s\), one cache writer/);

  const dryRun = runPlanner("--dry-run");
  assert.equal(dryRun.status, 0, dryRun.stderr);
  assert.match(
    dryRun.stdout,
    new RegExp(`dry-run: ${githubMatrix({ includeWorkspace: true }).include.length} full-test shard\\(s\\)`),
  );
  assert.match(dryRun.stdout, /selection-1: cargo test .*--test-threads=1/);
  assert.match(dryRun.stdout, /remainder: cargo test .*--skip/);

  const matrixRun = runPlanner("--github-matrix");
  assert.equal(matrixRun.status, 0, matrixRun.stderr);
  assert.deepEqual(JSON.parse(matrixRun.stdout), ciGithubMatrix());

  const windowsMatrixRun = runPlanner("--windows-github-matrix");
  assert.equal(windowsMatrixRun.status, 0, windowsMatrixRun.stderr);
  assert.deepEqual(JSON.parse(windowsMatrixRun.stdout), windowsGithubMatrix());

  const fullMatrixRun = runPlanner("--full-test-matrix");
  assert.equal(fullMatrixRun.status, 0, fullMatrixRun.stderr);
  assert.deepEqual(JSON.parse(fullMatrixRun.stdout), githubMatrix({ includeWorkspace: true }));
});

test("CI consumes generated app shards and retains required safety gates", () => {
  const workflow = readFileSync(".github/workflows/ci.yml", "utf8");
  const fullWorkflow = readFileSync(".github/workflows/full-test.yml", "utf8");
  const staticGuards = readFileSync("scripts/ci/static-guards-parallel.mjs", "utf8");

  assert.match(workflow, /prodex_app_matrix: \$\{\{ steps\.prodex-app-matrix\.outputs\.matrix \}\}/);
  assert.match(
    workflow,
    /windows_prodex_app_matrix: \$\{\{ steps\.prodex-app-matrix\.outputs\.windows_matrix \}\}/,
  );
  assert.match(workflow, /matrix: \$\{\{ fromJSON\(needs\.changes\.outputs\.prodex_app_matrix\) \}\}/);
  assert.match(
    workflow,
    /matrix: \$\{\{ fromJSON\(needs\.changes\.outputs\.windows_prodex_app_matrix\) \}\}/,
  );
  assert.match(workflow, /PRODEX_APP_SKIP_FILTERS/);
  assert.match(workflow, /PRODEX_APP_FILTERS/);
  assert.match(workflow, /--test-threads=1/);
  assert.match(workflow, /save-if: \$\{\{ matrix\.save_cache \}\}/);
  assert.match(workflow, /prodex-app shard matched no tests/);
  assert.match(workflow, /"\$\{skip_args\[@\]\}"/);
  assert.match(workflow, /^  profile-commands-internal:/m);
  assert.match(workflow, /^  main-internal-core:/m);
  assert.match(workflow, /^  main-internal-runtime-proxy:/m);
  assert.match(fullWorkflow, /full_test_shards:/);
  assert.match(fullWorkflow, /--full-test-matrix/);
  assert.match(fullWorkflow, /fromJSON\(needs\.full_test_shards\.outputs\.matrix\)/);
  assert.match(fullWorkflow, /prodex-app shard matched no tests/);
  assert.match(fullWorkflow, /- name: Install Node\.js\n        if: matrix\.suite == 'workspace'/);
  assert.match(fullWorkflow, /^  CARGO_PROFILE_DEV_DEBUG: "0"$/m);
  assert.match(fullWorkflow, /^  CARGO_PROFILE_TEST_DEBUG: "0"$/m);

  const macosWorkspace = workflow.match(/\n  macos-workspace:\n([\s\S]*?)\n  process-guard:/)?.[1];
  assert.ok(macosWorkspace, "macOS workspace job missing");
  assert.match(macosWorkspace, /CARGO_PROFILE_DEV_DEBUG: "0"/);
  assert.match(macosWorkspace, /CARGO_PROFILE_TEST_DEBUG: "0"/);
  assert.match(macosWorkspace, /Run native macOS Kiro selector and resume tests/);
  for (const filter of [
    "provider_default_efforts_follow_catalog_for_every_provider",
    "pure_interactive_resolution_prompts_presidio_before_resumed_sub_agent_config",
    "resume_provider_bridge_leaves_the_session_model_authoritative",
    "runtime_kiro_streaming_command_forwards_model_and_effort",
  ]) {
    assert.match(macosWorkspace, new RegExp(filter));
  }

  for (const job of [
    "docs-lint",
    "secret-scan",
    "windows-workspace",
    "windows-prodex-app",
    "process-guard",
  ]) {
    assert.match(workflow, new RegExp(`^  ${job}:`, "m"), `${job} job missing`);
  }
  assert.doesNotMatch(workflow, /^  windows-security:/m);
  assert.ok(PRODEX_APP_LIB_FILTERS.includes("app_commands::"));
  assert.ok(PRODEX_APP_LIB_FILTERS.includes("runtime_broker::"));
  assert.ok(PRODEX_APP_LIB_FILTERS.includes("runtime_gemini_config::"));
  const processGuard = workflow.match(/\n  process-guard:\n([\s\S]*?)\n  compat-replay-gate:/)?.[1];
  assert.ok(processGuard, "process-guard job missing");
  assert.match(processGuard, /RUSTC_WRAPPER: sccache/);
  assert.match(processGuard, /mozilla-actions\/sccache-action@/);
  assert.match(processGuard, /Swatinem\/rust-cache@/);
  assert.match(processGuard, /scripts\/ci\/static-guards-parallel\.mjs/);
  assert.equal(processGuard.includes("node scripts/docs/smart-context-evidence.mjs --check"), false);
  assert.equal(processGuard.match(/if: matrix\.lane == 'static'/g)?.length, 6);
  assert.match(processGuard, /Download runtime Mojo archive/);
  assert.doesNotMatch(
    workflow,
    /PRODEX_MOJO_ARCHIVE: \$\{\{ github\.workspace \}\}\/target\/mojo-runtime\//,
    'prebuilt Mojo archives must stay outside target so rust-cache cannot remove them',
  );
  assert.match(processGuard, /PRODEX_MOJO_ARCHIVE: \$\{\{ github\.workspace \}\}\/\.prodex-ci\/mojo-runtime\/x86_64-unknown-linux-gnu\/libprodex_mojo_core\.a/);
  assert.equal(processGuard.match(/if: matrix\.lane == 'hygiene'/g)?.length, 2);
  assert.equal(processGuard.match(/if: matrix\.lane == 'enterprise-core'/g)?.length, 1);
  assert.equal(processGuard.match(/if: matrix\.lane == 'node'/g)?.length, 1);
  assert.doesNotMatch(processGuard, /enterprise-storage/);
  const telemetry = workflow.match(/\n  ci-duration-telemetry:\n([\s\S]*)$/)?.[1];
  for (const [source, command] of [
    [workflow, "node scripts/docs/lint-markdown.mjs && node scripts/docs/runtime-policy.mjs --self-test && node scripts/docs/runtime-policy.mjs --check"],
    [staticGuards, "scripts/ci/secret-boundary-guard.mjs"],
    [staticGuards, "scripts/ci/crate-boundary-guard.mjs"],
  ]) {
    assert.match(source, new RegExp(command.replace(/[.*+?^${}()|[\\]\\]/g, "\\$&")), `${command} missing`);
  }
});

test("manifest rejects a remainder that drops a targeted filter", () => {
  const broken = PRODEX_APP_LIB_SHARDS.map((shard) =>
    shard.skipFilters ? { ...shard, skipFilters: shard.skipFilters.slice(1) } : shard,
  );
  assert.ok(validateShards(broken).some((issue) => issue.includes("remainder shard")));
});


test("Windows remainder slices stay disjoint with substring-based libtest filters", () => {
  const entries = windowsGithubMatrix().include;
  const representativeNames = [
    "app_commands::tests::pings_all_profiles",
    "main_internal_tests::runtime_proxy_selection_and_pressure::admission::guards::never_rotate_after_commit",
    "main_internal_tests::runtime_proxy_selection_and_pressure::admission::rotation_matrix::rotation_covers_ready_profiles",
    "main_internal_tests::runtime_proxy_selection_and_pressure::persistence::affinity::affinity_saved",
    "main_internal_tests::runtime_proxy_continuations::http_followups::quota_rotates_before_commit",
    "main_internal_tests::runtime_proxy_continuations::websocket_precommit::precommit_retry",
    "runtime_store::io::tests::recovers_cached_profiles",
    "runtime_external_provider_config::tests::normalizes_models",
    "runtime_core_shared::tests::runtime_profile_limits",
    "reports::info::tests::snapshot_preserved",
    "runtime_background::worker_spawn::tests::does_not_duplicate_workers",
    "runtime_thread_index::tests::canonical_thread_identity",
    "main_internal_tests::quota_selection::uses_ready_profiles",
    "runtime_launch::execution::tests::native_process_exit",
    "update_notice::tests::parses_current_version",
  ];
  for (const name of representativeNames) {
    const owners = entries.filter((entry) => {
      const filters = entry.filters.split("\n").filter(Boolean);
      const skips = entry.skip_filters.split("\n").filter(Boolean);
      return (!filters.length || filters.some((filter) => name.includes(filter)))
        && !skips.some((filter) => name.includes(filter));
    });
    assert.equal(owners.length, 1, name + " belongs to " + owners.map((entry) => entry.suite).join(","));
  }
  const selection = entries.find((entry) => entry.suite === "remainder-selection");
  assert.ok(selection.skip_filters.split("\n").some((filter) => filter.includes("admission::guards::")));
  assert.ok(selection.skip_filters.split("\n").some((filter) => filter.includes("admission::compact::")));
  assert.ok(entries.at(-1).skip_filters.includes("main_internal_tests::runtime_proxy_continuations::"));
});


test("Windows HTTP/WebSocket/other continuation partitions preserve exactly one serial owner", () => {
  const entries = windowsGithubMatrix().include;
  const prefix = "main_internal_tests::runtime_proxy_continuations::";
  const sampleCases = [
    ["http_followups::quota::ready_profile", "remainder-continuations-http"],
    ["http_followups::fresh_and_resume::no_replay", "remainder-continuations-http"],
    ["websocket_precommit::keepalive_does_not_commit", "remainder-continuations-websocket"],
    ["websocket_retry_recovery::budget_expired", "remainder-continuations-websocket"],
    ["websocket_invalid_previous_response::reject_missing", "remainder-continuations-websocket"],
    ["http_tool_and_compact::tool_guard", "remainder-continuations-other"],
    ["http_backend_passthrough::native_headers", "remainder-continuations-other"],
    ["http_compact_transparency::static_context", "remainder-continuations-other"],
    ["parallel_capacity::reject_colliding_turns", "remainder-continuations-other"],
    ["post_commit::forbid_replay", "remainder-continuations-other"],
    ["future_continuation_module::new_regression", "remainder-continuations-other"],
  ];
  for (const [name, expected] of sampleCases) {
    const testName = prefix + name;
    const owners = entries.filter((entry) => {
      const filters = entry.filters.split("\n").filter(Boolean);
      const skips = entry.skip_filters.split("\n").filter(Boolean);
      return (!filters.length || filters.some((filter) => testName.includes(filter)))
        && !skips.some((filter) => testName.includes(filter));
    });
    assert.deepEqual(owners.map((entry) => entry.suite), [expected], testName);
  }
  assert.equal(entries.filter((entry) => entry.save_cache).length, 1);
});


test("nightly full suite shards unassigned cases without changing ordinary app or Windows matrices", () => {
  const full = githubMatrix({ includeWorkspace: true });
  const app = githubMatrix();
  const windows = windowsGithubMatrix();
  const names = [
    "full-remainder-continuations",
    "full-remainder-selection",
    "full-remainder-foundation",
    "remainder",
  ];
  const extras = full.include.filter((entry) => names.includes(entry.suite));
  assert.deepEqual(extras.map((entry) => entry.suite), names);
  assert.equal(full.include.filter((entry) => entry.save_cache).length, 1);
  assert.deepEqual(app.include.map((entry) => entry.suite), PRODEX_APP_LIB_SHARDS.map((entry) => entry.suite));
  assert.equal(windows.include.length, 13, "nightly sharding cannot alter Windows CI");

  const foundation = extras.find((entry) => entry.suite === "full-remainder-foundation");
  assert.equal(foundation.filters.split("\n").length, 6);
  const selection = extras.find((entry) => entry.suite === "full-remainder-selection");
  assert.equal(
    selection.skip_filters.split("\n").length,
    PRODEX_APP_LIB_FILTERS.filter((filter) =>
      filter.startsWith("main_internal_tests::runtime_proxy_selection_and_pressure::"),
    ).length,
  );
  const broadSkips = extras.at(-1).skip_filters.split("\n");
  assert.deepEqual(
    broadSkips,
    [
      ...PRODEX_APP_LIB_FILTERS,
      ...extras.slice(0, -1).flatMap((entry) => entry.filters.split("\n")),
    ],
    "the residual suite must exclude all other full-suite owners",
  );

  const corpus = [
    ["main_internal_tests::runtime_proxy_continuations::http_followups::rotates_before_commit", "full-remainder-continuations"],
    ["main_internal_tests::runtime_proxy_continuations::websocket_precommit::keepalive", "full-remainder-continuations"],
    ["main_internal_tests::runtime_proxy_selection_and_pressure::admission::guards::never_replay", "targeted-admission"],
    ["main_internal_tests::runtime_proxy_selection_and_pressure::admission::persistence::affinity_store", "full-remainder-selection"],
    ["main_internal_tests::runtime_proxy_selection_and_pressure::persistence::affinity", "full-remainder-selection"],
    ["runtime_store::io::tests::restores_profiles", "full-remainder-foundation"],
    ["runtime_external_provider_config::tests::catalog_parse", "full-remainder-foundation"],
    ["runtime_core_shared::tests::profile_resolution", "full-remainder-foundation"],
    ["reports::info::tests::status", "full-remainder-foundation"],
    ["runtime_background::worker_spawn::tests::start", "full-remainder-foundation"],
    ["runtime_thread_index::tests::canonical", "full-remainder-foundation"],
    ["main_internal_tests::quota_selection::plans_valid_quota", "remainder"],
    ["runtime_claude_auth::tests::rejects_missing_secret", "remainder"],
    ["future_unassigned_test_module::test_new_case", "remainder"],
  ];
  for (const [testName, expected] of corpus) {
    const owners = full.include.filter((entry) => {
      if (entry.suite === "workspace") return false;
      const filters = entry.filters.split("\n").filter(Boolean);
      const skips = entry.skip_filters.split("\n").filter(Boolean);
      return (!filters.length || filters.some((value) => testName.includes(value)))
        && !skips.some((value) => testName.includes(value));
    });
    if (expected === "targeted-admission") {
      assert.equal(owners.length, 1, testName);
      assert.match(owners[0].suite, /^admission-affinity-/);
    } else {
      assert.deepEqual(owners.map((entry) => entry.suite), [expected], testName);
    }
  }

  const matrixPlan = runPlanner("--full-test-matrix");
  assert.equal(matrixPlan.status, 0, matrixPlan.stderr);
  assert.deepEqual(JSON.parse(matrixPlan.stdout), full);
  const dryRun = runPlanner("--dry-run");
  assert.equal(dryRun.status, 0, dryRun.stderr);
  assert.match(dryRun.stdout, /full-remainder-selection: cargo test [^\n]+ --skip/);
  assert.match(dryRun.stdout, /full-remainder-foundation: cargo test /);
});
