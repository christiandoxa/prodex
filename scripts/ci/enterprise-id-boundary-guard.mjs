#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..", "..");

const ENTERPRISE_BOUNDARY_CRATES = Object.freeze([
  "prodex-application",
  "prodex-authn",
  "prodex-authz",
  "prodex-config",
  "prodex-control-plane",
  "prodex-domain",
  "prodex-gateway-core",
  "prodex-gateway-http",
  "prodex-observability",
  "prodex-provider-core",
  "prodex-provider-spi",
  "prodex-storage",
  "prodex-storage-postgres",
  "prodex-storage-redis",
  "prodex-storage-sqlite",
]);

const FORBIDDEN_ID_GENERATOR_PATTERNS = Object.freeze([
  { name: "process-local AtomicU64", pattern: /\bAtomicU64\b/u },
  { name: "monotonic counter increment", pattern: /\.fetch_add\s*\(/u },
]);

async function rustFilesUnder(dir) {
  const entries = await fs.readdir(dir, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await rustFilesUnder(fullPath)));
    } else if (entry.isFile() && entry.name.endsWith(".rs")) {
      files.push(fullPath);
    }
  }
  return files;
}

export function validateDomainIdSource(sourceText) {
  const errors = [];
  for (const idName of [
    "TenantId",
    "PrincipalId",
    "RequestId",
    "CallId",
    "ReservationId",
    "VirtualKeyId",
    "PolicyRevisionId",
    "AuditEventId",
  ]) {
    if (!sourceText.includes(`domain_id!(${idName},`)) {
      errors.push(`crates/prodex-domain/src/ids.rs: missing ${idName} typed identifier`);
    }
  }
  if (!sourceText.includes("Uuid::now_v7()")) {
    errors.push("crates/prodex-domain/src/ids.rs: domain_id! must generate UUIDv7 identifiers");
  }
  return errors;
}

export function validateBoundarySource(sourceText, sourcePath = "source.rs") {
  const errors = [];
  sourceText.split(/\r?\n/u).forEach((line, index) => {
    for (const { name, pattern } of FORBIDDEN_ID_GENERATOR_PATTERNS) {
      if (pattern.test(line)) {
        errors.push(
          `${sourcePath}:${index + 1}: enterprise boundary must not use ${name} for request/call/resource identifiers`,
        );
      }
    }
  });
  return errors;
}

function assertSelfTest(condition, message) {
  if (!condition) throw new Error(`self-test failed: ${message}`);
}

function runSelfTest() {
  assertSelfTest(
    ENTERPRISE_BOUNDARY_CRATES.includes("prodex-provider-core"),
    "provider core must be scanned for process-local identifiers",
  );
  const validIds = `
macro_rules! domain_id {
    ($name:ident, $kind:literal) => {
        impl $name {
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }
        }
    };
}
domain_id!(TenantId, "tenant_id");
domain_id!(PrincipalId, "principal_id");
domain_id!(RequestId, "request_id");
domain_id!(CallId, "call_id");
domain_id!(ReservationId, "reservation_id");
domain_id!(VirtualKeyId, "virtual_key_id");
domain_id!(PolicyRevisionId, "policy_revision_id");
domain_id!(AuditEventId, "audit_event_id");
`;
  assertSelfTest(validateDomainIdSource(validIds).length === 0, "valid typed ID source rejected");
  assertSelfTest(
    validateDomainIdSource(validIds.replace("Uuid::now_v7()", "Uuid::new_v4()")).some((error) =>
      error.includes("UUIDv7"),
    ),
    "non-UUIDv7 ID generator accepted",
  );
  assertSelfTest(
    validateDomainIdSource(validIds.replace('domain_id!(CallId, "call_id");', "")).some((error) =>
      error.includes("CallId"),
    ),
    "missing typed CallId accepted",
  );
  assertSelfTest(
    validateBoundarySource("use std::sync::atomic::AtomicU64;", "bad.rs").some((error) =>
      error.includes("AtomicU64"),
    ),
    "process-local AtomicU64 accepted",
  );
  assertSelfTest(
    validateBoundarySource("let id = next.fetch_add(1, Ordering::Relaxed);", "bad.rs").some(
      (error) => error.includes("monotonic counter increment"),
    ),
    "monotonic fetch_add identity accepted",
  );
  assertSelfTest(
    validateBoundarySource("let request_id = RequestId::new();", "good.rs").length === 0,
    "typed ID source rejected",
  );
}

async function main() {
  if (process.argv.includes("--self-test")) {
    runSelfTest();
    return;
  }

  runSelfTest();

  const errors = [];
  const idsPath = path.join(repoRoot, "crates/prodex-domain/src/ids.rs");
  const idsSource = await fs.readFile(idsPath, "utf8");
  errors.push(...validateDomainIdSource(idsSource));

  for (const crateName of ENTERPRISE_BOUNDARY_CRATES) {
    const srcDir = path.join(repoRoot, "crates", crateName, "src");
    const files = await rustFilesUnder(srcDir);
    for (const file of files) {
      const source = await fs.readFile(file, "utf8");
      errors.push(...validateBoundarySource(source, path.relative(repoRoot, file)));
    }
  }

  if (errors.length > 0) {
    console.error(errors.join("\n"));
    process.exit(1);
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
