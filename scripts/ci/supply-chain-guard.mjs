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
  "SonarSource/sonarqube-scan-action@22918119ff8e1ca75a623e15c8296b6ea4fbe28f # v8.2.1";
const SONAR_IMAGE =
  "docker.io/library/sonarqube:26.7.0.124771-community@sha256:160bd2f6a3485bd09b655ef22dd63c02bd1fa7ba82aa5d9973fd010b8bcca0b3";
const KICS_IMAGE =
  "docker.io/checkmarx/kics:v2.1.20@sha256:3e5a268eb8adda2e5a483c9359ddfc4cd520ab856a7076dc0b1d8784a37e2602";
const KICS_NON_ACTIONABLE_QUERY_IDS =
  "e84eaf4d-2f45-47b2-abe8-e581b06deb66,8c978947-0ff6-485c-b0c2-0bfca6026466";
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
  const app = workflowJob(contents, "windows-prodex-app");
  if (!workspace || !app) return [".github/workflows/ci.yml: missing Windows test job"];
  const violations = [];
  if (job) {
    violations.push(".github/workflows/ci.yml: windows-security duplicates windows-prodex-app coverage");
  }
  for (const marker of [
    "cargo test --locked -q --workspace --exclude prodex --exclude prodex-app --exclude 'prodex-runtime-*' --exclude 'prodex-storage*' --all-features",
    "cargo test --locked -q -p 'prodex-runtime-*' --all-features",
    "cargo test --locked -q -p 'prodex-storage*' --all-features",
    "- name: Build Windows installer fixture binary",
    "- name: Test Windows installer",
  ]) {
    if (!workspace.includes(marker)) {
      violations.push(`.github/workflows/ci.yml: windows-workspace job missing ${marker}`);
    }
  }
  for (const marker of [
    "runs-on: windows-latest",
    "matrix: ${{ fromJSON(needs.changes.outputs.windows_prodex_app_matrix) }}",
    "PRODEX_APP_FILTERS:",
    "PRODEX_APP_SKIP_FILTERS:",
    "cargo test --locked -q -p prodex-app --lib --all-features",
    "--test-threads=1",
    "save-if: ${{ matrix.save_cache }}",
    "prodex-app filter matched no Windows tests",
  ]) {
    if (!app.includes(marker)) {
      violations.push(`.github/workflows/ci.yml: windows-prodex-app job missing ${marker}`);
    }
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
  const job = workflowJob(workflowContents, "rust-quality");
  const supplyChainJob = workflowJob(workflowContents, "supply-chain");
  if (!job) return [".github/workflows/ci.yml: missing rust-quality job"];
  const violations = [];
  for (const marker of [
    "name: SonarQube Rust quality gate",
    `image: ${SONAR_IMAGE}`,
    'SONAR_ES_BOOTSTRAP_CHECKS_DISABLE: "true"',
    "SONAR_HOST_URL: http://127.0.0.1:9000",
    "- 9000:9000",
    PRODUCTION_CLIPPY_COMMAND,
    "mkdir -p target/sonar",
    "> target/sonar/clippy-report.json",
    "cargo clippy --locked --workspace --all-targets --all-features -- -D warnings",
    "Create ephemeral local Sonar token",
    'local_admin="admin"',
    "base64 --wrap=0",
    '--header "Authorization: Basic ${local_auth}"',
    "/api/user_tokens/generate",
    'echo "::add-mask::${token}"',
    'sonar_token_key="SONAR_TOKEN"',
    `printf '%s=%s\\n' "\${sonar_token_key}" "\${token}"`,
    SONAR_ACTION,
    "-Dsonar.projectKey=prodex-ci",
    "Require zero Sonar issues",
    "/api/qualitygates/project_status",
    '.projectStatus.status == "OK"',
    "/api/issues/search",
    '--data-urlencode "resolved=false"',
    "Sonar unresolved issues:",
    'if [ "${total}" -ne 0 ]',
    "/api/hotspots/search",
    '--data-urlencode "status=TO_REVIEW"',
    "unreviewed security hotspot(s)",
    "/api/measures/component",
    "security_rating,reliability_rating,sqale_rating,duplicated_lines_density",
    ".duplicated_lines_density <= 3",
    "Sonar zero-issue, zero-hotspot, A-rating, and duplication gates: passed.",
    "Revoke ephemeral local Sonar token",
    "/api/user_tokens/revoke",
  ]) {
    if (!job.includes(marker)) {
      violations.push(`.github/workflows/ci.yml: rust-quality job missing ${marker}`);
    }
  }
  if (/^    (?:if|needs):/mu.test(job)) {
    violations.push(".github/workflows/ci.yml: rust-quality must run on every commit");
  }
  if (supplyChainJob?.includes(PRODUCTION_CLIPPY_COMMAND)) {
    violations.push(".github/workflows/ci.yml: production Clippy must run in parallel rust-quality job");
  }
  for (const marker of [
    "secrets.SONAR_TOKEN",
    "vars.SONAR_PROJECT_KEY",
    "vars.SONAR_HOST_URL",
    "vars.SONAR_ORGANIZATION",
  ]) {
    if (job.includes(marker)) {
      violations.push(`.github/workflows/ci.yml: rust-quality must not require ${marker}`);
    }
  }
  if (workflowContents.includes("sonarlint-vscode") || properties.includes("sonarlint-vscode")) {
    violations.push("Sonar Rust gate must not use the unsupported sonarlint-vscode analyzer");
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

export function validateKicsConfiguration(workflowContents) {
  const job = workflowJob(workflowContents, "kics");
  const telemetry = workflowJob(workflowContents, "ci-duration-telemetry");
  if (!job) return [".github/workflows/ci.yml: missing kics job"];
  const violations = [];
  for (const marker of [
    "name: KICS IaC security gate",
    "timeout-minutes: 10",
    `kics_image="${KICS_IMAGE}"`,
    'docker pull "${kics_image}"',
    "docker run --rm",
    '--user "$(id -u):$(id -g)"',
    "--read-only",
    "--cap-drop ALL",
    "--security-opt no-new-privileges:true",
    "--network none",
    "--tmpfs /tmp:rw,noexec,nosuid,size=64m",
    '--volume "${PWD}:/path:ro"',
    '--volume "${PWD}/target/kics:/results"',
    "scan -p /path -o /results",
    "--output-name prodex-kics",
    "--report-formats json,sarif",
    "--disable-secrets",
    "--disable-full-descriptions",
    `--exclude-queries ${KICS_NON_ACTIONABLE_QUERY_IDS}`,
    "--fail-on critical,high,medium,low,info",
    "'.total_counter | select(type == \"number\")'",
    "KICS zero-finding JSON gate: passed.",
    "if: always()",
    "name: kics-iac-results",
    "if-no-files-found: error",
  ]) {
    if (!job.includes(marker)) {
      violations.push(`.github/workflows/ci.yml: kics job missing ${marker}`);
    }
  }
  if (/^    (?:if|needs):/mu.test(job)) {
    violations.push(".github/workflows/ci.yml: kics must run on every commit");
  }
  if (job.includes("continue-on-error") || job.includes("--ignore-on-exit")) {
    violations.push(".github/workflows/ci.yml: kics must fail closed");
  }
  for (const broadExclusion of [
    "--exclude-severities",
    "--exclude-categories",
    "--exclude-paths",
    "--exclude-files",
    "kics-scan ignore",
  ]) {
    if (job.includes(broadExclusion)) {
      violations.push(`.github/workflows/ci.yml: kics must not use broad or inline exclusions: ${broadExclusion}`);
    }
  }
  const queryExclusions = [...job.matchAll(/--exclude-queries\s+([^\s]+)/gu)].map((match) => match[1]);
  if (queryExclusions.length !== 1 || queryExclusions[0] !== KICS_NON_ACTIONABLE_QUERY_IDS) {
    violations.push(".github/workflows/ci.yml: kics query exclusions must remain the exact reviewed INFO-only set");
  }
  if (!telemetry?.includes("- kics")) {
    violations.push(".github/workflows/ci.yml: CI telemetry must wait for kics");
  }
  return violations;
}

const RELEASE_MALWARE_IGNORE_POST = `          ignores_post: |-
            Param([PSCustomObject]$ElementPostMeta)
            Return (
              $ElementPostMeta.Path -ceq 'release-assets/prodex-x86_64-pc-windows-msvc.exe' -and
              $ElementPostMeta.Symbol -ceq 'Win.Trojan.Virut-32' -and
              $ElementPostMeta.Tool -ceq 'clamav'
            )`;
const RELEASE_MALWARE_GATE_ENV = `        env:
          SCAN_OUTCOME: \${{ steps.release_malware_scan.outcome }}
          SCAN_FINISH: \${{ steps.release_malware_scan.outputs.finish }}
          SCAN_FOUND: \${{ steps.release_malware_scan.outputs.found }}`;
const RELEASE_MALWARE_SCAN_STEP = `- name: Scan release assets for malware
        id: release_malware_scan
        continue-on-error: true
        uses: hugoalh/scan-virus-ghaction/clamav@99c81e8991ad1074a14e5f22a21bce9be035e14e # v0.20.1
        with:
          clamav_update: "True"
          found_summary: "True"
          statistics_summary: "True"
          ignores_pre: |-
            Param($ElementPreMeta)
            Return ($ElementPreMeta.Path -notmatch '^release-assets[\\\\/]')
          # ClamAV's Win.Trojan.Virut-32 signature false-positives on the
          # provenance-attested Rust x86_64 Windows binary. Keep this exception
          # exact so every other path, signature, and scanner result fails closed.
          ignores_post: |-
            Param([PSCustomObject]$ElementPostMeta)
            Return (
              $ElementPostMeta.Path -ceq 'release-assets/prodex-x86_64-pc-windows-msvc.exe' -and
              $ElementPostMeta.Symbol -ceq 'Win.Trojan.Virut-32' -and
              $ElementPostMeta.Tool -ceq 'clamav'
            )`;
const RELEASE_MALWARE_GATE_STEP = `- name: Require clean release assets
        env:
          SCAN_OUTCOME: \${{ steps.release_malware_scan.outcome }}
          SCAN_FINISH: \${{ steps.release_malware_scan.outputs.finish }}
          SCAN_FOUND: \${{ steps.release_malware_scan.outputs.found }}
        shell: bash
        run: |
          set -euo pipefail
          if [ "\${SCAN_FINISH}" != "true" ] || [ "\${SCAN_FOUND}" != "false" ]; then
            echo "release asset malware scan did not finish cleanly" >&2
            exit 1
          fi
          if ! find release-assets -maxdepth 1 -type f -print -quit | grep -q .; then
            echo "release asset malware scan scope is empty" >&2
            exit 1
          fi`;

function hasExactReleaseMalwareIgnore(scanStep) {
  const blocks = [...scanStep.matchAll(/^ {10}ignores_post: \|-\n(?:^ {12}.*\n?)*/gmu)];
  return blocks.length === 1 && blocks[0][0].trimEnd() === RELEASE_MALWARE_IGNORE_POST;
}

function hasExactReleaseMalwareGateEnv(requireCleanStep) {
  const blocks = [...requireCleanStep.matchAll(/^ {8}env:\n(?:^ {10}.*\n?)*/gmu)];
  return blocks.length === 1 && blocks[0][0].trimEnd() === RELEASE_MALWARE_GATE_ENV;
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
    "$ElementPostMeta.Path -ceq 'release-assets/prodex-x86_64-pc-windows-msvc.exe'",
    "$ElementPostMeta.Symbol -ceq 'Win.Trojan.Virut-32'",
    "$ElementPostMeta.Tool -ceq 'clamav'",
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
  if ((job.match(/id: release_malware_scan/gu) ?? []).length !== 1) {
    violations.push(
      ".github/workflows/standalone-release.yml: malware gate must use one canonical scan step",
    );
  }
  if (!hasExactReleaseMalwareIgnore(scanStep)) {
    violations.push(
      ".github/workflows/standalone-release.yml: malware false-positive exception must remain exact",
    );
  }
  if (scanStep.trimEnd() !== RELEASE_MALWARE_SCAN_STEP) {
    violations.push(
      ".github/workflows/standalone-release.yml: release asset scan step must remain canonical",
    );
  }
  const requireCleanEnd = requireClean >= 0 ? job.indexOf("\n      - name:", requireClean + 1) : -1;
  const requireCleanStep = requireClean >= 0
    ? job.slice(requireClean, requireCleanEnd >= 0 ? requireCleanEnd : publish)
    : "";
  if (!hasExactReleaseMalwareGateEnv(requireCleanStep)) {
    violations.push(
      ".github/workflows/standalone-release.yml: clean-release gate must consume canonical scan outputs",
    );
  }
  if (requireCleanStep.trimEnd() !== RELEASE_MALWARE_GATE_STEP) {
    violations.push(
      ".github/workflows/standalone-release.yml: clean-release gate step must remain canonical",
    );
  }
  if (requireCleanStep.includes("GITHUB_STEP_SUMMARY")) {
    violations.push(
      ".github/workflows/standalone-release.yml: malware gate must use action outputs, not another step's summary file",
    );
  }
  return violations;
}

export function validateReleaseContainerPublication(contents) {
  const verify = workflowJob(contents, "verify-ci");
  const build = workflowJob(contents, "build");
  const container = workflowJob(contents, "publish-container");
  const release = workflowJob(contents, "publish-github-release");
  if (!verify || !build || !container || !release) {
    return [".github/workflows/standalone-release.yml: missing verify, build, container, or release publish job"];
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
    "persist-credentials: false",
    "cargo install cross --version 0.2.5 --locked",
    "unset GITHUB_TOKEN GH_TOKEN ACTIONS_ID_TOKEN_REQUEST_TOKEN ACTIONS_ID_TOKEN_REQUEST_URL",
    "npm install --global --ignore-scripts @google/gemini-cli@0.53.0",
    "91a21bfa05cd7b58601cb83e0f1f187a9d0084726e5b824d4a4cf60306250908",
    "cd45508981a9baee5fb8f5e38495d315758cd7fea4a715b53a9f26c12544dc95",
    "cde4f1702d3b1695f92b73d26888364e17bca476e17f0fd676484c951d36c125",
    "VERSION=v1.0.77",
    "claude-install.sh\" 2.1.220",
  ]) {
    if (!build.includes(marker)) {
      violations.push(`.github/workflows/standalone-release.yml: release build pin missing ${marker}`);
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
  const exactToolchains = contents.match(/^\s*toolchain:\s*1\.97\.1\s*$/gmu)?.length ?? 0;
  if (rustActions !== exactToolchains) {
    violations.push(`${filePath}: every rust-toolchain action must install exact toolchain 1.97.1`);
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

export function validateDockerfile(contents, rustToolchain = "1.97.1") {
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
  assert.equal(hasExactReleaseMalwareIgnore(RELEASE_MALWARE_IGNORE_POST), true);
  assert.equal(
    hasExactReleaseMalwareIgnore(RELEASE_MALWARE_IGNORE_POST.replace("            )", "              -or $True\n            )")),
    false,
  );
  assert.notEqual(
    RELEASE_MALWARE_SCAN_STEP.replace("Return ($ElementPreMeta.Path", "Return ($True -or $ElementPreMeta.Path"),
    RELEASE_MALWARE_SCAN_STEP,
  );
  assert.notEqual(
    RELEASE_MALWARE_GATE_STEP.replace("set -euo pipefail", "SCAN_FOUND=false\n          set -euo pipefail"),
    RELEASE_MALWARE_GATE_STEP,
  );
  assert.equal(hasExactReleaseMalwareGateEnv(RELEASE_MALWARE_GATE_ENV), true);
  assert.equal(
    hasExactReleaseMalwareGateEnv(
      RELEASE_MALWARE_GATE_ENV.replaceAll("release_malware_scan", "permissive_scan"),
    ),
    false,
  );
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
    validateDockerfile("FROM rust:1.97.1@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef AS builder\n"),
    [],
  );
  assert.equal(validateDockerfile("FROM rust:latest\n").length, 2);
  assert.equal(
    validateDockerfile("FROM rust:1.97.0@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef AS builder\n").length,
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
  const windowsJobs = `jobs:
  windows-workspace:
    steps:
      - run: cargo test --locked -q --workspace --exclude prodex --exclude prodex-app --exclude 'prodex-runtime-*' --exclude 'prodex-storage*' --all-features
      - run: cargo test --locked -q -p 'prodex-runtime-*' --all-features
      - run: cargo test --locked -q -p 'prodex-storage*' --all-features
      - name: Build Windows installer fixture binary
      - name: Test Windows installer
  windows-prodex-app:
    runs-on: windows-latest
    strategy:
      matrix: \${{ fromJSON(needs.changes.outputs.windows_prodex_app_matrix) }}
    steps:
      - uses: Swatinem/rust-cache@0123456789abcdef0123456789abcdef01234567 # v2
        with:
          save-if: \${{ matrix.save_cache }}
      - env:
          PRODEX_APP_FILTERS: filters
          PRODEX_APP_SKIP_FILTERS: skips
        run: cargo test --locked -q -p prodex-app --lib --all-features filter -- --test-threads=1
      - run: echo "prodex-app filter matched no Windows tests"
`;
  assert.deepEqual(validateWindowsSecurityJob(windowsJobs), []);
  assert.equal(
    validateWindowsSecurityJob(windowsJobs.replace("--exclude prodex-app", "--exclude missing-app")).length,
    1,
  );
  assert.equal(
    validateWindowsSecurityJob(`${windowsJobs}  windows-security:\n    runs-on: windows-latest\n`).length,
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
  rust-quality:
    name: SonarQube Rust quality gate
    env:
      SONAR_HOST_URL: http://127.0.0.1:9000
    services:
      sonarqube:
        image: ${SONAR_IMAGE}
        env:
          SONAR_ES_BOOTSTRAP_CHECKS_DISABLE: "true"
        ports:
          - 9000:9000
    steps:
      - run: |
          mkdir -p target/sonar
          ${PRODUCTION_CLIPPY_COMMAND} > target/sonar/clippy-report.json
      - run: cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
      - name: Create ephemeral local Sonar token
        run: |
          local_admin="admin"
          local_auth="$(printf '%s:%s' "\${local_admin}" "\${local_admin}" | base64 --wrap=0)"
          curl --header "Authorization: Basic \${local_auth}" /api/user_tokens/generate
          echo "::add-mask::\${token}"
          sonar_token_key="SONAR_TOKEN"
          printf '%s=%s\\n' "\${sonar_token_key}" "\${token}"
      - uses: ${SONAR_ACTION}
        with:
          args: -Dsonar.projectKey=prodex-ci
      - name: Require zero Sonar issues
        run: |
          curl /api/qualitygates/project_status
          jq '.projectStatus.status == "OK"'
          curl --data-urlencode "resolved=false" /api/issues/search
          if [ "\${total}" -ne 0 ]; then echo "Sonar unresolved issues:"; exit 1; fi
          curl --data-urlencode "status=TO_REVIEW" /api/hotspots/search
          echo "unreviewed security hotspot(s)"
          curl /api/measures/component
          echo "security_rating,reliability_rating,sqale_rating,duplicated_lines_density"
          jq '.duplicated_lines_density <= 3'
          echo "Sonar zero-issue, zero-hotspot, A-rating, and duplication gates: passed."
      - name: Revoke ephemeral local Sonar token
        run: curl /api/user_tokens/revoke
  supply-chain:
    steps:
      - run: cargo audit
  other:
    steps: []
  kics:
    name: KICS IaC security gate
    runs-on: ubuntu-latest
    timeout-minutes: 10
    steps:
      - run: |
          kics_image="${KICS_IMAGE}"
          docker pull "\${kics_image}"
          docker run --rm --user "$(id -u):$(id -g)" --read-only --cap-drop ALL --security-opt no-new-privileges:true --network none --tmpfs /tmp:rw,noexec,nosuid,size=64m --volume "\${PWD}:/path:ro" --volume "\${PWD}/target/kics:/results" "\${kics_image}" scan -p /path -o /results --output-name prodex-kics --report-formats json,sarif --disable-secrets --disable-full-descriptions --exclude-queries ${KICS_NON_ACTIONABLE_QUERY_IDS} --fail-on critical,high,medium,low,info
          kics_total="$(jq -er '.total_counter | select(type == \"number\")' target/kics/prodex-kics.json)"
          test "\${kics_total}" -eq 0
          echo "KICS zero-finding JSON gate: passed."
      - name: Upload KICS reports
        if: always()
        with:
          name: kics-iac-results
          if-no-files-found: error
  ci-duration-telemetry:
    needs:
      - kics
`;
  const sonarProperties = `sonar.sources=src,crates
sonar.inclusions=src/**/*.rs,crates/**/*.rs
sonar.exclusions=${SONAR_EXCLUSIONS.join(",")}
sonar.rust.clippy.enabled=false
sonar.rust.clippyReport.reportPaths=target/sonar/clippy-report.json
sonar.qualitygate.wait=true
`;
  assert.deepEqual(validateSonarConfiguration(sonarWorkflow, sonarProperties), []);
  assert.deepEqual(validateKicsConfiguration(sonarWorkflow), []);
  assert.equal(
    validateSonarConfiguration(
      sonarWorkflow.replace(
        "    name: SonarQube Rust quality gate",
        "    name: SonarQube Rust quality gate\n    needs: changes",
      ),
      sonarProperties,
    ).length,
    1,
  );
  assert.equal(
    validateKicsConfiguration(
      sonarWorkflow.replace(
        "    name: KICS IaC security gate",
        "    name: KICS IaC security gate\n    needs: changes",
      ),
    ).length,
    1,
  );
  assert.equal(validateKicsConfiguration(sonarWorkflow.replace(KICS_IMAGE, "checkmarx/kics:latest")).length, 1);
  assert.equal(validateKicsConfiguration(sonarWorkflow.replace("--fail-on critical,high,medium,low,info", "--fail-on critical,high,medium,low")).length, 1);
  assert.equal(
    validateKicsConfiguration(sonarWorkflow.replace(`--exclude-queries ${KICS_NON_ACTIONABLE_QUERY_IDS}`, "--exclude-queries wrong")).length > 0,
    true,
  );
  assert.equal(
    validateKicsConfiguration(sonarWorkflow.replace(`--exclude-queries ${KICS_NON_ACTIONABLE_QUERY_IDS}`, "--exclude-severities info")).length > 0,
    true,
  );
  assert.equal(
    validateKicsConfiguration(sonarWorkflow.replace("KICS zero-finding JSON gate: passed.", "KICS scan passed.")).length,
    1,
  );
  assert.equal(
    validateSonarConfiguration(sonarWorkflow.replace(SONAR_ACTION, "SonarSource/sonarqube-scan-action@v8.2.1"), sonarProperties).length,
    1,
  );
  assert.equal(
    validateSonarConfiguration(sonarWorkflow.replace(SONAR_IMAGE, "sonarqube:community"), sonarProperties).length,
    1,
  );
  assert.equal(
    validateSonarConfiguration(sonarWorkflow.replace("/api/hotspots/search", "/api/hotspots/ignored"), sonarProperties).length,
    1,
  );
  assert.equal(
    validateSonarConfiguration(sonarWorkflow.replace(".duplicated_lines_density <= 3", ".duplicated_lines_density <= 5"), sonarProperties).length,
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
        "      - name: Create ephemeral local Sonar token",
        "      - run: git clone https://github.com/SonarSource/sonarlint-vscode\n      - name: Create ephemeral local Sonar token",
      ),
      sonarProperties,
    ).length,
    1,
  );
  assert.equal(
    validateSonarConfiguration(
      sonarWorkflow.replace("      SONAR_HOST_URL: http://127.0.0.1:9000", "      SONAR_TOKEN: \${{ secrets.SONAR_TOKEN }}\n      SONAR_HOST_URL: http://127.0.0.1:9000"),
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
      violations.push(
        ...validateWindowsSecurityJob(contents),
        ...validateProcessGuard(contents),
        ...validateKicsConfiguration(contents),
      );
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

  for (const marker of ['channel = "1.97.1"', 'components = ["clippy", "rustfmt"]']) {
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
