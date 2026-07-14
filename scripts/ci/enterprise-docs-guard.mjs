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
    ],
  },
  {
    path: "docs/migration-guide.md",
    required: [
      "# Prodex Enterprise Migration Guide",
      "## Phase 0: Baseline and Characterization",
      "## Phase 2: Application and Gateway Boundaries",
      "## Phase 4: Durable Storage and Migrations",
      "## Phase 5: Reservation-Based Accounting",
      "## Phase 6: Async Gateway Adapter",
      "## Release Gates",
      "external migrators",
      "PostgreSQL Row-Level Security",
      "Redis is not used as durable whole-map billing state",
    ],
  },
  {
    path: "docs/enterprise-readiness-audit.md",
    required: [
      "# Prodex Enterprise Readiness Audit",
      "## Audit Matrix",
      "SSO Role Fallback Must Not Become Admin",
      "Root Token Must Not Bypass Data-Plane Authorization",
      "Process-Local IDs Are Not Globally Unique",
      "Ledger Uniqueness Must Survive Multi-Replica Writes",
      "Read-Modify-Write Budget Updates Can Lose Usage",
      "Admission Must Not Use Only Local In-Memory Usage",
      "Tenant ID Must Be Mandatory and Keyed",
      "DDL Must Not Run While Opening Request-Serving Backends",
      "Redis Must Not Store Whole-Map Durable Billing JSON",
      "OIDC Discovery and JWKS Fetch Must Stay Off Request Path",
      "Blocking HTTP, Worker Threads, and Mutex Hot Paths Must Be Bounded",
      "Dependency Inversion Must Keep Domain and Shared Logic Clean",
      "Telemetry Must Propagate End-to-End Trace Context",
      "Config and Policy Cache Need Revision and Last-Known-Good Semantics",
      "Remaining gap",
      "Cross-Cutting Release Gates",
    ],
  },
];

const REQUIRED_ENTERPRISE_ARTIFACT_PATHS = [
  ...[
    "00-baseline-and-inventory.md",
    "01-target-architecture.md",
    "02-trust-boundaries-and-data-flow.md",
    "03-data-classification-and-inspection.md",
    "04-policy-model-and-obligations.md",
    "05-approval-and-break-glass.md",
    "06-provider-registry-and-routing.md",
    "07-identity-session-and-api-gateway.md",
    "08-audit-siem-and-secret-management.md",
    "09-storage-ha-backup-and-dr.md",
    "10-rollout-rollback-and-compatibility.md",
    "11-security-test-matrix.md",
    "12-performance-baseline-and-results.md",
    "13-operator-runbooks.md",
    "14-final-implementation-report.md",
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
  parsed.tests.forEach((test, index) => {
    if (typeof test?.id !== "string" || test.id.trim() === "") {
      errors.push(`${matrixPath}: tests[${index}].id must be a non-empty string`);
    }
    if (!TEST_MATRIX_STATUSES.has(test?.implementation_status)) {
      errors.push(`${matrixPath}: tests[${index}].implementation_status is invalid`);
    }
  });
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
    tests: [{ id: "SEC-TEST-001", implementation_status: "planned" }],
  });
  if (validateTestMatrix(plannedMatrix, "test-matrix.json").length !== 0) {
    throw new Error("self-test failed: valid incomplete test matrix rejected");
  }
  const invalidMatrix = JSON.stringify({
    schema_version: 1,
    tests: [{ id: "SEC-TEST-001", implementation_status: "complete" }],
  });
  if (
    !validateTestMatrix(invalidMatrix, "test-matrix.json").some((error) =>
      error.includes("implementation_status is invalid"),
    )
  ) {
    throw new Error("self-test failed: invalid test matrix status accepted");
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
  if (fs.existsSync(testMatrixPath)) {
    errors.push(
      ...validateTestMatrix(fs.readFileSync(testMatrixPath, "utf8"), TEST_MATRIX_PATH),
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
