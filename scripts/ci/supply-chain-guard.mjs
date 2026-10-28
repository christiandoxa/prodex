#!/usr/bin/env node
import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import {
  platformPackages,
  repoRoot,
} from "../npm/common.mjs";
import { validateCodexPackageLock } from "./codex-purity-guard.mjs";

const ACTION = /^\s*uses:\s*([^\s#]+)(?:\s+#\s*(\S+))?\s*$/gmu;
const CONTAINER = /\b((?:ghcr\.io|quay\.io|docker\.io)\/[a-z0-9._/-]+|anchore\/[a-z0-9._/-]+):([a-z0-9._-]+)(?:@sha256:([a-f0-9]{64}))?/giu;
const SONAR_ACTION =
  "SonarSource/sonarqube-scan-action@d209202bc7d53ff1cc128f7f907dac145c9d6ae9 # v8.3.0";
const SONAR_IMAGE =
  "docker.io/library/sonarqube:26.7.0.124771-community@sha256:160bd2f6a3485bd09b655ef22dd63c02bd1fa7ba82aa5d9973fd010b8bcca0b3";
const KICS_IMAGE =
  "docker.io/checkmarx/kics:v2.1.20@sha256:3e5a268eb8adda2e5a483c9359ddfc4cd520ab856a7076dc0b1d8784a37e2602";
const KICS_NON_ACTIONABLE_QUERY_IDS =
  "e84eaf4d-2f45-47b2-abe8-e581b06deb66,8c978947-0ff6-485c-b0c2-0bfca6026466";
const CLIPPY_QUALITY_COMMAND =
  "cargo clippy --locked --workspace --all-targets --message-format=json -- -D warnings";
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

function workflowStep(job, name) {
  const start = job.indexOf(`      - name: ${name}\n`);
  if (start < 0) return "";
  const next = job.indexOf("\n      - name:", start + 1);
  return job.slice(start, next < 0 ? job.length : next);
}

export function validateWindowsSecurityJob(contents) {
  const job = workflowJob(contents, "windows-security");
  const workspace = workflowJob(contents, "windows-workspace");
  const app = workflowJob(contents, "windows-prodex-app");
  const producer = workflowJob(contents, "windows-app-test-build");
  if (!workspace || !app || !producer) return [".github/workflows/ci.yml: missing Windows test job"];
  const violations = [];
  if (job) {
    violations.push(".github/workflows/ci.yml: windows-security duplicates windows-prodex-app coverage");
  }
  for (const marker of [
    "cargo test --locked -q --workspace --exclude prodex --exclude prodex-app --exclude 'prodex-runtime-*' -- --test-threads=4",
    "cargo test --locked -q -p 'prodex-runtime-*' -- --test-threads=4",
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
    "needs: [changes, windows-app-test-build]",
    "TEST_MANIFEST_SHA256: ${{ needs.windows-app-test-build.outputs.manifest_sha256 }}",
    "PRODEX_APP_SHARD_INDEX: ${{ matrix.subshard_index }}",
    "PRODEX_APP_SHARD_COUNT: ${{ matrix.subshard_count }}",
    "node scripts/ci/native-test-artifact.mjs run",
    "name: windows-app-tests",
  ]) {
    if (!app.includes(marker)) {
      violations.push(`.github/workflows/ci.yml: windows-prodex-app job missing ${marker}`);
    }
  }
  for (const marker of [
    "needs: [changes, mojo-runtime-platform-archives]",
    "node scripts/ci/native-test-artifact.mjs pack",
    "manifest_sha256: ${{ steps.pack.outputs.manifest_sha256 }}",
    "name: windows-app-tests", "if-no-files-found: error", "overwrite: true",
  ]) {
    if (!producer.includes(marker)) violations.push(`.github/workflows/ci.yml: Windows test producer missing ${marker}`);
  }
  if (/cargo test|rust-cache@|sccache-action@/.test(app)) {
    violations.push(".github/workflows/ci.yml: Windows shards must not rebuild the shared test executable");
  }
  violations.push(...validateWindowsIntegrationTests(contents));
  return violations;
}

export function validateWindowsIntegrationTests(contents) {
  const producer = workflowJob(contents, "windows-integration-build") ?? "";
  const consumer = workflowJob(contents, "windows-integration") ?? "";
  const violations = [];
  for (const [job, markers] of [
    [producer, ["needs: [changes, mojo-runtime-platform-archives]", "runs-on: windows-latest",
      "integration-test-artifact.mjs pack", "manifest_sha256: ${{ steps.pack.outputs.manifest_sha256 }}",
      "name: windows-integration-tests", "if-no-files-found: error", "overwrite: true"]],
    [consumer, ["needs: [changes, windows-integration-build]", "runs-on: windows-latest",
      "PRODEX_INTEGRATION_BUNDLE: .prodex-ci/windows-integration-tests",
      "INTEGRATION_MANIFEST_SHA256: ${{ needs.windows-integration-build.outputs.manifest_sha256 }}",
      "name: windows-integration-tests", "auto-rotate-shards.mjs --jobs 4 --shard-index",
      "--shard-count 2", "integration-test-artifact.mjs restore", "node --test scripts/install.test.mjs"]],
  ]) {
    for (const marker of markers) if (!job.includes(marker)) violations.push(`Windows integration binding missing ${marker}`);
  }
  if (/cargo (?:test|build)|rust-cache@|sccache-action@/.test(consumer)) {
    violations.push("Windows integration consumers must not recompile the verified CLI or harness");
  }
  return violations;
}

export function validateLinuxNativeTests(contents) {
  const producer = workflowJob(contents, "linux-app-test-build");
  const consumer = workflowJob(contents, "prodex-app-lib");
  if (!producer || !consumer) return ["CI requires the Linux native bundle producer and consumers"];
  const violations = [];
  for (const [job, markers] of [
    [producer, ["needs: [changes, mojo-runtime-linux-archive]", "runs-on: ubuntu-latest",
      "PRODEX_MOJO_ARCHIVE:", "native-test-artifact.mjs pack", "name: linux-app-tests",
      "manifest_sha256: ${{ steps.pack.outputs.manifest_sha256 }}", "if-no-files-found: error"]],
    [consumer, ["needs: [changes, linux-app-test-build]", "runs-on: ubuntu-latest",
      "fromJSON(needs.changes.outputs.prodex_app_matrix)", "PRODEX_APP_FILTERS:", "PRODEX_APP_SKIP_FILTERS:",
      "TEST_MANIFEST_SHA256: ${{ needs.linux-app-test-build.outputs.manifest_sha256 }}",
      "name: linux-app-tests", "native-test-artifact.mjs run"]],
  ]) {
    for (const marker of markers) {
      if (!job.includes(marker)) violations.push(`CI Linux native-test binding missing ${marker}`);
    }
  }
  for (const name of ["profile-commands-internal", "main-internal-core", "main-internal-runtime-proxy", "runtime-stress"]) {
    const internal = workflowJob(contents, name) ?? "";
    for (const marker of ["needs: [changes, linux-app-test-build]", "name: linux-app-tests",
      "PRODEX_TEST_BUNDLE: .prodex-ci/linux-app-tests",
      "TEST_MANIFEST_SHA256: ${{ needs.linux-app-test-build.outputs.manifest_sha256 }}"]) {
      if (!internal.includes(marker)) violations.push(`CI ${name} missing verified native bundle binding: ${marker}`);
    }
    if (/cargo test|rust-cache@|sccache-action@/.test(internal)) {
      violations.push(`CI ${name} must not rebuild the shared test executable`);
    }
  }
  return violations;
}

export function validateProcessGuard(contents) {
  const job = workflowJob(contents, "process-guard");
  if (!job) return [".github/workflows/ci.yml: missing process-guard job"];
  const violations = [];
  if (!job.includes('node scripts/ci/churn-hygiene.mjs --check --base "${before}" --head "${after}"')) {
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
    CLIPPY_QUALITY_COMMAND,
    "mkdir -p target/sonar",
    "> target/sonar/clippy-report.json",
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
  const qualityNeeds = job.match(/^    needs:\s*([a-z0-9_-]+)\s*$/mu)?.[1] ?? null;
  if (/^    if:/mu.test(job) ||
      (qualityNeeds !== null && qualityNeeds !== "mojo-runtime-linux-archive")) {
    violations.push(".github/workflows/ci.yml: rust-quality must run on every commit");
  }
  if (qualityNeeds === "mojo-runtime-linux-archive") {
    const archiveJob = workflowJob(workflowContents, "mojo-runtime-linux-archive");
    const changesJob = workflowJob(workflowContents, "changes");
    const archiveNeeds = archiveJob?.match(/^    needs:\s*([a-z0-9_-]+)\s*$/mu)?.[1] ?? null;
    if (!archiveJob ||
        /^    if:/mu.test(archiveJob) ||
        archiveNeeds !== "changes" ||
        !changesJob ||
        /^    if:/mu.test(changesJob)) {
      violations.push(
        ".github/workflows/ci.yml: rust-quality Mojo archive dependency must remain unconditional",
      );
    }
  }
  if (supplyChainJob?.includes(CLIPPY_QUALITY_COMMAND)) {
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
  return violations;
}

export function validateReleaseMalwareGate(contents) {
  const job = workflowJob(contents, "validate-release-assets");
  const publisher = workflowJob(contents, "publish-github-release");
  const health = workflowJob(contents, "antivirus-health");
  if (!job || !publisher || !health) return [".github/workflows/standalone-release.yml: missing release validation or publication job"];
  const violations = [];
  const action =
    "hugoalh/scan-virus-ghaction/clamav@99c81e8991ad1074a14e5f22a21bce9be035e14e";
  const prepare = job.indexOf("- name: Prepare release assets");
  const scan = job.indexOf("- name: Scan release assets for malware");
  const requireClean = job.indexOf("- name: Require clean release assets");
  const publish = job.indexOf("- name: Upload validated release assets for the gated publisher");
  if (!(prepare >= 0 && prepare < scan && scan < requireClean && requireClean < publish)) {
    violations.push(
      ".github/workflows/standalone-release.yml: malware scan must gate prepared assets before publication",
    );
  }
  for (const marker of [
    `uses: ${action} # v0.20.1`,
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
  for (const marker of [
    "needs: release-preflight", "- name: Verify antivirus engine detects EICAR",
    "- name: Require a working antivirus engine", `uses: ${action} # v0.20.1`,
    "SCAN_OUTCOME: ${{ steps.antivirus_health.outcome }}",
    "SCAN_FINISH: ${{ steps.antivirus_health.outputs.finish }}",
    "SCAN_FOUND: ${{ steps.antivirus_health.outputs.found }}",
    '"${SCAN_OUTCOME}" != "failure"', '"${SCAN_FINISH}" != "true"', '"${SCAN_FOUND}" != "true"',
  ]) {
    if (!health.includes(marker)) violations.push(`.github/workflows/standalone-release.yml: engine proof missing ${marker}`);
  }
  if (!job.includes("      - antivirus-health")) violations.push(".github/workflows/standalone-release.yml: asset scan must wait for engine proof");
  const digest = "806278ed1a2829743c4b9d92f29bdd49e384ab3cdc6bf19caea19b1390d4c085";
  if (!contents.includes(`CLAMAV_IMAGE: ghcr.io/hugoalh/scan-virus-ghaction:0.20.1-clamav@sha256:${digest}`)) {
    violations.push(".github/workflows/standalone-release.yml: antivirus image digest must stay pinned");
  }
  for (const body of [health, job]) {
    for (const marker of ["Verify pinned antivirus image", "RepoDigests", "clamav_update: \"True\"", "${CLAMAV_IMAGE##*@}"]) {
      if (!body.includes(marker)) violations.push(`.github/workflows/standalone-release.yml: scanner identity/freshness missing ${marker}`);
    }
    if (/contents: write|attestations: write|id-token: write/.test(body)) {
      violations.push(".github/workflows/standalone-release.yml: antivirus checks must be read-only");
    }
  }
  for (const marker of ["- validate-release-assets", "name: validated-release-assets", "sha256sum --check SHA256SUMS"]) {
    if (!publisher.includes(marker)) {
      violations.push(`.github/workflows/standalone-release.yml: publisher must consume immutable scanned assets: ${marker}`);
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

const releaseCiPollingMarkers = [
  "gh run list --workflow ci.yml --event push",
  'successful = [run for run in runs if run.get("status") == "completed" and run.get("conclusion") == "success"]',
  "if successful:",
  "run = successful[0]",
  'pending = [run for run in runs if run.get("status") != "completed"]',
  "if pending:",
  "run = pending[0]",
];

export function validateReleaseCiPolling(contents) {
  const verify = workflowJob(contents, "verify-ci");
  if (!verify) return [".github/workflows/standalone-release.yml: missing verify-ci job"];
  return releaseCiPollingMarkers
    .filter((marker) => !verify.includes(marker))
    .map((marker) => `.github/workflows/standalone-release.yml: CI polling missing ${marker}`);
}

function validateReleaseSbom(validation) {
  const violations = [];
  const sbom = workflowStep(validation, "Generate release SBOM");
  const required = [
    'find artifacts -mindepth 2 -maxdepth 2 -type f',
    'codex-purity-guard.mjs sbom-input',
    'anchore/syft:v1.41.2@sha256:962c5626d30cdfb814802598cbb7b63cb690bf979a4bc0ff1d9c6e0cb441db7f',
    'sbom-input:/source:ro', 'dir:/source -o spdx-json > release-sbom.spdx.json',
    'codex-purity-guard.mjs release-sbom.spdx.json',
  ];
  for (const marker of required) {
    if (!sbom.includes(marker)) violations.push(`release asset validation missing SBOM proof: ${marker}`);
  }
  if (/^        (?:if:|continue-on-error:)/mu.test(sbom)) {
    violations.push("release SBOM must execute unconditionally and fail closed");
  }
  const steps = ["Download built binaries", "Generate release SBOM", "Prepare release assets",
    "Generate and verify release checksums", "Scan release assets for malware"];
  const positions = steps.map((name) => validation.indexOf(`      - name: ${name}\n`));
  if (positions.some((index, i) => index < 0 || (i > 0 && positions[i - 1] >= index))) {
    violations.push("release SBOM must use downloaded binaries before checksums and malware scan");
  }
  if (!workflowStep(validation, "Prepare release assets").includes(
    "cp release-sbom.spdx.json release-assets/release-sbom.spdx.json")) {
    violations.push("generated SBOM must enter the final scanned asset bundle");
  }
  return violations;
}

export function validateReleaseContainerPublication(contents, targetWorkflow = "") {
  const preflight = workflowJob(contents, "release-preflight");
  const validation = workflowJob(contents, "validate-release-assets");
  const verify = workflowJob(contents, "verify-ci");
  const caller = workflowJob(contents, "build");
  const build = workflowJob(targetWorkflow, "build");
  const archive = workflowJob(targetWorkflow, "build-mojo-archives");
  const attestBinaries = workflowJob(contents, "attest-binaries");
  const syncDocs = workflowJob(contents, "sync-release-docs");
  const release = workflowJob(contents, "publish-github-release");
  if (!preflight || !validation || !verify || !caller || !archive || !build || !attestBinaries || !syncDocs || !release) {
    return [".github/workflows/standalone-release.yml: missing required release job"];
  }
  const violations = [];
  violations.push(...validateReleaseCiPolling(contents), ...validateReleaseSbom(validation));
  for (const marker of [
    "if: ${{ always() && !cancelled() }}", "      - release-preflight", "      - build",
    "      - validate-release-assets", "      - antivirus-health",
    "- name: Require all artifact gates to succeed", "RESULTS: ${{ toJSON(needs) }}",
    'keys == ["antivirus-health", "build", "release-preflight", "validate-release-assets"] and all(.[]; .result == "success")',
    "- name: Record release verification verdict",
  ]) {
    if (!verify.includes(marker)) violations.push(`release verdict missing ${marker}`);
  }
  const prerequisite = workflowStep(verify, "Require all artifact gates to succeed");
  const ciWait = workflowStep(verify, "Wait for CI success");
  for (const step of [prerequisite, ciWait]) {
    if (!step || /\n\s+(?:if|continue-on-error):/u.test(step)) {
      violations.push("release verdict prerequisite and CI checks must execute and fail closed");
    }
  }
  if (verify.indexOf(prerequisite) >= verify.indexOf("      - name: Check out") ||
      verify.indexOf(ciWait) >= verify.indexOf("      - name: Record release verification verdict") ||
      /continue-on-error:/.test(verify.split("    steps:")[0])) {
    violations.push("release verdict ordering or failure policy is unsafe");
  }

  for (const marker of [
    "needs: release-preflight",
    "uses: ./.github/workflows/release-target.yml",
    "target_sha: ${{ needs.release-preflight.outputs.target_sha }}",
    "version: ${{ needs.release-preflight.outputs.version }}",
    "kiro_cli_version: ${{ needs.release-preflight.outputs.kiro_cli_version }}",
    "platforms: ${{ format('[{0}]', toJSON(matrix)) }}",
  ]) {
    if (!caller.includes(marker)) violations.push(`.github/workflows/standalone-release.yml: target invocation missing ${marker}`);
  }
  if (!build.includes("needs: build-mojo-archives") ||
      !build.includes("include: ${{ fromJSON(inputs.platforms) }}") ||
      !archive.includes("include: ${{ fromJSON(inputs.platforms) }}")) {
    violations.push(".github/workflows/release-target.yml: binaries must depend on their exact target archive");
  }
  if (!targetWorkflow.includes("workflow_call:") || /workflow_dispatch:|gh release|git push/.test(targetWorkflow) ||
      /^\s+(?:artifact-metadata|attestations|id-token|packages|contents):\s*write\s*$/mu.test(targetWorkflow + caller)) {
    violations.push(".github/workflows/release-target.yml: target preparation must remain local, called and read-only");
  }

  const download = workflowStep(build, "Download official Linux artifact");
  const smoke = workflowStep(build, "Run official artifact smoke");
  const linuxCondition = "if: ${{ matrix.target == 'x86_64-unknown-linux-gnu' }}";
  for (const step of [download, smoke]) {
    if (!step.includes(linuxCondition) || /continue-on-error: true|if:.*always\(/u.test(step)) {
      violations.push("release-target.yml: native artifact smoke must run and fail closed on the Linux target");
    }
  }
  for (const marker of ["name: x86_64-unknown-linux-gnu", "path: artifact", "uses: actions/download-artifact@"]) {
    if (!download.includes(marker)) violations.push(`release-target.yml: native artifact download missing ${marker}`);
  }
  for (const marker of [
    "RELEASE_VERSION: ${{ inputs.version }}", "node scripts/ci/release-artifact-smoke.mjs",
    '--binary "${binary}"', '--version "${RELEASE_VERSION}"',
  ]) {
    if (!smoke.includes(marker)) violations.push(`release-target.yml: native artifact smoke missing ${marker}`);
  }
  if (!build.includes("persist-credentials: false") ||
      build.indexOf("      - name: Upload binary artifact\n") >= build.indexOf(download) ||
      build.indexOf(download) >= build.indexOf(smoke) ||
      /continue-on-error: true/u.test(build.split("    steps:")[0])) {
    violations.push("release-target.yml: downloaded smoke must gate the successful owning binary build");
  }
  for (const name of ["validate-release-assets", "verify-ci"]) {
    if (!workflowJob(contents, name)?.includes("      - build\n")) {
      violations.push(`standalone-release.yml: ${name} must include all target builds and native smoke`);
    }
  }

  const workflowHeader = contents.slice(0, contents.indexOf("\njobs:"));
  if (/^\s*(?:artifact-metadata|attestations|id-token):\s*write\s*$/mu.test(workflowHeader)) {
    violations.push(
      ".github/workflows/standalone-release.yml: privileged permissions must be scoped to owning jobs",
    );
  }
  if (/^\s+(?:artifact-metadata|attestations|id-token|packages|contents):\s*write\s*$/mu.test(build)) {
    violations.push(
      ".github/workflows/standalone-release.yml: release build must not hold write or OIDC permissions",
    );
  }
  if (attestBinaries.includes("actions/checkout@")) {
    violations.push(
      ".github/workflows/standalone-release.yml: binary attestation job must not execute checked-out code",
    );
  }
  for (const [name, job, timeout] of [
    ["build", build, 180],
    ["attest-binaries", attestBinaries, 15],
    ["validate-release-assets", validation, 35],
    ["sync-release-docs", syncDocs, 15],
    ["publish-github-release", release, 360],
  ]) {
    if (!job.includes(`timeout-minutes: ${timeout}`)) {
      violations.push(`.github/workflows/standalone-release.yml: ${name} timeout must be ${timeout} minutes`);
    }
  }
  for (const marker of [
    "- validate-release-assets",
    "- verify-ci",
    "artifact-metadata: write",
    "attestations: write",
    "contents: read",
    "id-token: write",
    "actions/download-artifact@",
    "actions/attest-build-provenance@",
  ]) {
    if (!attestBinaries.includes(marker)) {
      violations.push(`.github/workflows/standalone-release.yml: binary attestation missing ${marker}`);
    }
  }
  for (const marker of ["target_sha:", "version:", "publish_at:", "required: true"]) {
    if (!workflowHeader.includes(marker)) {
      violations.push(`.github/workflows/standalone-release.yml: release dispatch input missing ${marker}`);
    }
  }
  for (const [name, job] of [
    ["release-preflight", preflight],
    ["verify-ci", verify],
    ["validate-release-assets", validation],
    ["build-mojo-archives", archive],
    ["build", build],
    ["sync-release-docs", syncDocs],
    ["publish-github-release", release],
  ]) {
    const checkoutCount = job.split("uses: actions/checkout@").length - 1;
    const protectedCheckoutCount = job.split("persist-credentials: false").length - 1;
    if (checkoutCount === 0 || checkoutCount !== protectedCheckoutCount) {
      violations.push(
        `.github/workflows/standalone-release.yml: ${name} checkouts must not persist credentials`,
      );
    }
  }
  for (const marker of ["inputs.publish && 'standalone-release'", "cancel-in-progress: ${{ !inputs.publish }}"]) {
    if (!contents.includes(marker)) {
      violations.push(`.github/workflows/standalone-release.yml: release concurrency missing ${marker}`);
    }
  }
  if (/\n\s+push:\s*\n\s+tags:/u.test(contents)) {
    violations.push(".github/workflows/standalone-release.yml: release tags must be workflow-created");
  }
  for (const marker of [
    'if [ "${ref_type}" != "branch" ] || [ "${ref_name}" != "main" ]',
    "RELEASE_TARGET_SHA: ${{ inputs.target_sha }}",
    "RELEASE_VERSION: ${{ inputs.version }}",
    '[[ ! "${target_sha}" =~ ^[0-9a-f]{40}$ ]]',
    '[ "${GITHUB_SHA}" != "${target_sha}" ]',
    '[ "${version}" != "${RELEASE_VERSION}" ]',
    "git fetch origin --tags --force",
    'release_tag_sha="$(git rev-list -n1 "${version}" 2>/dev/null)"',
    "release tag ${version} targets ${release_tag_sha}, not ${target_sha}",
    'node scripts/ci/release-version-order.mjs --version "${version}" --head "${target_sha}"',
  ]) {
    if (!preflight.includes(marker)) {
      violations.push(`.github/workflows/standalone-release.yml: release target safety missing ${marker}`);
    }
  }
  for (const marker of [
    "persist-credentials: false",
    "cargo install cross --version 0.2.5 --locked",
    "RUSTC_WRAPPER=sccache cargo build --locked",
    "unset GITHUB_TOKEN GH_TOKEN ACTIONS_ID_TOKEN_REQUEST_TOKEN ACTIONS_ID_TOKEN_REQUEST_URL",
    "npm install --global --ignore-scripts @google/gemini-cli@0.53.0",
    "d8dd659c11c156002942f09443b6cb7904959004d4e7f1f73777fdbb2d909b54",
    "00efcc2e196f0793d097935d6230062fc66e8247dc01334d2ac3eae4e26a1c7f",
    "3a68d3406cf674e17bed1733a4dcf37805e2e47d87417700007d7e1aa766a944",
    "VERSION=v1.0.77",
    "claude-install.sh\" 2.1.220",
  ]) {
    if (!build.includes(marker)) {
      violations.push(`.github/workflows/standalone-release.yml: release build pin missing ${marker}`);
    }
  }
  const nativeClientRetryFlags = "--retry 5 --retry-all-errors --retry-delay 2";
  if (`${preflight}\n${build}`.split(nativeClientRetryFlags).length - 1 !== 4) {
    violations.push(
      ".github/workflows/standalone-release.yml: every native client download must retry transient failures",
    );
  }
  for (const [name, job] of [["attest-binaries", attestBinaries], ["sync-release-docs", syncDocs], ["publish-github-release", release]]) {
    for (const marker of ["if: ${{ inputs.publish }}", "- verify-ci", "- validate-release-assets"]) {
      if (!job.includes(marker)) violations.push(`.github/workflows/standalone-release.yml: ${name} missing publication gate ${marker}`);
    }
  }
  for (const [name, job] of [["release-preflight", preflight], ["verify-ci", verify], ["validate-release-assets", validation]]) {
    if (/^\s+(?:artifact-metadata|attestations|id-token|packages|contents):\s*write\s*$/mu.test(job)) {
      violations.push(`.github/workflows/standalone-release.yml: ${name} must be read-only`);
    }
  }
  if (!attestBinaries.includes("subject-path: release-assets/*")) {
    violations.push(".github/workflows/standalone-release.yml: attestation must cover every scanned release asset");
  }
  if (!workflowHeader.includes("default: false\n        type: boolean") || !release.includes('test "${RELEASE_PUBLISH}" = true')) {
    violations.push(".github/workflows/standalone-release.yml: publication must be explicit and independently checked");
  }
  for (const marker of [
    "- sync-release-docs",
    "- attest-binaries",
    '--source-digest "${TARGET_SHA}"',
    "--source-ref refs/heads/main",
    "--deny-self-hosted-runners",
    "find release-assets -maxdepth 1 -type f -print0",
    'TARGET_SHA: $' + '{{ steps.release.outputs.target_sha }}',
    'gh release create "${tag}" release-assets/*',
    '--target "${TARGET_SHA}"',
    "--checkpoint P --release-sha",
    "refusing to overwrite it",
  ]) {
    if (!release.includes(marker)) {
      violations.push(`.github/workflows/standalone-release.yml: release publication missing ${marker}`);
    }
  }
  if (release.includes('git tag "${version}"') || release.includes('push origin "refs/tags/')) {
    violations.push(
      ".github/workflows/standalone-release.yml: release tags must be created atomically by the GitHub Releases API",
    );
  }
  if (release.includes("--clobber") || release.includes("gh release edit")) {
    violations.push(
      ".github/workflows/standalone-release.yml: existing public releases must not be overwritten",
    );
  }
  return violations;
}

export function validateWorkflow(filePath, contents) {
  const violations = [];
  for (const step of contents.split(/\n      - name:/u)) {
    if (step.includes("uses: Swatinem/rust-cache@") &&
        /^          shared-key:/mu.test(step) && /^          key:/mu.test(step)) {
      violations.push(`${filePath}: Rust cache key is ignored when shared-key is present; put discriminators in shared-key`);
    }
  }
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
  const exactToolchains = contents.match(/^\s*toolchain:\s*1\.99\.1\s*$/gmu)?.length ?? 0;
  if (rustActions !== exactToolchains) {
    violations.push(`${filePath}: every rust-toolchain action must install exact toolchain 1.99.1`);
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

export function validateDockerfile(contents, rustToolchain = "1.99.1") {
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

export function validateProdexPackaging(
  workspaceManifest,
  manifest,
  installer,
  windowsInstaller,
  npmLauncher,
) {
  const violations = [];
  for (const dependencies of [manifest.dependencies, manifest.optionalDependencies]) {
    for (const [dependencyName, specifier] of Object.entries(dependencies ?? {})) {
      if (
        dependencyName === "@openai/codex" ||
        dependencyName.startsWith("@openai/codex-") ||
        String(specifier).includes("@openai/codex")
      ) {
        violations.push(`npm/prodex/package.json: Codex dependency is not allowed (${dependencyName})`);
      }
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
  if (/codex/iu.test(`${installer}\n${windowsInstaller}\n${npmLauncher}`)) {
    violations.push("Prodex installers and npm launcher must not contain Codex packaging logic");
  }
  if (manifest.files?.includes("lib")) {
    violations.push("npm/prodex/package.json: release package must not include a runtime shim directory");
  }
  return violations;
}

export function validateReleaseCodexExclusion(contents, targetWorkflow = contents) {
  const violations = [];
  for (const marker of [
    "Build patched Codex runtime",
    "codex-*",
    "codex_binary",
    "release-assets/codex",
  ]) {
    if ((contents + targetWorkflow).includes(marker)) {
      violations.push(`.github/workflows/standalone-release.yml: release contains forbidden Codex packaging marker ${marker}`);
    }
  }
  if (!contents.includes("find release-assets -maxdepth 1 -type f")) {
    violations.push(".github/workflows/standalone-release.yml: release asset scope must be explicit");
  }
  const artifactSmoke = workflowStep(workflowJob(targetWorkflow, "build") ?? "", "Run official artifact smoke");
  const release = workflowJob(contents, "validate-release-assets") ?? "";
  for (const [surface, job, marker] of [
    ["artifact and installer", artifactSmoke, "codex-purity-guard.mjs package-lock.json install.sh install.ps1 artifact"],
    ["staged npm package", artifactSmoke, "npm-package-smoke.mjs --binary-dir artifact"],
    ["SBOM input", release, "codex-purity-guard.mjs sbom-input"],
    ["SBOM output", release, "codex-purity-guard.mjs release-sbom.spdx.json"],
    ["GitHub release assets", release, "codex-purity-guard.mjs release-assets"],
  ]) {
    if (!job.includes(marker)) {
      violations.push(`.github/workflows/standalone-release.yml: ${surface} lacks the Codex purity guard`);
    }
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
    validateDockerfile("FROM rust:1.99.1@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef AS builder\n"),
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
  const prodexManifest = {
    files: ["prodex", "README.md", "LICENSE"],
    optionalDependencies: Object.fromEntries(
      platformPackages.map((spec) => [spec.packageName, "0.2.0"]),
    ),
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
    validateProdexPackaging(
      workspaceManifest,
      prodexManifest,
      "standalone installer",
      "standalone installer",
      "prodex npm launcher",
    ),
    [],
  );
  assert.equal(
    validateProdexPackaging(
      workspaceManifest,
      { ...prodexManifest, dependencies: { "@openai/codex": "latest" } },
      "standalone installer",
      "standalone installer",
      "",
    ).length > 0,
    true,
  );
  assert.equal(
    validateCodexPackageLock({ packages: { "node_modules/@openai/codex": { name: "@openai/codex" } } }).length > 0,
    true,
  );
  const releasePurityFixture = `
  build:
    steps:
      - name: Run official artifact smoke
        run: node scripts/ci/codex-purity-guard.mjs package-lock.json install.sh install.ps1 artifact
        npm: node scripts/ci/npm-package-smoke.mjs --binary-dir artifact
  validate-release-assets:
    input: node scripts/ci/codex-purity-guard.mjs sbom-input
    output: node scripts/ci/codex-purity-guard.mjs release-sbom.spdx.json
    scope: find release-assets -maxdepth 1 -type f
    run: node scripts/ci/codex-purity-guard.mjs release-assets
`;
  assert.deepEqual(validateReleaseCodexExclusion(releasePurityFixture), []);
  assert.equal(
    validateReleaseCodexExclusion(releasePurityFixture.replace("codex-purity-guard.mjs sbom-input", "missing"))
      .length,
    1,
  );
  assert.equal(validateReleaseCodexExclusion("codex-asset").length > 0, true);
  const windowsJobs = `jobs:
  windows-workspace:
    steps:
      - run: cargo test --locked -q --workspace --exclude prodex --exclude prodex-app --exclude 'prodex-runtime-*' -- --test-threads=4
      - run: cargo test --locked -q -p 'prodex-runtime-*' -- --test-threads=4
      - name: Build Windows installer fixture binary
      - name: Test Windows installer
  windows-app-test-build:
    needs: [changes, mojo-runtime-platform-archives]
    outputs:
      manifest_sha256: \${{ steps.pack.outputs.manifest_sha256 }}
    steps:
      - run: node scripts/ci/native-test-artifact.mjs pack bundle
      - name: windows-app-tests
        if-no-files-found: error
        overwrite: true
  windows-prodex-app:
    needs: [changes, windows-app-test-build]
    runs-on: windows-latest
    strategy:
      matrix: \${{ fromJSON(needs.changes.outputs.windows_prodex_app_matrix) }}
    steps:
      - name: windows-app-tests
      - env:
          PRODEX_APP_FILTERS: filters
          PRODEX_APP_SKIP_FILTERS: skips
          TEST_MANIFEST_SHA256: \${{ needs.windows-app-test-build.outputs.manifest_sha256 }}
          PRODEX_APP_SHARD_INDEX: \${{ matrix.subshard_index }}
          PRODEX_APP_SHARD_COUNT: \${{ matrix.subshard_count }}
        run: node scripts/ci/native-test-artifact.mjs run bundle
  windows-integration-build:
    needs: [changes, mojo-runtime-platform-archives]
    runs-on: windows-latest
    manifest_sha256: \${{ steps.pack.outputs.manifest_sha256 }}
    run: node scripts/ci/integration-test-artifact.mjs pack bundle
    name: windows-integration-tests
    if-no-files-found: error
    overwrite: true
  windows-integration:
    needs: [changes, windows-integration-build]
    runs-on: windows-latest
    PRODEX_INTEGRATION_BUNDLE: .prodex-ci/windows-integration-tests
    INTEGRATION_MANIFEST_SHA256: \${{ needs.windows-integration-build.outputs.manifest_sha256 }}
    name: windows-integration-tests
    run: node scripts/ci/auto-rotate-shards.mjs --jobs 4 --shard-index 0 --shard-count 2
    restore: node scripts/ci/integration-test-artifact.mjs restore bundle
    test: node --test scripts/install.test.mjs
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
          node scripts/ci/churn-hygiene.mjs --check --base "\${before}" --head "\${after}"
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
          ${CLIPPY_QUALITY_COMMAND} > target/sonar/clippy-report.json
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
    validateSonarConfiguration(sonarWorkflow.replace(SONAR_ACTION, "SonarSource/sonarqube-scan-action@v8.3.0"), sonarProperties).length,
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
    validateSonarConfiguration(sonarWorkflow.replace(CLIPPY_QUALITY_COMMAND, "cargo clippy --locked --workspace --all-targets --all-features --message-format=json -- -D warnings"), sonarProperties).length,
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
  const releaseCiPollingFixture = `jobs:
  verify-ci:
    steps:
      - run: |
          gh run list --workflow ci.yml --event push
          successful = [run for run in runs if run.get("status") == "completed" and run.get("conclusion") == "success"]
          if successful:
              run = successful[0]
          pending = [run for run in runs if run.get("status") != "completed"]
          if pending:
              run = pending[0]
`;
  assert.deepEqual(validateReleaseCiPolling(releaseCiPollingFixture), []);
  assert.equal(
    validateReleaseCiPolling(releaseCiPollingFixture.replace(" --event push", "")).length,
    1,
  );
  assert.equal(
    validateReleaseCiPolling(releaseCiPollingFixture.replace("run = pending[0]", "run = runs[0]")).length,
    1,
  );
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
        ...validateLinuxNativeTests(contents),
        ...validateProcessGuard(contents),
        ...validateKicsConfiguration(contents),
      );
    }
    if (fileName === "standalone-release.yml") {
      violations.push(
        ...validateReleaseMalwareGate(contents),
        ...validateReleaseContainerPublication(contents, await fs.readFile(path.join(workflowDir, "release-target.yml"), "utf8")),
        ...validateReleaseCodexExclusion(contents, await fs.readFile(path.join(workflowDir, "release-target.yml"), "utf8")),
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

  for (const marker of ['channel = "1.99.1"', 'components = ["clippy", "rustfmt"]']) {
    if (!toolchain.includes(marker)) violations.push(`rust-toolchain.toml: missing ${marker}`);
  }
  await fs.access(path.join(repoRoot, "Cargo.lock"));
  const npmManifest = JSON.parse(
    await fs.readFile(path.join(repoRoot, "npm/prodex/package.json"), "utf8"),
  );
  const npmWorkspaceManifest = JSON.parse(
    await fs.readFile(path.join(repoRoot, "package.json"), "utf8"),
  );
  const npmLock = JSON.parse(
    await fs.readFile(path.join(repoRoot, "package-lock.json"), "utf8"),
  );
  violations.push(
    ...validateCodexPackageLock(npmLock),
    ...validateProdexPackaging(
      npmWorkspaceManifest,
      npmManifest,
      await fs.readFile(path.join(repoRoot, "install.sh"), "utf8"),
      await fs.readFile(path.join(repoRoot, "install.ps1"), "utf8"),
      await fs.readFile(path.join(repoRoot, "npm/prodex/prodex"), "utf8"),
    ),
  );

  if (violations.length === 0) {
    process.stdout.write("supply-chain guard: ok\n");
    return;
  }
  process.stderr.write(`supply-chain guard failed:\n  - ${violations.join("\n  - ")}\n`);
  process.exitCode = 1;
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? "").href) {
  main().catch((error) => {
    process.stderr.write(`supply-chain-guard: ${error.message}\n`);
    process.exitCode = 1;
  });
}
