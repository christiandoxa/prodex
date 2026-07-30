#!/usr/bin/env node
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..", "..");

const DOCUMENTS = [
  {
    path: "docs/threat-model.md",
    required: [
      "# Prodex Enterprise Threat Model",
      "## Trust Boundaries",
      "## Threats and Controls",
      "## Required Negative Tests",
      "Row-Level Security",
      "break-glass",
      "Redis must not store",
      "OIDC discovery and JWKS network fetches must not happen on the request path",
      "no mid-stream rotation",
      "audit events",
      "Root/admin token used for inference",
      "Process-local request or call IDs collide",
      "Read-modify-write budget accounting",
      "DDL during request handling",
      "Redis whole-map JSON state",
      "Blocking I/O, unbounded workers, or mutex-held I/O on request paths",
      "Dependency inversion toward domain/application ports",
      "propagate bounded end-to-end trace context",
      "last-known-good",
    ],
  },
  {
    path: "docs/enterprise-governance/09-storage-ha-backup-and-dr.md",
    required: [
      "# Storage, High Availability, Backup, and Disaster Recovery",
      "## Migration Rules",
      "PostgreSQL",
      "forced RLS",
      "Redis",
      "expand, bounded backfill",
    ],
  },
  {
    path: "docs/enterprise-governance/22-rollout-rollback-and-deprecation.md",
    required: [
      "# Rollout, Rollback, and Deprecation",
      "## Promotion Gates",
      "## Schema and State Compatibility",
      "## Final Cutover and Exit Criteria",
      "external migrations",
      "expand -> bounded backfill",
    ],
  },
];

const REQUIRED_ENTERPRISE_ARTIFACT_PATHS = [
  ...[
    "06-provider-registry-and-routing.md",
    "09-storage-ha-backup-and-dr.md",
    "15-classification-contract-and-enforcement.md",
    "16-response-stream-enforcement.md",
    "17-policy-authority-and-revision-store.md",
    "18-audit-siem-and-evidence.md",
    "19-unified-gateway-and-identity.md",
    "20-operations-slos-and-alerts.md",
    "21-testing-performance-and-evidence.md",
    "22-rollout-rollback-and-deprecation.md",
    "implementation-ledger.md",
    "test-matrix.json",
  ].map((name) => `docs/enterprise-governance/${name}`),
  ...[
    "0001-classification-and-inspection.md",
    "0002-pdp-pap-pip-pep-snapshots.md",
    "0003-policy-approval-activation-lkg.md",
    "0004-execution-approval.md",
    "0005-provider-registry-routing.md",
    "0006-continuation-pinning-revocation.md",
    "0007-mandatory-audit-siem-outbox.md",
    "0008-session-trusted-proxy.md",
    "0009-external-secret-vault.md",
    "0010-bank-profile-fail-closed.md",
    "0011-sqlite-runtime-boundary.md",
  ].map((name) => `docs/enterprise-governance/adrs/${name}`),
  ...[
    "01-approved-cloud-public-internal.json",
    "02-confidential-region-retention.json",
    "03-restricted-local-only.json",
    "04-disable-tools-high-risk.json",
    "05-high-risk-execution-approval.json",
    "06-compliant-provider-outage-fallback.json",
    "07-bank-mode-fail-closed.json",
  ].map((name) => `docs/enterprise-governance/samples/${name}`),
];
const TEST_MATRIX_PATH = "docs/enterprise-governance/test-matrix.json";
const TEST_MATRIX_STATUSES = new Set([
  "tested",
  "implemented",
  "pending_validation",
  "partial",
  "planned",
]);
const GOVERNANCE_LIFECYCLE_OPENAPI_PATH =
  "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_openapi.json";
const GOVERNANCE_SECURITY_EVIDENCE_TESTS = [
  {
    matrixId: "SEC-POL-003",
    testName: "gateway_policy_http_revocation_invalidates_cache_and_lkg",
    sourcePath:
      "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_tests/gateway_admin_policy_lifecycle/policy.rs",
    requiredText: '"revoke"',
  },
  {
    matrixId: "SEC-POL-003",
    testName: "governance_invalidation_notification_is_delivered_only_after_commit",
    sourcePath: "crates/prodex-storage-postgres/tests/postgres_migration.rs",
  },
  ...[
    "invalidation_payload_is_bounded_and_strict",
    "unknown_tenant_notification_cannot_enroll_authority",
    "notification_reloads_latest_snapshot_and_wakes_recovery_poll",
  ].map((testName) => ({
    matrixId: "SEC-POL-003",
    testName,
    sourcePath:
      "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite/governance_invalidation.rs",
  })),
];
const GOVERNANCE_REVOCATION_TEST = GOVERNANCE_SECURITY_EVIDENCE_TESTS[0];
const GOVERNANCE_LIFECYCLE_FAMILIES = [
  "policies",
  "classification-rules",
  "provider-registries",
  "routing-scores",
];
const GOVERNANCE_LIFECYCLE_ACTIONS = ["activate", "rollback", "revoke"];

const WORKFLOW_PATH = ".github/workflows/ci.yml";
const PACKAGE_JSON_PATH = "package.json";
const TEST_IMPACT_MANIFEST_PATH = "scripts/ci/test-impact-manifest.json";
const REQUIRED_ENTERPRISE_WORKFLOW_COMMANDS = [
  "npm run ci:enterprise-docs-guard",
  "npm run ci:enterprise-id-boundary-guard",
  "npm run ci:enterprise-binaries-guard",
  "npm run ci:application-boundary-guard",
  "npm run ci:auth-boundary-guard",
  "npm run ci:config-boundary-guard",
  "npm run ci:control-plane-boundary-guard",
  "npm run ci:observability-boundary-guard",
  "npm run ci:provider-spi-boundary-guard",
  "npm run ci:storage-boundary-guard",
  "npm run ci:backup-restore-drill",
  "npm run ci:storage-postgres-boundary-guard",
  "npm run ci:storage-redis-boundary-guard",
  "npm run ci:storage-sqlite-boundary-guard",
  "npm run ci:gateway-core-boundary-guard",
  "npm run ci:gateway-http-boundary-guard",
  "npm run ci:deployment-security-guard",
];
const REQUIRED_ENTERPRISE_NPM_SCRIPTS = REQUIRED_ENTERPRISE_WORKFLOW_COMMANDS.map((command) =>
  command.replace(/^npm run /u, ""),
);
const FORBIDDEN_ENTERPRISE_DOC_PHRASES = [
  {
    path: "docs/runtime-policy.md",
    phrase: "prodex-42",
    reason: "call id examples must not imply process-local numeric ids",
  },
];

function validateDocument(document) {
  const filePath = path.join(repoRoot, document.path);
  const errors = [];
  if (!fs.existsSync(filePath)) {
    return [`${document.path}: required enterprise document is missing`];
  }
  const content = fs.readFileSync(filePath, "utf8");
  for (const required of document.required) {
    if (!content.includes(required)) {
      errors.push(`${document.path}: missing required enterprise documentation phrase '${required}'`);
    }
  }
  return errors;
}

function validateRequiredArtifacts(root = repoRoot, exists = fs.existsSync) {
  return REQUIRED_ENTERPRISE_ARTIFACT_PATHS.filter(
    (relativePath) => !exists(path.join(root, relativePath)),
  ).map((relativePath) => `${relativePath}: required enterprise artifact is missing`);
}

function validateTestMatrix(content, matrixPath = TEST_MATRIX_PATH) {
  let parsed;
  try {
    parsed = JSON.parse(content);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return [`${matrixPath}: invalid JSON: ${message}`];
  }

  const errors = [];
  if (!Number.isInteger(parsed?.schema_version) || parsed.schema_version < 1) {
    errors.push(`${matrixPath}: schema_version must be a positive integer`);
  }
  if (!Array.isArray(parsed?.tests) || parsed.tests.length === 0) {
    errors.push(`${matrixPath}: tests must be a non-empty array`);
    return errors;
  }
  const ids = new Set();
  parsed.tests.forEach((test, index) => {
    if (typeof test?.id !== "string" || test.id.trim() === "") {
      errors.push(`${matrixPath}: tests[${index}].id must be a non-empty string`);
    } else if (ids.has(test.id)) {
      errors.push(`${matrixPath}: tests[${index}].id '${test.id}' is duplicated`);
    } else {
      ids.add(test.id);
    }
    if (!TEST_MATRIX_STATUSES.has(test?.implementation_status)) {
      errors.push(`${matrixPath}: tests[${index}].implementation_status is invalid`);
    }
    if (
      !Array.isArray(test?.evidence) ||
      test.evidence.length === 0 ||
      test.evidence.some((item) => typeof item !== "string" || item.trim() === "")
    ) {
      errors.push(`${matrixPath}: tests[${index}].evidence must contain non-empty strings`);
    } else if (
      ["tested", "implemented"].includes(test.implementation_status) &&
      test.evidence.some((item) => /\b(?:pending|planned|todo)\b/iu.test(item))
    ) {
      errors.push(
        `${matrixPath}: tests[${index}].evidence contradicts implementation_status '${test.implementation_status}'`,
      );
    }
  });
  return errors;
}

function sourceTestBlock(source, testName) {
  const escapedName = testName.replace(/[.*+?^${}()|[\]\\]/gu, "\\$&");
  const match = new RegExp(
    `#\\[(?:[\\w:]+::)?test(?:\\([^\\]]*\\))?\\]\\s*(?:#\\[[^\\]]+\\]\\s*)*(?:async\\s+)?fn\\s+${escapedName}\\s*\\(`,
    "u",
  ).exec(source);
  if (!match) return null;
  const start = match.index;
  const nextTestOffset = source
    .slice(start + match[0].length)
    .search(/\n#\[(?:[\w:]+::)?test(?:\([^\]]*\))?\]/u);
  return source.slice(
    start,
    nextTestOffset < 0 ? source.length : start + match[0].length + nextTestOffset,
  );
}

function validateGovernanceLifecycleEvidence(
  matrixContent,
  openapiContent,
  evidenceSources,
  matrixPath = TEST_MATRIX_PATH,
  openapiPath = GOVERNANCE_LIFECYCLE_OPENAPI_PATH,
) {
  let matrix;
  let openapi;
  try {
    matrix = JSON.parse(matrixContent);
  } catch {
    return [];
  }
  try {
    openapi = JSON.parse(openapiContent);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return [`${openapiPath}: invalid JSON: ${message}`];
  }

  const errors = [];
  const row = matrix?.tests?.find((test) => test?.id === GOVERNANCE_REVOCATION_TEST.matrixId);
  if (!row) {
    errors.push(`${matrixPath}: missing required evidence row '${GOVERNANCE_REVOCATION_TEST.matrixId}'`);
  } else if (["tested", "implemented"].includes(row.implementation_status)) {
    for (const evidenceTest of GOVERNANCE_SECURITY_EVIDENCE_TESTS) {
      if (!row.evidence?.includes(evidenceTest.testName)) {
        errors.push(
          `${matrixPath}: ${evidenceTest.matrixId} must cite exact repository test '${evidenceTest.testName}'`,
        );
      }
      const source = evidenceSources[evidenceTest.sourcePath];
      const testBlock = source === undefined
        ? null
        : sourceTestBlock(source, evidenceTest.testName);
      if (testBlock === null) {
        errors.push(
          `${evidenceTest.sourcePath}: missing evidence test '${evidenceTest.testName}'`,
        );
      }
      if (
        testBlock !== null &&
        evidenceTest.requiredText !== undefined &&
        !testBlock.includes(evidenceTest.requiredText)
      ) {
        errors.push(
          `${evidenceTest.sourcePath}: evidence test '${evidenceTest.testName}' must exercise ${evidenceTest.requiredText}`,
        );
      }
    }
  }

  const paths = openapi?.paths ?? {};
  for (const family of GOVERNANCE_LIFECYCLE_FAMILIES) {
    for (const action of GOVERNANCE_LIFECYCLE_ACTIONS) {
      const route = `/v1/prodex/gateway/${family}/{revision_id}/${action}`;
      if (!Object.hasOwn(paths, route)) {
        errors.push(`${openapiPath}: missing documented governance lifecycle route '${route}'`);
      }
    }
  }
  return errors;
}

function validateForbiddenEnterpriseDocPhrases() {
  const errors = [];
  for (const forbidden of FORBIDDEN_ENTERPRISE_DOC_PHRASES) {
    const filePath = path.join(repoRoot, forbidden.path);
    if (!fs.existsSync(filePath)) continue;
    const content = fs.readFileSync(filePath, "utf8");
    if (content.includes(forbidden.phrase)) {
      errors.push(`${forbidden.path}: forbidden phrase '${forbidden.phrase}': ${forbidden.reason}`);
    }
  }
  return errors;
}

function validateEnterpriseWorkflow(workflowText, workflowPath = WORKFLOW_PATH) {
  const errors = [];
  if (!workflowText.includes("Enforce enterprise boundary guards")) {
    errors.push(`${workflowPath}: missing enterprise boundary guard workflow step`);
  }
  for (const command of REQUIRED_ENTERPRISE_WORKFLOW_COMMANDS) {
    if (!workflowText.includes(command)) {
      errors.push(`${workflowPath}: missing enterprise guard command '${command}'`);
    }
  }
  return errors;
}

function validateEnterprisePackageScripts(packageJsonText, packageJsonPath = PACKAGE_JSON_PATH) {
  const errors = [];
  let parsed;
  try {
    parsed = JSON.parse(packageJsonText);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return [`${packageJsonPath}: invalid JSON: ${message}`];
  }
  const scripts = parsed?.scripts ?? {};
  for (const scriptName of REQUIRED_ENTERPRISE_NPM_SCRIPTS) {
    if (typeof scripts[scriptName] !== "string" || scripts[scriptName].trim() === "") {
      errors.push(`${packageJsonPath}: missing enterprise npm script '${scriptName}'`);
    }
  }
  return errors;
}

function validateEnterprisePackageAliases(
  manifestText,
  manifestPath = TEST_IMPACT_MANIFEST_PATH,
) {
  const errors = [];
  let parsed;
  try {
    parsed = JSON.parse(manifestText);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return [`${manifestPath}: invalid JSON: ${message}`];
  }
  const aliases = parsed?.packageScriptAliases ?? {};
  for (const scriptName of REQUIRED_ENTERPRISE_NPM_SCRIPTS) {
    if (typeof aliases[scriptName] !== "string" || aliases[scriptName].trim() === "") {
      errors.push(`${manifestPath}: missing enterprise package alias '${scriptName}'`);
    }
  }
  return errors;
}

function validateEnterprisePackageAliasCommands(
  packageJsonText,
  manifestText,
  packageJsonPath = PACKAGE_JSON_PATH,
  manifestPath = TEST_IMPACT_MANIFEST_PATH,
) {
  const errors = [];
  let packageJson;
  let manifest;
  try {
    packageJson = JSON.parse(packageJsonText);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return [`${packageJsonPath}: invalid JSON: ${message}`];
  }
  try {
    manifest = JSON.parse(manifestText);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return [`${manifestPath}: invalid JSON: ${message}`];
  }
  const scripts = packageJson?.scripts ?? {};
  const aliases = manifest?.packageScriptAliases ?? {};
  for (const scriptName of REQUIRED_ENTERPRISE_NPM_SCRIPTS) {
    if (scripts[scriptName] !== aliases[scriptName]) {
      errors.push(
        `${manifestPath}: enterprise package alias '${scriptName}' must match ${packageJsonPath} script`,
      );
    }
  }
  return errors;
}

function enterpriseGuardScriptPath(scriptCommand) {
  if (typeof scriptCommand !== "string") return null;
  const match = scriptCommand.match(
    /^node\s+(scripts\/ci\/[^\s]+\.mjs)(?:\s+--self-test)?(?:\s+&&\s+node\s+\1)?$/u,
  );
  return match?.[1] ?? null;
}

function validateEnterpriseGuardSelfTests(packageJsonText, packageJsonPath = PACKAGE_JSON_PATH) {
  const errors = [];
  let parsed;
  try {
    parsed = JSON.parse(packageJsonText);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return [`${packageJsonPath}: invalid JSON: ${message}`];
  }
  const scripts = parsed?.scripts ?? {};
  for (const scriptName of REQUIRED_ENTERPRISE_NPM_SCRIPTS) {
    const scriptPath = enterpriseGuardScriptPath(scripts[scriptName]);
    if (scriptPath === null) {
      errors.push(`${packageJsonPath}: enterprise script '${scriptName}' must run a scripts/ci/*.mjs guard through node`);
      continue;
    }
    const fullPath = path.join(repoRoot, scriptPath);
    if (!fs.existsSync(fullPath)) {
      errors.push(`${scriptPath}: enterprise guard script is missing`);
      continue;
    }
    const source = fs.readFileSync(fullPath, "utf8");
    if (!source.includes("--self-test")) {
      errors.push(`${scriptPath}: enterprise guard must expose --self-test`);
    }
  }
  return errors;
}

function runSelfTest() {
  const fake = {
    path: "fake.md",
    required: ["alpha", "beta"],
  };
  const content = "alpha only";
  const missing = fake.required.filter((required) => !content.includes(required));
  if (missing.length !== 1 || missing[0] !== "beta") {
    throw new Error("self-test failed: required phrase detection broken");
  }

  const missingArtifact = REQUIRED_ENTERPRISE_ARTIFACT_PATHS[0];
  const artifactErrors = validateRequiredArtifacts("/repo", (candidate) =>
    candidate !== path.join("/repo", missingArtifact),
  );
  if (artifactErrors.length !== 1 || !artifactErrors[0].includes(missingArtifact)) {
    throw new Error("self-test failed: missing enterprise artifact accepted");
  }

  const plannedMatrix = JSON.stringify({
    schema_version: 1,
    tests: [
      {
        id: "SEC-TEST-001",
        implementation_status: "planned",
        evidence: ["design evidence only"],
      },
    ],
  });
  if (validateTestMatrix(plannedMatrix, "test-matrix.json").length !== 0) {
    throw new Error("self-test failed: valid incomplete test matrix rejected");
  }
  const invalidMatrix = JSON.stringify({
    schema_version: 1,
    tests: [
      {
        id: "SEC-TEST-001",
        implementation_status: "complete",
        evidence: ["test evidence"],
      },
    ],
  });
  if (
    !validateTestMatrix(invalidMatrix, "test-matrix.json").some((error) =>
      error.includes("implementation_status is invalid"),
    )
  ) {
    throw new Error("self-test failed: invalid test matrix status accepted");
  }
  const contradictoryMatrix = JSON.stringify({
    schema_version: 1,
    tests: [
      {
        id: "SEC-TEST-001",
        implementation_status: "tested",
        evidence: ["database validation pending"],
      },
    ],
  });
  if (
    !validateTestMatrix(contradictoryMatrix, "test-matrix.json").some((error) =>
      error.includes("contradicts implementation_status"),
    )
  ) {
    throw new Error("self-test failed: contradictory test matrix evidence accepted");
  }
  const duplicateMatrix = JSON.stringify({
    schema_version: 1,
    tests: [
      { id: "SEC-TEST-001", implementation_status: "planned", evidence: ["first"] },
      { id: "SEC-TEST-001", implementation_status: "planned", evidence: ["second"] },
    ],
  });
  if (
    !validateTestMatrix(duplicateMatrix, "test-matrix.json").some((error) =>
      error.includes("is duplicated"),
    )
  ) {
    throw new Error("self-test failed: duplicate test matrix id accepted");
  }
  const emptyEvidenceMatrix = JSON.stringify({
    schema_version: 1,
    tests: [{ id: "SEC-TEST-001", implementation_status: "planned", evidence: [] }],
  });
  if (
    !validateTestMatrix(emptyEvidenceMatrix, "test-matrix.json").some((error) =>
      error.includes("evidence must contain non-empty strings"),
    )
  ) {
    throw new Error("self-test failed: empty test matrix evidence accepted");
  }

  const lifecycleMatrix = JSON.stringify({
    tests: [
      {
        id: GOVERNANCE_REVOCATION_TEST.matrixId,
        implementation_status: "tested",
        evidence: GOVERNANCE_SECURITY_EVIDENCE_TESTS.map(({ testName }) => testName),
      },
    ],
  });
  const lifecyclePaths = Object.fromEntries(
    GOVERNANCE_LIFECYCLE_FAMILIES.flatMap((family) =>
      GOVERNANCE_LIFECYCLE_ACTIONS.map((action) => [
        `/v1/prodex/gateway/${family}/{revision_id}/${action}`,
        {},
      ]),
    ),
  );
  const lifecycleSources = {};
  for (const evidenceTest of GOVERNANCE_SECURITY_EVIDENCE_TESTS) {
    lifecycleSources[evidenceTest.sourcePath] = [
      lifecycleSources[evidenceTest.sourcePath] ?? "",
      `#[test]\nfn ${evidenceTest.testName}() { let action = ${evidenceTest.requiredText ?? '"evidence"'}; }`,
    ].join("\n");
  }
  const lifecycleErrors = (matrix = lifecycleMatrix, sources = lifecycleSources) =>
    validateGovernanceLifecycleEvidence(
      matrix,
      JSON.stringify({ paths: lifecyclePaths }),
      sources,
    );
  if (lifecycleErrors().length !== 0) {
    throw new Error("self-test failed: valid governance lifecycle evidence rejected");
  }
  if (
    !lifecycleErrors(
      lifecycleMatrix.replace(
        GOVERNANCE_REVOCATION_TEST.testName,
        "arbitrary non-empty evidence",
      ),
    ).some((error) => error.includes("must cite exact repository test"))
  ) {
    throw new Error("self-test failed: arbitrary governance lifecycle evidence accepted");
  }
  if (
    !lifecycleErrors(
      lifecycleMatrix,
      {
        ...lifecycleSources,
        [GOVERNANCE_REVOCATION_TEST.sourcePath]: lifecycleSources[
          GOVERNANCE_REVOCATION_TEST.sourcePath
        ].replace("#[test]", "#[allow(dead_code)]"),
      },
    ).some((error) => error.includes("missing evidence test"))
  ) {
    throw new Error("self-test failed: non-test governance evidence symbol accepted");
  }
  const notificationTest = GOVERNANCE_SECURITY_EVIDENCE_TESTS[1];
  if (
    !lifecycleErrors(
      lifecycleMatrix,
      {
        ...lifecycleSources,
        [notificationTest.sourcePath]: lifecycleSources[notificationTest.sourcePath].replace(
          `fn ${notificationTest.testName}`,
          "fn unrelated_notification_test",
        ),
      },
    ).some((error) => error.includes(notificationTest.testName))
  ) {
    throw new Error("self-test failed: missing PostgreSQL notification evidence accepted");
  }
  delete lifecyclePaths["/v1/prodex/gateway/policies/{revision_id}/revoke"];
  if (
    !lifecycleErrors().some((error) =>
      error.includes("missing documented governance lifecycle route"),
    )
  ) {
    throw new Error("self-test failed: missing governance lifecycle route accepted");
  }

  const incompleteWorkflow = "name: CI\n- name: Enforce enterprise boundary guards\n  run: npm run ci:enterprise-docs-guard\n";
  const workflowErrors = validateEnterpriseWorkflow(incompleteWorkflow, "ci.yml");
  if (
    !workflowErrors.some((error) =>
      error.includes("npm run ci:deployment-security-guard"),
    )
  ) {
    throw new Error("self-test failed: missing enterprise workflow command accepted");
  }

  const completeWorkflow = [
    "name: CI",
    "- name: Enforce enterprise boundary guards",
    ...REQUIRED_ENTERPRISE_WORKFLOW_COMMANDS,
  ].join("\n");
  if (validateEnterpriseWorkflow(completeWorkflow, "ci.yml").length !== 0) {
    throw new Error("self-test failed: complete enterprise workflow rejected");
  }

  if (!FORBIDDEN_ENTERPRISE_DOC_PHRASES.some((entry) => entry.phrase === "prodex-42")) {
    throw new Error("self-test failed: forbidden legacy id example guard missing");
  }

  const incompletePackage = JSON.stringify({
    scripts: {
      "ci:enterprise-docs-guard": "node scripts/ci/enterprise-docs-guard.mjs",
    },
  });
  if (
    !validateEnterprisePackageScripts(incompletePackage, "package.json").some((error) =>
      error.includes("ci:deployment-security-guard"),
    )
  ) {
    throw new Error("self-test failed: missing enterprise npm script accepted");
  }

  const completePackage = JSON.stringify({
    scripts: Object.fromEntries(
      REQUIRED_ENTERPRISE_NPM_SCRIPTS.map((scriptName) => [scriptName, "node guard.mjs"]),
    ),
  });
  if (validateEnterprisePackageScripts(completePackage, "package.json").length !== 0) {
    throw new Error("self-test failed: complete enterprise npm scripts rejected");
  }

  const incompleteManifest = JSON.stringify({
    packageScriptAliases: {
      "ci:enterprise-docs-guard": "node scripts/ci/enterprise-docs-guard.mjs",
    },
  });
  if (
    !validateEnterprisePackageAliases(incompleteManifest, "test-impact-manifest.json").some(
      (error) => error.includes("ci:deployment-security-guard"),
    )
  ) {
    throw new Error("self-test failed: missing enterprise package alias accepted");
  }

  const completeManifest = JSON.stringify({
    packageScriptAliases: Object.fromEntries(
      REQUIRED_ENTERPRISE_NPM_SCRIPTS.map((scriptName) => [scriptName, "node guard.mjs"]),
    ),
  });
  if (
    validateEnterprisePackageAliases(completeManifest, "test-impact-manifest.json").length !== 0
  ) {
    throw new Error("self-test failed: complete enterprise package aliases rejected");
  }

  const mismatchedPackage = JSON.stringify({
    scripts: Object.fromEntries(
      REQUIRED_ENTERPRISE_NPM_SCRIPTS.map((scriptName) => [scriptName, "node package.mjs"]),
    ),
  });
  const mismatchedManifest = JSON.stringify({
    packageScriptAliases: Object.fromEntries(
      REQUIRED_ENTERPRISE_NPM_SCRIPTS.map((scriptName) => [scriptName, "node manifest.mjs"]),
    ),
  });
  if (
    !validateEnterprisePackageAliasCommands(
      mismatchedPackage,
      mismatchedManifest,
      "package.json",
      "test-impact-manifest.json",
    ).some((error) => error.includes("must match"))
  ) {
    throw new Error("self-test failed: mismatched enterprise package alias accepted");
  }

  if (
    validateEnterprisePackageAliasCommands(
      completePackage,
      completeManifest,
      "package.json",
      "test-impact-manifest.json",
    ).length !== 0
  ) {
    throw new Error("self-test failed: matching enterprise package aliases rejected");
  }

  const validSelfTestPackage = JSON.stringify({
    scripts: Object.fromEntries(
      REQUIRED_ENTERPRISE_NPM_SCRIPTS.map((scriptName) => [
        scriptName,
        "node scripts/ci/enterprise-docs-guard.mjs --self-test && node scripts/ci/enterprise-docs-guard.mjs",
      ]),
    ),
  });
  if (validateEnterpriseGuardSelfTests(validSelfTestPackage, "package.json").length !== 0) {
    throw new Error("self-test failed: explicit guard self-test command rejected");
  }

  const invalidSelfTestPackage = JSON.stringify({
    scripts: Object.fromEntries(
      REQUIRED_ENTERPRISE_NPM_SCRIPTS.map((scriptName) => [scriptName, "echo guard"]),
    ),
  });
  if (
    !validateEnterpriseGuardSelfTests(invalidSelfTestPackage, "package.json").some((error) =>
      error.includes("must run a scripts/ci/*.mjs guard through node"),
    )
  ) {
    throw new Error("self-test failed: non-guard enterprise script accepted");
  }
}

function main() {
  if (process.argv.includes("--self-test")) {
    runSelfTest();
    return;
  }
  runSelfTest();

  const errors = DOCUMENTS.flatMap(validateDocument);
  errors.push(...validateRequiredArtifacts());
  const testMatrixPath = path.join(repoRoot, TEST_MATRIX_PATH);
  let testMatrixText = null;
  if (fs.existsSync(testMatrixPath)) {
    testMatrixText = fs.readFileSync(testMatrixPath, "utf8");
    errors.push(
      ...validateTestMatrix(testMatrixText, TEST_MATRIX_PATH),
    );
  }
  const lifecycleOpenapiPath = path.join(repoRoot, GOVERNANCE_LIFECYCLE_OPENAPI_PATH);
  const evidenceSources = {};
  for (const { sourcePath } of GOVERNANCE_SECURITY_EVIDENCE_TESTS) {
    if (Object.hasOwn(evidenceSources, sourcePath)) continue;
    const fullPath = path.join(repoRoot, sourcePath);
    if (!fs.existsSync(fullPath)) {
      errors.push(`${sourcePath}: required governance evidence source is missing`);
    } else {
      evidenceSources[sourcePath] = fs.readFileSync(fullPath, "utf8");
    }
  }
  if (!fs.existsSync(lifecycleOpenapiPath)) {
    errors.push(`${GOVERNANCE_LIFECYCLE_OPENAPI_PATH}: required governance OpenAPI is missing`);
  } else if (testMatrixText !== null) {
    errors.push(
      ...validateGovernanceLifecycleEvidence(
        testMatrixText,
        fs.readFileSync(lifecycleOpenapiPath, "utf8"),
        evidenceSources,
      ),
    );
  }
  errors.push(...validateForbiddenEnterpriseDocPhrases());
  const workflowPath = path.join(repoRoot, WORKFLOW_PATH);
  if (!fs.existsSync(workflowPath)) {
    errors.push(`${WORKFLOW_PATH}: required CI workflow is missing`);
  } else {
    errors.push(
      ...validateEnterpriseWorkflow(
        fs.readFileSync(workflowPath, "utf8"),
        WORKFLOW_PATH,
      ),
    );
  }
  const packageJsonPath = path.join(repoRoot, PACKAGE_JSON_PATH);
  let packageJsonText = null;
  if (!fs.existsSync(packageJsonPath)) {
    errors.push(`${PACKAGE_JSON_PATH}: required package manifest is missing`);
  } else {
    packageJsonText = fs.readFileSync(packageJsonPath, "utf8");
    errors.push(
      ...validateEnterprisePackageScripts(
        packageJsonText,
        PACKAGE_JSON_PATH,
      ),
    );
  }
  const testImpactManifestPath = path.join(repoRoot, TEST_IMPACT_MANIFEST_PATH);
  let testImpactManifestText = null;
  if (!fs.existsSync(testImpactManifestPath)) {
    errors.push(`${TEST_IMPACT_MANIFEST_PATH}: required test impact manifest is missing`);
  } else {
    testImpactManifestText = fs.readFileSync(testImpactManifestPath, "utf8");
    errors.push(
      ...validateEnterprisePackageAliases(
        testImpactManifestText,
        TEST_IMPACT_MANIFEST_PATH,
      ),
    );
  }
  if (packageJsonText !== null && testImpactManifestText !== null) {
    errors.push(
      ...validateEnterprisePackageAliasCommands(
        packageJsonText,
        testImpactManifestText,
        PACKAGE_JSON_PATH,
        TEST_IMPACT_MANIFEST_PATH,
      ),
    );
  }
  if (packageJsonText !== null) {
    errors.push(...validateEnterpriseGuardSelfTests(packageJsonText, PACKAGE_JSON_PATH));
  }
  if (errors.length > 0) {
    for (const error of errors) process.stderr.write(`${error}\n`);
    process.exitCode = 1;
  }
}

main();
