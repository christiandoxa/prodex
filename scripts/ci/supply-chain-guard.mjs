#!/usr/bin/env node
import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import {
  openaiCodexDependencySpecifier,
  openaiCodexPlatformDependencySpecifier,
  openaiCodexPlatformPackages,
  openaiCodexVersion,
  platformPackages,
  repoRoot,
} from "../npm/common.mjs";

const ACTION = /^\s*uses:\s*([^\s#]+)(?:\s+#\s*(\S+))?\s*$/gmu;
const CONTAINER = /\b((?:ghcr\.io|quay\.io|docker\.io)\/[a-z0-9._/-]+|anchore\/[a-z0-9._/-]+):([a-z0-9._-]+)(?:@sha256:([a-f0-9]{64}))?/giu;
const SONAR_ACTION =
  "SonarSource/sonarqube-scan-action@7006c4492b2e0ee0f816d36501671557c97f5995 # v8.1.0";
const PRODUCTION_CLIPPY_COMMAND =
  "cargo clippy --locked --workspace --exclude prodex-bench-support --lib --bins --all-features --message-format=json -- -D warnings";
const SONAR_EXCLUSIONS = [
  "**/test/**",
  "**/tests/**",
  "**/benches/**",
  "**/examples/**",
  "**/fuzz/**",
  "**/generated/**",
  "**/vendor/**",
  "**/target/**",
  "**/fixtures/**",
  "**/test_support/**",
  "**/test_support.rs",
  "**/test.rs",
  "**/tests.rs",
  "**/*_tests.rs",
  "**/tests_*.rs",
  "**/*_test/**",
  "**/*_tests/**",
  "crates/prodex-app/src/runtime_config/test_compat.rs",
  "crates/prodex-bench-support/**",
];

function workflowJob(contents, name) {
  const lines = contents.split(/\r?\n/u);
  const start = lines.findIndex((line) => line === `  ${name}:`);
  if (start < 0) return null;
  const end = lines.findIndex((line, index) => index > start && /^  [a-z0-9_-]+:\s*$/iu.test(line));
  return lines.slice(start, end < 0 ? lines.length : end).join("\n");
}

export function validateWindowsSecurityJob(contents) {
  const job = workflowJob(contents, "windows-security");
  const workspace = workflowJob(contents, "windows-workspace");
  if (!job || !workspace) return [".github/workflows/ci.yml: missing Windows test job"];
  const violations = [];
  for (const marker of [
    "runs-on: windows-latest",
    "timeout-minutes: 30",
    'CARGO_INCREMENTAL: "0"',
    'CARGO_PROFILE_DEV_DEBUG: "0"',
    'CARGO_PROFILE_TEST_DEBUG: "0"',
    "toolchain: 1.97.0",
    "uses: Swatinem/rust-cache@",
    "cache-bin: false",
    "save-if: false",
    "cargo test --locked -p prodex-app --all-features --lib 'runtime_broker::registry::store::tests::' -- --test-threads=1",
    "cargo test --locked -q -p prodex-app --lib --all-features 'app_commands::runtime_launch::tests::' -- --test-threads=1 --format pretty",
  ]) {
    if (!job.includes(marker)) {
      violations.push(`.github/workflows/ci.yml: windows-security job missing ${marker}`);
    }
  }
  if (job.includes("continue-on-error: true")) {
    violations.push(".github/workflows/ci.yml: windows-security job must fail closed");
  }
  for (const marker of [
    "--workspace --exclude prodex --exclude prodex-app --all-features",
    "cargo test --locked -q -p prodex-app --all-features -- --test-threads=4",
    "- name: Build Windows installer fixture binary",
    "- name: Test Windows installer",
  ]) {
    if (!workspace.includes(marker)) {
      violations.push(`.github/workflows/ci.yml: windows-workspace job missing ${marker}`);
    }
  }
  if (job.includes("prodex-secret-store") || job.includes("installer:test")) {
    violations.push(".github/workflows/ci.yml: windows-security job duplicates workspace coverage");
  }
  return violations;
}

export function validateProcessGuard(contents) {
  const job = workflowJob(contents, "process-guard");
  if (!job) return [".github/workflows/ci.yml: missing process-guard job"];
  const violations = [];
  if (!job.includes('npm run ci:churn-hygiene:check -- --base "${before}" --head "${after}"')) {
    violations.push(".github/workflows/ci.yml: process-guard must check the complete push range");
  }
  if (job.includes("for commit in") || job.includes("git rev-list --reverse")) {
    violations.push(".github/workflows/ci.yml: process-guard must not split push allowances per commit");
  }
  return violations;
}

export function validateSonarConfiguration(workflowContents, properties) {
  const job = workflowJob(workflowContents, "supply-chain");
  if (!job) return [".github/workflows/ci.yml: missing supply-chain job"];
  const violations = [];
  for (const marker of [
    PRODUCTION_CLIPPY_COMMAND,
    "mkdir -p target/sonar",
    "> target/sonar/clippy-report.json",
    "cargo clippy --locked --workspace --all-targets --all-features -- -D warnings",
    "id: sonar-config",
    "SONAR_TOKEN: ${{ secrets.SONAR_TOKEN }}",
    "SONAR_PROJECT_KEY: ${{ vars.SONAR_PROJECT_KEY }}",
    "SONAR_HOST_URL: ${{ vars.SONAR_HOST_URL }}",
    "SONAR_ORGANIZATION: ${{ vars.SONAR_ORGANIZATION }}",
    "Sonar scan activation boundary",
    "if: ${{ steps.sonar-config.outputs.enabled == 'true' }}",
    SONAR_ACTION,
    "Require zero Sonar issues",
    "/api/qualitygates/project_status",
    "/api/issues/search",
    '--data-urlencode "resolved=false"',
    'if [ "${total}" -ne 0 ]',
  ]) {
    if (!job.includes(marker)) {
      violations.push(`.github/workflows/ci.yml: supply-chain job missing ${marker}`);
    }
  }
  if (workflowContents.includes("sonarlint-vscode") || properties.includes("sonarlint-vscode")) {
    violations.push("Sonar scan must not clone sonarlint-vscode");
  }
  for (const marker of [
    "sonar.sources=src,crates",
    "sonar.inclusions=src/**/*.rs,crates/**/*.rs",
    "sonar.rust.clippy.enabled=false",
    "sonar.rust.clippyReport.reportPaths=target/sonar/clippy-report.json",
    "sonar.qualitygate.wait=true",
  ]) {
    if (!properties.includes(marker)) {
      violations.push(`sonar-project.properties: missing ${marker}`);
    }
  }
  for (const exclusion of SONAR_EXCLUSIONS) {
    if (!properties.includes(exclusion)) {
      violations.push(`sonar-project.properties: missing exclusion ${exclusion}`);
    }
  }
  for (const unsafeExclusion of ["**/*_test.rs", "**/test_*.rs", "**/self_test.rs"]) {
    if (properties.includes(unsafeExclusion)) {
      violations.push(`sonar-project.properties: production self-test excluded by ${unsafeExclusion}`);
    }
  }
  for (const marker of ["sonar.projectKey=", "sonar.organization=", "sonar.host.url=", "SONAR_TOKEN="]) {
    if (properties.includes(marker)) {
      violations.push(`sonar-project.properties: must not contain ${marker}`);
    }
  }
  return violations;
}

export function validateReleaseMalwareGate(contents) {
  const job = workflowJob(contents, "publish-github-release");
  if (!job) return [".github/workflows/standalone-release.yml: missing publish-github-release job"];
  const violations = [];
  const action =
    "hugoalh/scan-virus-ghaction/clamav@99c81e8991ad1074a14e5f22a21bce9be035e14e";
  const prepare = job.indexOf("- name: Prepare release assets");
  const scan = job.indexOf("- name: Scan release assets for malware");
  const requireClean = job.indexOf("- name: Require clean release assets");
  const publish = job.indexOf("- name: Publish GitHub release");
  if (!(prepare >= 0 && prepare < scan && scan < requireClean && requireClean < publish)) {
    violations.push(
      ".github/workflows/standalone-release.yml: malware scan must gate prepared assets before publication",
    );
  }
  for (const marker of [
    "- name: Verify antivirus engine detects EICAR",
    "- name: Require a working antivirus engine",
    `uses: ${action} # v0.20.1`,
    "SCAN_OUTCOME: ${{ steps.antivirus_health.outcome }}",
    "Return ($ElementPreMeta.Path -notmatch '^release-assets[\\\\/]')",
    "SCAN_OUTCOME: ${{ steps.release_malware_scan.outcome }}",
    "SCAN_FINISH: ${{ steps.release_malware_scan.outputs.finish }}",
    "SCAN_FOUND: ${{ steps.release_malware_scan.outputs.found }}",
    "release asset malware scan scope is empty",
  ]) {
    if (!job.includes(marker)) {
      violations.push(`.github/workflows/standalone-release.yml: malware gate missing ${marker}`);
    }
  }
  const scanStep = scan >= 0 && requireClean > scan ? job.slice(scan, requireClean) : "";
  if (scanStep.includes("continue-on-error: true")) {
    violations.push(".github/workflows/standalone-release.yml: release asset scan must fail closed");
  }
  const requireCleanStep = requireClean >= 0 && publish > requireClean ? job.slice(requireClean, publish) : "";
  if (requireCleanStep.includes("GITHUB_STEP_SUMMARY")) {
    violations.push(
      ".github/workflows/standalone-release.yml: malware gate must use action outputs, not another step's summary file",
    );
  }
  return violations;
}

export function validateReleaseContainerPublication(contents) {
  const verify = workflowJob(contents, "verify-ci");
  const container = workflowJob(contents, "publish-container");
  const release = workflowJob(contents, "publish-github-release");
  if (!verify || !container || !release) {
    return [".github/workflows/standalone-release.yml: missing verify, container, or release publish job"];
  }
  const violations = [];
  for (const marker of [
    "git fetch origin --tags --force",
    'release_tag_sha="$(git rev-list -n1 "${version}" 2>/dev/null)"',
    "release tag ${version} targets ${release_tag_sha}, not ${target_sha}",
  ]) {
    if (!verify.includes(marker)) {
      violations.push(`.github/workflows/standalone-release.yml: release target safety missing ${marker}`);
    }
  }
  for (const marker of [
    "- build",
    "packages: write",
    "docker push",
    "docker.io/aquasec/trivy:0.72.0@sha256:",
    "--severity HIGH,CRITICAL --exit-code 1 --format json",
    "prodex-container-vulnerability-${VERSION}.json",
    "subject-digest: ${{ steps.image.outputs.digest }}",
    "push-to-registry: true",
    "sed \"s/PRODEX_IMAGE_DIGEST/",
    "name: kubernetes-manifest",
  ]) {
    if (!container.includes(marker)) {
      violations.push(`.github/workflows/standalone-release.yml: container publication missing ${marker}`);
    }
  }
  for (const marker of [
    "- publish-container",
    "- sync-release-docs",
    "name: kubernetes-manifest",
    "cp artifacts/kubernetes-manifest/prodex-* release-assets/",
  ]) {
    if (!release.includes(marker)) {
      violations.push(`.github/workflows/standalone-release.yml: release publication missing ${marker}`);
    }
  }
  return violations;
}

export function validateWorkflow(filePath, contents) {
  const violations = [];
  let rustActions = 0;
  for (const match of contents.matchAll(ACTION)) {
    const [target, comment] = match.slice(1);
    if (target.startsWith("./")) continue;
    const ref = target.slice(target.lastIndexOf("@") + 1);
    if (!/^[0-9a-f]{40}$/u.test(ref)) {
      violations.push(`${filePath}: third-party action is not pinned to a full commit SHA: ${target}`);
    } else if (!comment || /^[0-9a-f]{40}$/u.test(comment)) {
      violations.push(`${filePath}: pinned action must retain its readable tag comment: ${target}`);
    }
    if (target.startsWith("dtolnay/rust-toolchain@")) rustActions += 1;
  }
  const exactToolchains = contents.match(/^\s*toolchain:\s*1\.97\.0\s*$/gmu)?.length ?? 0;
  if (rustActions !== exactToolchains) {
    violations.push(`${filePath}: every rust-toolchain action must install exact toolchain 1.97.0`);
  }
  for (const match of contents.matchAll(CONTAINER)) {
    if (!match[3]) {
      violations.push(`${filePath}: CI container is not digest-pinned: ${match[0]}`);
    }
  }
  for (const line of contents.split(/\r?\n/u)) {
    if (/\b(?:cargo|cross)\s+(?:bench|build|check|clippy|run|test)\b/u.test(line) && !/--locked\b/u.test(line)) {
      violations.push(`${filePath}: Cargo graph command must use --locked: ${line.trim()}`);
    }
  }
  return violations;
}

export function validateDockerfile(contents, rustToolchain = "1.97.0") {
  const fromLines = contents.split(/\r?\n/u).filter((line) => /^FROM\s+/iu.test(line));
  const violations = fromLines
    .filter((line) => !/^FROM\s+(?:--platform=\S+\s+)?\S+:[^@\s]+@sha256:[0-9a-f]{64}(?:\s+AS\s+\S+)?$/iu.test(line))
    .map((line) => `Dockerfile: base image is not tag-and-digest pinned: ${line}`);
  const builderTag = fromLines
    .find((line) => /\s+AS\s+builder\s*$/iu.test(line))
    ?.match(/^FROM\s+(?:--platform=\S+\s+)?rust:([^@\s]+)@sha256:/iu)?.[1];
  if (builderTag !== rustToolchain && !builderTag?.startsWith(`${rustToolchain}-`)) {
    violations.push(`Dockerfile: Rust builder must match rust-toolchain.toml channel ${rustToolchain}`);
  }
  return violations;
}

export function validateCompose(contents) {
  return contents
    .split(/\r?\n/u)
    .filter((line) => /^\s*image:\s*/u.test(line) && !/:local\s*$/u.test(line))
    .filter((line) => !/^\s*image:\s*\S+:[^@\s]+@sha256:[0-9a-f]{64}\s*$/iu.test(line))
    .map((line) => `compose.yaml: service image is not tag-and-digest pinned: ${line.trim()}`);
}

export function validateCodexPins(workspaceManifest, manifest, installer, windowsInstaller, shim, lockfile) {
  const violations = [];
  if (manifest.dependencies?.["@openai/codex"] !== openaiCodexDependencySpecifier) {
    violations.push(`npm/prodex/package.json: @openai/codex must equal ${openaiCodexVersion}`);
  }
  for (const spec of openaiCodexPlatformPackages) {
    if (
      manifest.optionalDependencies?.[spec.packageName] !==
      openaiCodexPlatformDependencySpecifier(spec)
    ) {
      violations.push(`npm/prodex/package.json: ${spec.packageName} is not exact-version pinned`);
    }
  }
  for (const spec of platformPackages) {
    const directory = spec.packageName.replace("@christiandoxa/prodex-", "");
    if (
      workspaceManifest.optionalDependencies?.[spec.packageName] !==
      `file:npm/platforms/${directory}`
    ) {
      violations.push(`package.json: ${spec.packageName} must be a local optional lock input`);
    }
  }
  if (/@openai\/codex@latest\b/u.test(`${installer}\n${windowsInstaller}\n${shim}`)) {
    violations.push("Codex install paths must not use @openai/codex@latest");
  }
  if (!installer.includes(`CODEX_NPM_VERSION="${openaiCodexVersion}"`)) {
    violations.push("install.sh: Codex migration version is not synchronized");
  }
  if (!windowsInstaller.includes(`$CodexNpmVersion = "${openaiCodexVersion}"`)) {
    violations.push("install.ps1: Codex migration version is not synchronized");
  }
  if (!windowsInstaller.includes('"@openai/codex@$CodexNpmVersion"')) {
    violations.push("install.ps1: Codex migration must use the synchronized version");
  }
  if (!shim.includes('require("./codex-compat.cjs")')) {
    violations.push("npm/prodex/lib/codex-shim.cjs: missing canonical compatibility metadata");
  }
  if (
    lockfile.packages?.["npm/prodex"]?.dependencies?.["@openai/codex"] !==
    openaiCodexDependencySpecifier
  ) {
    violations.push("package-lock.json: Prodex Codex dependency is not exact-version locked");
  }
  return violations;
}

function selfTest() {
  assert.deepEqual(
    validateWorkflow("safe.yml", "uses: owner/action@0123456789abcdef0123456789abcdef01234567 # v1\n"),
    [],
  );
  assert.equal(validateWorkflow("bad.yml", "uses: owner/action@v1\n").length, 1);
  assert.equal(
    validateWorkflow("bad.yml", "run: docker run ghcr.io/example/tool:v1 scan\n").length,
    1,
  );
  assert.equal(validateWorkflow("bad.yml", "run: cargo test --workspace\n").length, 1);
  assert.deepEqual(
    validateDockerfile("FROM rust:1.97.0@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef AS builder\n"),
    [],
  );
  assert.equal(validateDockerfile("FROM rust:latest\n").length, 2);
  assert.equal(
    validateDockerfile("FROM rust:1.97.1@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef AS builder\n").length,
    1,
  );
  assert.deepEqual(
    validateCompose("services:\n  db:\n    image: postgres:16@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\n"),
    [],
  );
  assert.equal(validateCompose("services:\n  db:\n    image: postgres:16\n").length, 1);
  const codexManifest = {
    dependencies: { "@openai/codex": openaiCodexDependencySpecifier },
    optionalDependencies: Object.fromEntries(
      openaiCodexPlatformPackages.map((spec) => [
        spec.packageName,
        openaiCodexPlatformDependencySpecifier(spec),
      ]),
    ),
  };
  const codexLock = {
    packages: {
      "npm/prodex": { dependencies: { "@openai/codex": openaiCodexDependencySpecifier } },
    },
  };
  const workspaceManifest = {
    optionalDependencies: Object.fromEntries(
      platformPackages.map((spec) => [
        spec.packageName,
        `file:npm/platforms/${spec.packageName.replace("@christiandoxa/prodex-", "")}`,
      ]),
    ),
  };
  assert.deepEqual(
    validateCodexPins(
      workspaceManifest,
      codexManifest,
      `CODEX_NPM_VERSION="${openaiCodexVersion}"`,
      `$CodexNpmVersion = "${openaiCodexVersion}"\n"@openai/codex@$CodexNpmVersion"`,
      'require("./codex-compat.cjs")',
      codexLock,
    ),
    [],
  );
  assert.equal(
    validateCodexPins(
      workspaceManifest,
      { ...codexManifest, dependencies: { "@openai/codex": "latest" } },
      "npm install -g @openai/codex@latest",
      "npm install -g @openai/codex@latest",
      "",
      { packages: {} },
    ).length > 0,
    true,
  );
  const windowsJob = `jobs:
  windows-security:
    runs-on: windows-latest
    timeout-minutes: 30
    env:
      CARGO_INCREMENTAL: "0"
      CARGO_PROFILE_DEV_DEBUG: "0"
      CARGO_PROFILE_TEST_DEBUG: "0"
    steps:
      - uses: dtolnay/rust-toolchain@0123456789abcdef0123456789abcdef01234567 # stable
        with:
          toolchain: 1.97.0
      - uses: Swatinem/rust-cache@0123456789abcdef0123456789abcdef01234567 # v2
        with:
          cache-bin: false
          save-if: false
      - run: cargo test --locked -p prodex-app --all-features --lib 'runtime_broker::registry::store::tests::' -- --test-threads=1
      - run: cargo test --locked -q -p prodex-app --lib --all-features 'app_commands::runtime_launch::tests::' -- --test-threads=1 --format pretty
  windows-workspace:
    steps:
      - run: cargo test --locked -q --workspace --exclude prodex --exclude prodex-app --all-features
      - run: cargo test --locked -q -p prodex-app --all-features -- --test-threads=4
      - name: Build Windows installer fixture binary
      - name: Test Windows installer
`;
  assert.deepEqual(validateWindowsSecurityJob(windowsJob), []);
  assert.equal(
    validateWindowsSecurityJob(windowsJob.replace("--exclude prodex-app", "--exclude missing-app")).length,
    1,
  );
  assert.equal(validateWindowsSecurityJob("jobs:\n  fmt:\n    runs-on: ubuntu-latest\n").length, 1);
  const processJob = `jobs:
  process-guard:
    runs-on: ubuntu-latest
    steps:
      - run: |
          before="before"
          after="after"
          npm run ci:churn-hygiene:check -- --base "\${before}" --head "\${after}"
`;
  assert.deepEqual(validateProcessGuard(processJob), []);
  assert.equal(validateProcessGuard(`${processJob}          for commit in commits; do :; done\n`).length, 1);
  const sonarWorkflow = `jobs:
  supply-chain:
    steps:
      - run: |
          mkdir -p target/sonar
          ${PRODUCTION_CLIPPY_COMMAND} > target/sonar/clippy-report.json
      - run: cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
      - id: sonar-config
        env:
          SONAR_TOKEN: \${{ secrets.SONAR_TOKEN }}
          SONAR_PROJECT_KEY: \${{ vars.SONAR_PROJECT_KEY }}
          SONAR_HOST_URL: \${{ vars.SONAR_HOST_URL }}
          SONAR_ORGANIZATION: \${{ vars.SONAR_ORGANIZATION }}
        run: echo "Sonar scan activation boundary"
      - if: \${{ steps.sonar-config.outputs.enabled == 'true' }}
        uses: ${SONAR_ACTION}
      - name: Require zero Sonar issues
        if: \${{ steps.sonar-config.outputs.enabled == 'true' }}
        run: |
          curl /api/qualitygates/project_status
          curl --data-urlencode "resolved=false" /api/issues/search
          if [ "\${total}" -ne 0 ]; then exit 1; fi
  other:
    steps: []
`;
  const sonarProperties = `sonar.sources=src,crates
sonar.inclusions=src/**/*.rs,crates/**/*.rs
sonar.exclusions=${SONAR_EXCLUSIONS.join(",")}
sonar.rust.clippy.enabled=false
sonar.rust.clippyReport.reportPaths=target/sonar/clippy-report.json
sonar.qualitygate.wait=true
`;
  assert.deepEqual(validateSonarConfiguration(sonarWorkflow, sonarProperties), []);
  assert.equal(
    validateSonarConfiguration(sonarWorkflow.replace(SONAR_ACTION, "SonarSource/sonarqube-scan-action@v8.1.0"), sonarProperties).length,
    1,
  );
  assert.equal(
    validateSonarConfiguration(sonarWorkflow.replace(PRODUCTION_CLIPPY_COMMAND, "cargo clippy --locked --workspace --all-targets --all-features --message-format=json -- -D warnings"), sonarProperties).length,
    1,
  );
  assert.equal(
    validateSonarConfiguration(sonarWorkflow, sonarProperties.replace("**/vendor/**,", "")).length,
    1,
  );
  assert.equal(
    validateSonarConfiguration(
      sonarWorkflow,
      sonarProperties.replace("**/*_tests.rs", "**/*_test.rs"),
    ).length,
    2,
  );
  assert.equal(
    validateSonarConfiguration(
      sonarWorkflow,
      sonarProperties.replace("sonar.rust.clippyReport.reportPaths=target/sonar/clippy-report.json", ""),
    ).length,
    1,
  );
  assert.equal(
    validateSonarConfiguration(
      sonarWorkflow,
      sonarProperties.replace("sonar.rust.clippy.enabled=false", "sonar.rust.clippy.enable=false"),
    ).length,
    1,
  );
  assert.equal(
    validateSonarConfiguration(sonarWorkflow, sonarProperties.replace("sonar.qualitygate.wait=true", "sonar.qualitygate.wait=false")).length,
    1,
  );
  assert.equal(
    validateSonarConfiguration(
      sonarWorkflow.replace(
        '        run: echo "Sonar scan activation boundary"',
        '        run: echo "Sonar scan activation boundary"\n      - run: git clone https://github.com/SonarSource/sonarlint-vscode',
      ),
      sonarProperties,
    ).length,
    1,
  );
  assert.equal(validateReleaseMalwareGate("jobs:\n  other:\n").length, 1);
  assert.equal(validateReleaseContainerPublication("jobs:\n  other:\n").length, 1);
}

async function main() {
  if (process.argv.includes("--self-test")) selfTest();
  const workflowDir = path.join(repoRoot, ".github", "workflows");
  const violations = [];
  for (const fileName of (await fs.readdir(workflowDir)).filter((name) => /\.ya?ml$/u.test(name)).sort()) {
    const filePath = `.github/workflows/${fileName}`;
    const contents = await fs.readFile(path.join(workflowDir, fileName), "utf8");
    violations.push(...validateWorkflow(filePath, contents));
    if (fileName === "ci.yml") {
      violations.push(...validateWindowsSecurityJob(contents), ...validateProcessGuard(contents));
    }
    if (fileName === "standalone-release.yml") {
      violations.push(
        ...validateReleaseMalwareGate(contents),
        ...validateReleaseContainerPublication(contents),
      );
    }
  }
  const sonarProperties = await fs.readFile(path.join(repoRoot, "sonar-project.properties"), "utf8");
  const ciContents = await fs.readFile(path.join(repoRoot, ".github/workflows/ci.yml"), "utf8");
  violations.push(...validateSonarConfiguration(ciContents, sonarProperties));
  const toolchain = await fs.readFile(path.join(repoRoot, "rust-toolchain.toml"), "utf8");
  const rustToolchain = toolchain.match(/^channel\s*=\s*"([^"]+)"/mu)?.[1];
  if (!rustToolchain) {
    violations.push("rust-toolchain.toml: missing toolchain channel");
  } else {
    violations.push(
      ...validateDockerfile(await fs.readFile(path.join(repoRoot, "Dockerfile"), "utf8"), rustToolchain),
    );
  }
  violations.push(...validateCompose(await fs.readFile(path.join(repoRoot, "compose.yaml"), "utf8")));

  for (const marker of ['channel = "1.97.0"', 'components = ["clippy", "rustfmt"]']) {
    if (!toolchain.includes(marker)) violations.push(`rust-toolchain.toml: missing ${marker}`);
  }
  await fs.access(path.join(repoRoot, "Cargo.lock"));
  const npmManifest = JSON.parse(
    await fs.readFile(path.join(repoRoot, "npm/prodex/package.json"), "utf8"),
  );
  const npmWorkspaceManifest = JSON.parse(
    await fs.readFile(path.join(repoRoot, "package.json"), "utf8"),
  );
  const npmLock = JSON.parse(await fs.readFile(path.join(repoRoot, "package-lock.json"), "utf8"));
  violations.push(
    ...validateCodexPins(
      npmWorkspaceManifest,
      npmManifest,
      await fs.readFile(path.join(repoRoot, "install.sh"), "utf8"),
      await fs.readFile(path.join(repoRoot, "install.ps1"), "utf8"),
      await fs.readFile(path.join(repoRoot, "npm/prodex/lib/codex-shim.cjs"), "utf8"),
      npmLock,
    ),
  );

  if (violations.length === 0) {
    process.stdout.write("supply-chain guard: ok\n");
    return;
  }
  process.stderr.write(`supply-chain guard failed:\n  - ${violations.join("\n  - ")}\n`);
  process.exitCode = 1;
}

main().catch((error) => {
  process.stderr.write(`supply-chain-guard: ${error.message}\n`);
  process.exitCode = 1;
});
