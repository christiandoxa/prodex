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
const TEST_MATRIX_SCHEMA_VERSION = 2;
const TEST_MATRIX_STATUSES = new Set([
  "tested",
  "implemented",
  "pending_validation",
  "partial",
  "planned",
]);
const ORDINARY_TEST_REFERENCE_PATTERN = /^[A-Za-z_][A-Za-z0-9_]*$/u;
const TEST_DECLARATION_PATTERN = /#\[(?:(?:[\w:]+::)?(?:test|rstest)|test_case)(?:\([^\]]*\))?\]\s*(?:#\[[^\]]+\]\s*)*(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(/gu;
let repositoryTestNamesCache;
let packageScriptNamesCache;
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
  {
    matrixId: "SEC-POL-003",
    testName: "governance_invalidation_outbox_is_bounded_tenant_scoped_and_transactional",
    sourcePath: "crates/prodex-storage-postgres/tests/postgres_migration.rs",
  },
  ...[
    "notify_payload_is_only_a_bounded_wakeup_hint",
    "durable_event_is_acknowledged_only_after_refresh",
  ].map((testName) => ({
    matrixId: "SEC-POL-003",
    testName,
    sourcePath:
      "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite/governance_invalidation.rs",
  })),
  {
    matrixId: "SEC-POL-003",
    testName: "governance_invalidation_outbox_converges_replicas_and_compacts_safely",
    sourcePath:
      "crates/prodex-storage-postgres-runtime/tests/postgres_runtime/invalidation_outbox.rs",
  },
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
const REQUIRED_ENTERPRISE_WORKFLOW_COMMANDS = [
  "node scripts/ci/enterprise-docs-guard.mjs --self-test && node scripts/ci/enterprise-docs-guard.mjs",
  "node scripts/ci/enterprise-id-boundary-guard.mjs --self-test && node scripts/ci/enterprise-id-boundary-guard.mjs",
  "node scripts/ci/enterprise-binaries-guard.mjs --self-test && node scripts/ci/enterprise-binaries-guard.mjs",
  "node scripts/ci/application-boundary-guard.mjs --self-test && node scripts/ci/application-boundary-guard.mjs",
  "node scripts/ci/auth-boundary-guard.mjs --self-test && node scripts/ci/auth-boundary-guard.mjs",
  "node scripts/ci/config-boundary-guard.mjs --self-test && node scripts/ci/config-boundary-guard.mjs",
  "node scripts/ci/control-plane-boundary-guard.mjs --self-test && node scripts/ci/control-plane-boundary-guard.mjs",
  "node scripts/ci/observability-boundary-guard.mjs --self-test && node scripts/ci/observability-boundary-guard.mjs",
  "node scripts/ci/provider-spi-boundary-guard.mjs --self-test && node scripts/ci/provider-spi-boundary-guard.mjs",
  "node scripts/ci/storage-boundary-guard.mjs --self-test && node scripts/ci/storage-boundary-guard.mjs",
  "node scripts/ci/backup-restore-drill.mjs --self-test && node scripts/ci/backup-restore-drill.mjs",
  "node scripts/ci/storage-postgres-boundary-guard.mjs --self-test && node scripts/ci/storage-postgres-boundary-guard.mjs",
  "node scripts/ci/storage-redis-boundary-guard.mjs --self-test && node scripts/ci/storage-redis-boundary-guard.mjs",
  "node scripts/ci/storage-sqlite-boundary-guard.mjs --self-test && node scripts/ci/storage-sqlite-boundary-guard.mjs",
  "node scripts/ci/gateway-core-boundary-guard.mjs --self-test && node scripts/ci/gateway-core-boundary-guard.mjs",
  "node scripts/ci/gateway-http-boundary-guard.mjs --self-test && node scripts/ci/gateway-http-boundary-guard.mjs",
  "node scripts/ci/deployment-security-guard.mjs --self-test && node scripts/ci/deployment-security-guard.mjs",
];
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

function repositoryTestNames() {
  if (repositoryTestNamesCache !== undefined) return repositoryTestNamesCache;

  const names = new Set();
  const pending = ["crates", "scripts"]
    .map((relativePath) => path.join(repoRoot, relativePath))
    .filter((directory) => fs.existsSync(directory));
  while (pending.length > 0) {
    const directory = pending.pop();
    for (const entry of fs.readdirSync(directory, { withFileTypes: true })) {
      const entryPath = path.join(directory, entry.name);
      if (entry.isDirectory()) {
        if (!new Set([".git", "node_modules", "target"]).has(entry.name)) {
          pending.push(entryPath);
        }
        continue;
      }
      if (!entry.isFile() || !/[.](?:rs|mjs|js|ts)$/u.test(entry.name)) continue;
      const source = fs.readFileSync(entryPath, "utf8");
      for (const match of source.matchAll(TEST_DECLARATION_PATTERN)) names.add(match[1]);
    }
  }
  repositoryTestNamesCache = names;
  return names;
}

function packageScriptNames() {
  if (packageScriptNamesCache !== undefined) return packageScriptNamesCache;
  const packagePath = path.join(repoRoot, PACKAGE_JSON_PATH);
  try {
    const parsed = JSON.parse(fs.readFileSync(packagePath, "utf8"));
    packageScriptNamesCache = new Set(Object.keys(parsed?.scripts ?? {}));
  } catch {
    packageScriptNamesCache = new Set();
  }
  return packageScriptNamesCache;
}

function validateTestCommand(command, status, index, matrixPath, testNames) {
  if (/^(?:external|planned):\s*/u.test(command)) {
    if (["tested", "implemented"].includes(status)) {
      return `${matrixPath}: tests[${index}].test_command cannot be external or planned for implementation_status '${status}'`;
    }
    return null;
  }

  const npmMatch = /^npm run ([A-Za-z0-9:_-]+)$/u.exec(command);
  if (npmMatch !== null) {
    if (!packageScriptNames().has(npmMatch[1])) {
      return `${matrixPath}: tests[${index}].test_command references missing npm script '${npmMatch[1]}'`;
    }
    return null;
  }

  const cargoMatch = /^cargo test(?:\s+\S+)*\s+--\s+([A-Za-z_][A-Za-z0-9_]*)$/u.exec(command);
  if (cargoMatch !== null) {
    if (!testNames.has(cargoMatch[1])) {
      return `${matrixPath}: tests[${index}].test_command references missing repository test '${cargoMatch[1]}'`;
    }
    return null;
  }

  const nodeMatch = /^node\s+(scripts\/ci\/[^\s]+\.mjs)(?:\s+--self-test)?$/u.exec(command);
  if (nodeMatch !== null && fs.existsSync(path.join(repoRoot, nodeMatch[1]))) return null;

  return `${matrixPath}: tests[${index}].test_command must be a stable cargo test, npm run, node guard, planned:, or external: command`;
}

function validateEvidenceReferences(evidence, index, matrixPath, testNames) {
  const errors = [];
  evidence.forEach((item, evidenceIndex) => {
    if (typeof item !== "string") return;
    if (ORDINARY_TEST_REFERENCE_PATTERN.test(item) && !testNames.has(item)) {
      errors.push(
        `${matrixPath}: tests[${index}].evidence[${evidenceIndex}] ordinary test reference '${item}' does not resolve to a repository test`,
      );
    }
    if (/^[A-Za-z0-9_-]+-guard\.mjs$/u.test(item) && !fs.existsSync(path.join(repoRoot, "scripts/ci", item))) {
      errors.push(
        `${matrixPath}: tests[${index}].evidence[${evidenceIndex}] guard reference '${item}' does not resolve to scripts/ci/${item}`,
      );
    }
  });
  return errors;
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
  if (parsed?.schema_version !== TEST_MATRIX_SCHEMA_VERSION) {
    errors.push(`${matrixPath}: schema_version must be ${TEST_MATRIX_SCHEMA_VERSION}`);
  }
  if (!Array.isArray(parsed?.tests) || parsed.tests.length === 0) {
    errors.push(`${matrixPath}: tests must be a non-empty array`);
    return errors;
  }
  const ids = new Set();
  const testNames = repositoryTestNames();
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
    if (!Number.isInteger(test?.phase) || test.phase < 1) {
      errors.push(`${matrixPath}: tests[${index}].phase must be a positive integer`);
    }
    if (
      !Array.isArray(test?.modes) ||
      test.modes.length === 0 ||
      test.modes.some((mode) => typeof mode !== "string" || mode.trim() === "")
    ) {
      errors.push(`${matrixPath}: tests[${index}].modes must contain non-empty strings`);
    }
    for (const field of ["test_command", "expected", "evidence_artifact", "owner"]) {
      if (typeof test?.[field] !== "string" || test[field].trim() === "") {
        errors.push(`${matrixPath}: tests[${index}].${field} must be a non-empty string`);
      }
    }
    if (typeof test?.test_command === "string" && test.test_command.trim() !== "") {
      const commandError = validateTestCommand(
        test.test_command,
        test.implementation_status,
        index,
        matrixPath,
        testNames,
      );
      if (commandError !== null) errors.push(commandError);
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
    if (Array.isArray(test?.evidence)) {
      errors.push(...validateEvidenceReferences(test.evidence, index, matrixPath, testNames));
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

function runSelfTest() {
  const validMatrixRow = (overrides = {}) => ({
    id: "SEC-TEST-001",
    phase: 1,
    modes: ["enterprise_enforce"],
    test_command: "planned: deployment evidence command",
    expected: "The control remains explicitly tracked.",
    evidence_artifact: "evidence/enterprise-test-matrix/SEC-TEST-001/",
    owner: "prodex-security",
    implementation_status: "planned",
    evidence: ["design evidence only"],
    ...overrides,
  });
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
    schema_version: TEST_MATRIX_SCHEMA_VERSION,
    tests: [validMatrixRow()],
  });
  if (validateTestMatrix(plannedMatrix, "test-matrix.json").length !== 0) {
    throw new Error("self-test failed: valid incomplete test matrix rejected");
  }
  const invalidMatrix = JSON.stringify({
    schema_version: TEST_MATRIX_SCHEMA_VERSION,
    tests: [validMatrixRow({
      implementation_status: "complete",
      evidence: ["test evidence"],
    })],
  });
  if (
    !validateTestMatrix(invalidMatrix, "test-matrix.json").some((error) =>
      error.includes("implementation_status is invalid"),
    )
  ) {
    throw new Error("self-test failed: invalid test matrix status accepted");
  }
  for (const field of ["id", "phase", "modes", "test_command", "expected", "evidence_artifact", "owner"]) {
    const incompleteRow = validMatrixRow();
    delete incompleteRow[field];
    if (
      !validateTestMatrix(
        JSON.stringify({ schema_version: TEST_MATRIX_SCHEMA_VERSION, tests: [incompleteRow] }),
        "test-matrix.json",
      ).some((error) => error.includes(`tests[0].${field}`))
    ) {
      throw new Error(`self-test failed: missing test matrix field '${field}' accepted`);
    }
  }
  const validReferenceMatrix = JSON.stringify({
    schema_version: TEST_MATRIX_SCHEMA_VERSION,
    tests: [validMatrixRow({
      test_command: "cargo test --locked --workspace -- explicit_deny_wins_and_drops_obligations",
      evidence: ["explicit_deny_wins_and_drops_obligations"],
    })],
  });
  if (validateTestMatrix(validReferenceMatrix, "test-matrix.json").length !== 0) {
    throw new Error("self-test failed: valid ordinary test reference rejected");
  }
  const staleReferenceMatrix = JSON.stringify({
    schema_version: TEST_MATRIX_SCHEMA_VERSION,
    tests: [validMatrixRow({ evidence: ["missing_test_reference"] })],
  });
  if (
    !validateTestMatrix(staleReferenceMatrix, "test-matrix.json").some((error) =>
      error.includes("ordinary test reference") && error.includes("missing_test_reference"),
    )
  ) {
    throw new Error("self-test failed: stale ordinary test reference accepted");
  }
  const staleGuardReferenceMatrix = JSON.stringify({
    schema_version: TEST_MATRIX_SCHEMA_VERSION,
    tests: [validMatrixRow({ evidence: ["missing-guard.mjs"] })],
  });
  if (
    !validateTestMatrix(staleGuardReferenceMatrix, "test-matrix.json").some((error) =>
      error.includes("guard reference") && error.includes("missing-guard.mjs"),
    )
  ) {
    throw new Error("self-test failed: stale guard reference accepted");
  }
  const contradictoryMatrix = JSON.stringify({
    schema_version: TEST_MATRIX_SCHEMA_VERSION,
    tests: [validMatrixRow({
      implementation_status: "tested",
      test_command: "cargo test --locked --workspace -- explicit_deny_wins_and_drops_obligations",
      evidence: ["database validation pending"],
    })],
  });
  if (
    !validateTestMatrix(contradictoryMatrix, "test-matrix.json").some((error) =>
      error.includes("contradicts implementation_status"),
    )
  ) {
    throw new Error("self-test failed: contradictory test matrix evidence accepted");
  }
  const duplicateMatrix = JSON.stringify({
    schema_version: TEST_MATRIX_SCHEMA_VERSION,
    tests: [
      validMatrixRow({ evidence: ["first"] }),
      validMatrixRow({ evidence: ["second"] }),
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
    schema_version: TEST_MATRIX_SCHEMA_VERSION,
    tests: [validMatrixRow({ evidence: [] })],
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

  const incompleteWorkflow = "name: CI\n- name: Enforce enterprise boundary guards\n  run: node scripts/ci/enterprise-docs-guard.mjs --self-test && node scripts/ci/enterprise-docs-guard.mjs\n";
  const workflowErrors = validateEnterpriseWorkflow(incompleteWorkflow, "ci.yml");
  if (
    !workflowErrors.some((error) =>
      error.includes("node scripts/ci/deployment-security-guard.mjs --self-test && node scripts/ci/deployment-security-guard.mjs"),
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
  if (errors.length > 0) {
    for (const error of errors) process.stderr.write(`${error}\n`);
    process.exitCode = 1;
  }
}

main();
