import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const ci = readFileSync(".github/workflows/ci.yml", "utf8");
const release = readFileSync(".github/workflows/standalone-release.yml", "utf8");

function workflowJob(source, name, nextName) {
  const start = source.indexOf("\n  " + name + ":\n");
  const end = source.indexOf("\n  " + nextName + ":\n", start + 1);
  assert.ok(start >= 0 && end > start, "missing job " + name);
  return source.slice(start, end);
}

test("Mojo parity fans out without dropping production tests", () => {
  const lint = workflowJob(ci, "real-mojo", "real-mojo-parity");
  const parity = workflowJob(ci, "real-mojo-parity", "ci-duration-telemetry");
  const archive = workflowJob(ci, "mojo-runtime-linux-archive", "mojo-runtime-platform-archives");
  assert.match(lint, /needs: \[changes, mojo-runtime-linux-archive\]/);
  assert.match(parity, /needs: \[changes, mojo-runtime-linux-archive\]/);
  assert.match(lint, /cargo clippy --locked --workspace --all-targets --all-features -- -D warnings/);
  assert.match(lint, /cargo build --locked --features mojo-core --bin prodex/);
  assert.match(lint, /"implementation": "mojo-compiled-in"/);
  assert.match(lint, /"fallback": false/);
  assert.match(lint, /- name: Download strict Linux Mojo archive/);
  assert.doesNotMatch(lint, /Install pinned Mojo toolchain/);
  assert.match(archive, /Install pinned Mojo toolchain/);
  assert.match(archive, /PRODEX_MOJO_REQUIRED=1/);

  const groups = [...parity.matchAll(/^\s+- group: ([a-z-]+)$/gm)].map((match) => match[1]);
  assert.deepEqual(groups, ["package", "provider", "app-core"]);
  for (const group of groups) {
    assert.ok(parity.includes("if: $" + "{{ matrix.group == '" + group + "' }}"));
  }
  const labels = [...parity.matchAll(/^\s+run_test ([a-z-]+) cargo test --locked -q -p ([a-z-]+) ([^\n]+)$/gm)];
  assert.equal(labels.length, 14, "original 14 tests retained");
  const names = labels.map((match) => match[1]);
  assert.equal(new Set(names).size, 14);
  assert.deepEqual(names, [
    "quota", "runtime-proxy", "runtime-quota", "provider-spi",
    "provider-core", "runtime-tuning", "runtime-json-only", "runtime-launch",
    "context", "runtime-policy", "gateway-constraints",
    "smart-context-rehydrate", "mojo-cli", "mojo-core",
  ]);
  for (const match of labels) {
    assert.match(match[0], /--test-threads=1/, "missing regression boundary for " + match[1]);
  }
  assert.equal([...parity.matchAll(/wait "\$\{pids\[index\]\}"/g)].length, 3);
  assert.match(parity, /PRODEX_MOJO_REQUIRED: "1"/);
  assert.match(parity, /PRODEX_MOJO_ARCHIVE:/);
  assert.match(parity, /fail-fast: false/);
  const telemetry = ci.slice(ci.indexOf("\n  ci-duration-telemetry:\n"));
  assert.match(telemetry, /- real-mojo-parity\n/);
  assert.match(telemetry, /- real-mojo\n/);
});

test("release archives overlap CI verification but binary publication remains gated", () => {
  const verification = workflowJob(release, "verify-ci", "build-mojo-archives");
  const archive = workflowJob(release, "build-mojo-archives", "build");
  const build = workflowJob(release, "build", "attest-binaries");
  const attest = workflowJob(release, "attest-binaries", "prepare-release");
  assert.match(verification, /Wait for CI success/);
  assert.doesNotMatch(archive.split("    steps:")[0], /needs:\s*verify-ci/);
  assert.match(archive, /Validate archive source identity before expensive compilation/);
  assert.match(archive, /RELEASE_TARGET_SHA:/);
  assert.match(archive, /RELEASE_VERSION:/);
  assert.match(archive, /test "\$\{GITHUB_SHA\}" = "\$\{RELEASE_TARGET_SHA\}"/);
  assert.match(archive, /test "\$\{actual_version\}" = "\$\{RELEASE_VERSION\}"/);
  assert.match(archive, /PRODEX_MOJO_REQUIRED=1/);
  assert.match(archive, /Build target Mojo archive/);
  assert.match(archive, /Verify target archive/);
  assert.match(build, /needs:\s*\n\s*- verify-ci\n\s*- build-mojo-archives/);
  assert.match(build, /PRODEX_MOJO_ARCHIVE=/);
  assert.match(build, /Verify compiled-in Mojo release/);
  assert.match(attest, /needs:\s*\n\s*- build\n\s*- verify-ci/);
  assert.match(release, /Verify release provenance/);
  assert.match(release, /sha256sum --check SHA256SUMS/);
  assert.match(release, /Scan release assets for malware/);
});


test("Apple Silicon Intel release benchmark is opt-in, source-pinned and cannot publish", () => {
  const workflow = readFileSync(".github/workflows/release-macos-cross-calibration.yml", "utf8");
  assert.match(workflow, /name: macOS Intel cross-build calibration/);
  assert.match(workflow, /workflow_dispatch:/);
  assert.doesNotMatch(workflow, /^  push:/m);
  assert.match(workflow, /runs-on: macos-14/);
  assert.match(workflow, /permissions:\n  contents: read\n  actions: read/);
  assert.match(workflow, /\.head_sha == \$sha/);
  assert.match(workflow, /\.conclusion == "success"/);
  assert.match(workflow, /\.event == "push"/);
  assert.match(workflow, /- name: Check out exact CI source/);
  assert.match(workflow, /run-id:/);
  assert.match(workflow, /name: mojo-runtime-archive-x86_64-apple-darwin/);
  assert.match(workflow, /PRODEX_MOJO_REQUIRED: "1"/);
  assert.match(workflow, /cargo build --release --locked --target x86_64-apple-darwin/);
  assert.match(workflow, /lipo -archs/);
  assert.match(workflow, /arch -x86_64/);
  assert.match(workflow, /"implementation": "mojo-compiled-in"/);
  assert.doesNotMatch(workflow, /gh release (?:create|upload|edit)/);
  assert.doesNotMatch(workflow, /git push/);
  assert.doesNotMatch(workflow, /attest-build-provenance/);
});


test("official Intel macOS release uses measured Apple Silicon cross-build with Rosetta safety", () => {
  const build = workflowJob(release, "build", "attest-binaries");
  const intel = build.match(/- target: x86_64-apple-darwin\n([\s\S]*?)- target: aarch64-apple-darwin/)?.[1];
  assert.ok(intel, "x86_64 macOS release matrix entry missing");
  assert.match(intel, /runs-on: macos-14/);
  assert.doesNotMatch(intel, /macos-15-intel/);
  assert.match(intel, /can-native-smoke: true/);
  assert.match(intel, /can-mojo-runtime-smoke: true/);

  assert.match(build, /name: Require Rosetta for Intel macOS cross-target smoke/);
  assert.match(build, /test "\$\(uname -m\)" = arm64/);
  assert.match(build, /softwareupdate --install-rosetta --agree-to-license/);
  assert.match(build, /key: release-[^\n]*-arm64-cross/);
  assert.match(build, /name: Verify macOS Intel artifact architecture and Rosetta execution/);
  assert.match(build, /test "\$\(lipo -archs "\$\{binary\}"\)" = x86_64/);
  assert.match(build, /arch -x86_64 "\$\{binary\}" --version/);
  assert.match(build, /name: Verify compiled-in Mojo release/);
  assert.match(build, /"implementation": "mojo-compiled-in"/);
  assert.match(build, /"self_test": "passed"/);
  assert.match(build, /name: Native smoke test/);
  assert.match(build, /name: Upload binary artifact/);
  assert.match(build, /needs:\s*\n\s*- verify-ci\n\s*- build-mojo-archives/);
});


test("standalone release reuses only verified same-SHA CI Mojo archives with strict fallback", () => {
  const verification = workflowJob(release, "verify-ci", "build-mojo-archives");
  const archives = workflowJob(release, "build-mojo-archives", "build");
  const build = workflowJob(release, "build", "attest-binaries");

  assert.match(archives, /actions: read/);
  assert.match(archives, /name: Reuse exact-source strict CI Mojo archive when available/);
  assert.match(archives, /id: reuse-ci-archive/);
  assert.match(archives, /GH_TOKEN:/);
  assert.match(archives, /RELEASE_TARGET_SHA:/);
  assert.match(archives, /python3 scripts\/ci\/reuse_mojo_ci_archive\.py/);
  assert.equal(
    [...archives.matchAll(/if: \$\{\{ steps\.reuse-ci-archive\.outputs\.reused != 'true' && steps\.release-arm64-archive-cache\.outputs\.cache-hit != 'true' \}\}/g)].length,
    4,
    "both verified CI artifact and strict ARM cache must skip the four expensive install and compile steps",
  );
  assert.match(archives, /name: Verify target archive/);
  assert.match(archives, /llvm-nm --format=posix/);
  assert.match(archives, /KGEN_CompilerRT_/);
  assert.match(archives, /name: Upload target Mojo archive/);

  assert.match(verification, /name: Wait for CI success/);
  assert.match(verification, /conclusion.*success/);
  assert.match(build, /needs:\s*\n\s*- verify-ci\n\s*- build-mojo-archives/);
  assert.match(build, /name: Download target Mojo archive/);
  assert.match(build, /name: Verify compiled-in Mojo release/);
  assert.match(release, /Verify release provenance/);
  assert.match(release, /sha256sum --check SHA256SUMS/);
});


test("trusted main archive cache preserves strict Mojo compile and ABI verification for all four targets", () => {
  const linux = workflowJob(ci, "mojo-runtime-linux-archive", "mojo-runtime-platform-archives");
  const platform = workflowJob(ci, "mojo-runtime-platform-archives", "fmt");
  const cacheSource = "mojo/**/*.mojo";
  const requiredHashes = [
    cacheSource,
    "crates/prodex-mojo-core/build.rs",
    "crates/prodex-mojo-core/build/*.rs",
    "crates/prodex-mojo-core/Cargo.toml",
  ];
  for (const block of [linux, platform]) {
    assert.match(block, /uses: actions\/cache@55cc8345863c7cc4c66a329aec7e433d2d1c52a9/);
    assert.match(block, /id: mojo-archive-cache/);
    assert.match(block, /if: \$\{\{ github.event_name == 'push' && github.ref == 'refs\/heads\/main' \}\}/);
    assert.match(block, /mojo-static-v2-/);
    assert.match(block, /rust1\.99\.0/);
    assert.match(block, /env\.MOJO_VERSION/);
    for (const item of requiredHashes) {
      assert.ok(block.includes("'" + item + "'"), "cache fingerprint missing " + item);
    }
    assert.doesNotMatch(block, /restore-keys:/);
    // Workspace release version and unrelated dependency lock changes do
    // not change Mojo compiler objects. The actual Mojo and owner inputs do.
    assert.doesNotMatch(block, /'Cargo\.lock'/);
    assert.doesNotMatch(block, /'Cargo\.toml'/);
    assert.equal(
      [...block.matchAll(/if: \$\{\{ steps\.mojo-archive-cache\.outputs\.cache-hit != 'true' \}\}/g)].length,
      4,
      "Rust/toolchain install, pinned Mojo install, and strict compilation must all run on cache miss",
    );
    assert.match(block, /- name: Ensure LLVM archive tools/);
    assert.match(block, /command -v llvm-nm/);
    assert.match(block, /command -v llvm-ar/);
    assert.match(block, /command -v llvm-lib/);
    assert.match(block, /sudo apt-get install -y llvm/);
    assert.match(block, /- name: Build strict runtime Mojo archive/);
    assert.match(block, /--features mojo-core/);
    assert.match(block, /- name: Upload (?:Linux|platform) runtime Mojo archive/);
  }
  assert.match(linux, /Validate cached or fresh Linux archive/);
  assert.match(platform, /Validate cached or fresh platform archive/);
  assert.match(linux, /prodex_runtime_profile_provider_order_batch/);
  assert.match(platform, /prodex_runtime_profile_rotation_order_batch/);
  assert.match(linux, /KGEN_CompilerRT_/);
  assert.match(platform, /KGEN_CompilerRT_/);
  assert.match(platform, /MSVC Mojo archive contains duplicate weak static-string aliases/);
  assert.match(linux, /key: mojo-static-v2-\$\{\{ runner.os \}\}-x86_64-unknown-linux-gnu-x86-64/);
  for (const variable of ["matrix.target", "matrix.mojo_target_cpu"]) {
    assert.ok(platform.includes(variable), "multi-target cache key missing " + variable);
  }
});


test("release-only ARM64 archive cache is bounded, source-addressed and preserves all six gates", () => {
  const archive = workflowJob(release, "build-mojo-archives", "build");
  const build = workflowJob(release, "build", "attest-binaries");
  const cache = archive.match(/      - name: Restore strict release-only ARM64 Mojo archive cache\n([\s\S]*?)\n      - name: Install Rust target/)?.[1];
  assert.ok(cache);
  assert.match(cache, /id: release-arm64-archive-cache/);
  assert.match(cache, /uses: actions\/cache@55cc8345863c7cc4c66a329aec7e433d2d1c52a9/);
  assert.match(cache, /github\.event_name == 'workflow_dispatch'/);
  assert.match(cache, /github\.ref == 'refs\/heads\/main'/);
  for (const target of ["aarch64-unknown-linux-gnu", "aarch64-pc-windows-msvc"]) {
    assert.match(cache, new RegExp("matrix.target == '" + target + "'"));
  }
  assert.match(cache, /mojo-release-arm64-v1-/);
  assert.match(cache, /matrix\.target/);
  assert.match(cache, /matrix\.mojo-target-cpu/);
  assert.match(cache, /env\.MOJO_VERSION/);
  assert.match(cache, /rust1\.99\.0/);
  assert.match(archive, /- name: Verify target archive\n\s+shell: bash/);
  for (const input of [
    "mojo/**/*.mojo", "crates/prodex-mojo-core/build.rs",
    "crates/prodex-mojo-core/build/*.rs",
    "crates/prodex-mojo-core/Cargo.toml",
  ]) {
    assert.ok(cache.includes("'" + input + "'"), "cache missing " + input);
  }
  assert.doesNotMatch(cache, /'Cargo\.toml'|'Cargo\.lock'/);
  assert.doesNotMatch(cache, /restore-keys:/);
  assert.match(archive, /name: Validate archive source identity before expensive compilation/);
  assert.match(archive, /test "\$\{GITHUB_SHA\}" = "\$\{RELEASE_TARGET_SHA\}"/);
  assert.match(archive, /name: Reuse exact-source strict CI Mojo archive when available/);
  assert.match(archive, /name: Build target Mojo archive/);
  assert.match(archive, /-p prodex-mojo-core --features mojo-core/);
  assert.match(archive, /name: Verify target archive\n\s+shell: bash/);
  assert.doesNotMatch(archive, /name: Verify target archive\n\s+if:/);
  assert.match(archive, /KGEN_CompilerRT_/);
  assert.match(archive, /duplicate_weak=/);
  assert.match(archive, /name: Upload target Mojo archive/);
  assert.match(build, /needs:\s*\n\s*- verify-ci\n\s*- build-mojo-archives/);
  assert.equal((archive.match(/^          - target: /gm) ?? []).length, 6);
});


test("native Windows ARM64 release calibration is source-pinned, opt-in and cannot publish", () => {
  const workflow = readFileSync(".github/workflows/release-windows-arm64-calibration.yml", "utf8");
  assert.match(workflow, /name: Windows ARM64 native release build calibration/);
  assert.match(workflow, /workflow_dispatch:/);
  assert.match(workflow, /runs-on: windows-11-arm/);
  assert.doesNotMatch(workflow, /^  push:/m);
  assert.match(workflow, /permissions:\n  contents: read\n  actions: read/);
  assert.match(workflow, /\.head_sha -cne \$env:TARGET_SHA/);
  assert.match(workflow, /\.event -cne 'workflow_dispatch'/);
  assert.match(workflow, /\.path -cne '\.github\/workflows\/standalone-release.yml'/);
  assert.match(workflow, /\.conclusion -cne 'success'/);
  assert.match(workflow, /\.repository\.full_name -cne \$env:GITHUB_REPOSITORY/);
  assert.match(workflow, /- name: Check out exact release source/);
  assert.match(workflow, /ref: \$\{\{ inputs\.target_sha \}\}/);
  assert.match(workflow, /name: mojo-archive-aarch64-pc-windows-msvc/);
  assert.match(workflow, /run-id: \$\{\{ inputs\.source_release_run_id \}\}/);
  assert.match(workflow, /PRODEX_MOJO_REQUIRED: "1"/);
  assert.match(workflow, /PRODEX_MOJO_ARCHIVE:/);
  assert.match(workflow, /llvm-nm -g/);
  assert.match(workflow, /KGEN_CompilerRT_/);
  assert.match(workflow, /cargo build --release --locked --target aarch64-pc-windows-msvc --features mojo-core --bin prodex/);
  assert.match(workflow, /\$machine -ne 0xAA64/);
  assert.match(workflow, /- name: Run real native ARM64 Mojo smoke/);
  assert.match(workflow, /"implementation": "mojo-compiled-in"/);
  assert.match(workflow, /"fallback": false/);
  assert.match(workflow, /"self_test": "passed"/);
  assert.doesNotMatch(workflow, /gh release (?:create|edit|upload)/);
  assert.doesNotMatch(workflow, /git push/);
  assert.doesNotMatch(workflow, /- name: Upload binary artifact/);
});


test("official Windows ARM64 release uses native benchmark, strict PE and runtime smoke", () => {
  const build = workflowJob(release, "build", "attest-binaries");
  const start = build.indexOf("          - target: aarch64-pc-windows-msvc\n");
  const end = build.indexOf("\n    steps:\n", start);
  assert.ok(start >= 0 && end > start, "official ARM64 matrix entry missing");
  const arm = build.slice(start, end);
  assert.match(arm, /runs-on: windows-11-arm/);
  assert.match(arm, /use-cross: false/);
  assert.match(arm, /can-native-smoke: true/);
  assert.match(arm, /can-mojo-runtime-smoke: true/);
  assert.match(arm, /mojo-release: true/);
  assert.match(arm, /mojo-features: mojo-core/);
  assert.match(build, /key: release-[^\n]*-arm64-native/);
  assert.match(build, /matrix.target != 'aarch64-pc-windows-msvc'/);
  assert.match(build, /if \[ "\$\{\{ matrix.target \}\}" = "aarch64-pc-windows-msvc" \]; then\n\s+cargo build --locked/);
  assert.match(build, /name: Verify native Windows ARM64 artifact architecture/);
  assert.match(build, /\$env:PROCESSOR_ARCHITECTURE -cne 'ARM64'/);
  assert.match(build, /\$machine -ne 0xAA64/);
  assert.match(build, /Native Windows ARM64 version smoke failed/);
  assert.match(build, /name: Verify compiled-in Mojo release/);
  assert.match(build, /"implementation": "mojo-compiled-in"/);
  assert.match(build, /"self_test": "passed"/);
  assert.match(build, /name: Native smoke test/);
  assert.match(build, /name: Upload binary artifact/);
  assert.match(build, /needs:\s*\n\s*- verify-ci\n\s*- build-mojo-archives/);
  assert.equal((build.match(/^          - target: /gm) ?? []).length, 6);
});


test("nightly full Rust suite reuses trusted main CI Mojo archive with strict fallback", () => {
  const nightly = readFileSync(".github/workflows/full-test.yml", "utf8");
  const archive = workflowJob(nightly, "mojo-runtime-linux-archive", "full_test_shards");
  const ciArchive = workflowJob(ci, "mojo-runtime-linux-archive", "mojo-runtime-platform-archives");

  const ciKey = ciArchive.match(/^          key: (mojo-static-v2-.*)$/m)?.[1];
  const nightlyKey = archive.match(/^          key: (mojo-static-v2-.*)$/m)?.[1];
  assert.ok(ciKey, "CI source fingerprint archive key missing");
  assert.equal(nightlyKey, ciKey, "nightly and push CI must share byte-identical immutable cache keys");

  assert.match(archive, /id: mojo-archive-cache/);
  assert.match(archive, /uses: actions\/cache@55cc8345863c7cc4c66a329aec7e433d2d1c52a9/);
  assert.match(archive, /github\.ref == 'refs\/heads\/main'/);
  assert.match(archive, /github\.event_name == 'schedule'/);
  assert.match(archive, /github\.event_name == 'workflow_dispatch'/);
  assert.doesNotMatch(archive, /restore-keys:/);
  assert.equal(
    [...archive.matchAll(/if: \$\{\{ steps\.mojo-archive-cache\.outputs\.cache-hit != 'true' \}\}/g)].length,
    4,
    "pinned Rust, uv, Mojo install and strict compilation must all run when cache misses",
  );
  assert.match(archive, /Install pinned Mojo toolchain/);
  assert.match(archive, /mojo==\$\{MOJO_VERSION\}/);
  assert.match(archive, /PRODEX_MOJO_REQUIRED=1/);
  assert.match(archive, /cargo build --release --locked --target .* -p prodex-mojo-core/);
  assert.match(archive, /--features mojo-core/);
  assert.match(archive, /- name: Validate cached or freshly compiled strict Linux Mojo archive\n\s+shell: bash/);
  assert.doesNotMatch(archive, /- name: Validate cached or freshly compiled strict Linux Mojo archive\n\s+if:/);
  assert.match(archive, /prodex_runtime_profile_provider_order_batch/);
  assert.match(archive, /prodex_mojo_provider_registry_plan_v1/);
  assert.match(archive, /KGEN_CompilerRT_/);
  assert.match(archive, /name: Upload Linux runtime Mojo archive/);
  assert.match(archive, /path: target\/mojo-runtime\/x86_64-unknown-linux-gnu\/libprodex_mojo_core\.a/);

  // The producer and consumer are different runners. Preserve the original
  // download path and failure propagation of all production tests.
  assert.match(nightly, /needs: \[full_test_shards, mojo-runtime-linux-archive\]/);
  assert.match(nightly, /path: \.prodex-ci\/mojo-runtime\/x86_64-unknown-linux-gnu/);
  assert.match(nightly, /--test-threads=1/);
  assert.match(nightly, /Run full test partition/);
  assert.match(nightly, /if \[ "\$\{status\}" -eq 0 \]; then/);
});


test("Windows x64 codegen A/B is opt-in, same official source and cannot publish", () => {
  const workflow = readFileSync(".github/workflows/release-windows-x64-codegen-calibration.yml", "utf8");
  assert.match(workflow, /name: Windows x64 ThinLTO codegen calibration/);
  assert.match(workflow, /workflow_dispatch:/);
  assert.doesNotMatch(workflow, /^  push:/m);
  assert.match(workflow, /permissions:\n  contents: read\n  actions: read/);
  assert.match(workflow, /runs-on: windows-latest/);
  assert.match(workflow, /units: \[16, 64\]/);
  assert.match(workflow, /CARGO_PROFILE_RELEASE_CODEGEN_UNITS: \$\{\{ matrix.units \}\}/);
  assert.match(workflow, /\.head_sha -cne \$env:TARGET_SHA/);
  assert.match(workflow, /\.event -cne 'workflow_dispatch'/);
  assert.match(workflow, /\.head_branch -cne 'main'/);
  assert.match(workflow, /\.path -cne '\.github\/workflows\/standalone-release.yml'/);
  assert.match(workflow, /\.conclusion -cne 'success'/);
  assert.match(workflow, /\.repository.full_name -cne \$env:GITHUB_REPOSITORY/);
  assert.match(workflow, /Check out exact source/);
  assert.match(workflow, /ref: \$\{\{ inputs.target_sha \}\}/);
  assert.match(workflow, /name: mojo-archive-x86_64-pc-windows-msvc/);
  assert.match(workflow, /run-id: \$\{\{ inputs.source_release_run_id \}\}/);
  assert.match(workflow, /PRODEX_MOJO_REQUIRED: "1"/);
  assert.match(workflow, /KGEN_CompilerRT_/);
  assert.match(workflow, /cargo build --locked --release --target x86_64-pc-windows-msvc --features mojo-core/);
  assert.doesNotMatch(workflow, /cargo build[^\n]* --bin prodex/);
  assert.match(workflow, /\$machine -ne 0x8664/);
  assert.match(workflow, /Real native compiled-in Mojo smoke/);
  assert.match(workflow, /"implementation": "mojo-compiled-in"/);
  assert.match(workflow, /"fallback": false/);
  assert.match(workflow, /"self_test": "passed"/);
  assert.match(workflow, /Upload text timings only/);
  assert.doesNotMatch(workflow, /gh release (?:create|upload|edit)/);
  assert.doesNotMatch(workflow, /git push/);
  assert.doesNotMatch(workflow, /subject-path:|attest-build-provenance/);
});


test("Windows fake Kiro ACP timing remains bounded without relaxing production deadlines", () => {
  const production = readFileSync("crates/prodex-app/src/runtime_kiro_acp/process.rs", "utf8");
  const tests = readFileSync("crates/prodex-app/src/runtime_kiro_acp/tests_bootstrap.rs", "utf8");
  const fixture = readFileSync("crates/prodex-app/src/runtime_kiro_acp/tests.rs", "utf8");

  assert.match(production, /RUNTIME_KIRO_ACP_BOOTSTRAP_TIMEOUT: Duration = Duration::from_secs\(5\)/);
  assert.match(tests, /fn kiro_acp_test_start_budget\(\) -> Duration/);
  assert.match(tests, /if cfg!\(windows\) \{\n\s+Duration::from_secs\(15\)/);
  assert.match(tests, /fn kiro_acp_test_response_budget\(\) -> Duration/);
  assert.match(tests, /Duration::from_secs\(10\)/);
  assert.match(tests, /runtime_kiro_acp_bootstrap_with_command_and_timeout\(/);
  assert.match(tests, /kiro_acp_test_start_budget\(\)/);
  assert.match(tests, /started\.elapsed\(\) < kiro_acp_test_response_budget\(\)/);
  assert.match(fixture, /time.sleep\(20 if os.name == "nt" else 5\)/);
  assert.match(tests, /activity-agent-invocations/);
  assert.match(tests, /kiro_acp_bootstrap_times_out_and_terminates_the_agent/);
  assert.match(tests, /kiro_acp_prompt_turn_times_out_and_terminates_the_agent/);
});
