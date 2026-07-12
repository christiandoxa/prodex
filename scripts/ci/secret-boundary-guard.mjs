#!/usr/bin/env node
import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import { git } from "./guard-common.mjs";
import { repoRoot } from "../npm/common.mjs";

const SENSITIVE = "(?:access[_-]?token|api[_-]?key|auth[_-]?token|bearer|capability|credential|password|secret|token)";
const ARG_BLOCK = /#\s*\[\s*arg\s*\((?<options>[\s\S]{0,1000}?)\)\s*\]\s*(?:pub(?:\([^)]*\))?\s+)?(?<field>[A-Za-z_][A-Za-z0-9_]*)\s*:/gu;
const EXPLICIT_LONG = new RegExp(`\\blong\\s*=\\s*"(${SENSITIVE})"`, "iu");
const LONG_OPTION = new RegExp(`\\blong\\s*=\\s*"(${SENSITIVE})"`, "giu");
const MANUAL_FLAG = new RegExp(`"--(${SENSITIVE})(?:=)?"`, "giu");
const QUERY_CAPABILITY = new RegExp(
  `(?:[a-z][a-z0-9+.-]*://|["'])[^\\s"']{0,1024}[?&]${SENSITIVE}=`,
  "giu",
);
const PATH_CAPABILITY = new RegExp(
  `(?:[a-z][a-z0-9+.-]*://|["'])[^\\s"']{0,1024}/\\{[^}]*${SENSITIVE}[^}]*\\}`,
  "giu",
);
const USERINFO_CAPABILITY = new RegExp(
  `[a-z][a-z0-9+.-]*://[^\\s/:"']{1,256}:\\{[^}]*${SENSITIVE}[^}]*\\}@`,
  "giu",
);
const DOCUMENTED_SECRET_ARG = new RegExp(
  `^\\s*(?:\\$\\s*)?prodex\\b[^\\n]*--${SENSITIVE}(?:=|\\s+\\S)`,
  "gimu",
);
const WEBSOCKET_SECRET_QUERY = /(?:[?&]|\{[^}]*separator[^}]*\})(?:key|access[_-]?token)=|(?:append_pair|query_pair)\s*\(\s*["'](?:key|access[_-]?token)["']/giu;
const PRODUCTION_WEBSOCKET_URL_FILES = new Set([
  "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gemini_live.rs",
  "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gemini_live/connection.rs",
]);
const RUNTIME_GATEWAY_SECRET_BOUNDARIES = new Map([
  [
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_config.rs",
    [
      "http_bearer_token: Option<RuntimeGatewaySecret>",
      "bearer_token: Option<RuntimeGatewaySecret>",
    ],
  ],
  [
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_options.rs",
    [
      "DevelopmentCompatibility(SecretMaterial)",
      "credential: RuntimeProjectedProviderCredential",
      "source: Arc<RuntimeGatewaySecretSource>",
    ],
  ],
  [
    "crates/prodex-app/src/app_commands/runtime_launch/gateway_secret_config.rs",
    [
      "RuntimeGatewaySecret::projected",
      "RuntimeGatewaySecret::development_compatibility",
    ],
  ],
  [
    "crates/prodex-app/src/app_commands/runtime_launch/gateway_observability_config.rs",
    ["resolver.runtime_secret("],
  ],
  [
    "crates/prodex-app/src/app_commands/runtime_launch/gateway_guardrail_config.rs",
    ["resolver.runtime_secret("],
  ],
  [
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_transport/projected_credential.rs",
    [
      "runtime_gateway_with_outbound_secret<T>",
      "SecretResolutionRequest::new(",
      "material.with_exposed_secret(",
    ],
  ],
  [
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_transport/observability.rs",
    ["runtime_gateway_with_outbound_secret(secret"],
  ],
  [
    "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_guardrail_webhook.rs",
    ["runtime_gateway_with_outbound_secret(secret"],
  ],
]);
const RAW_RUNTIME_GATEWAY_SECRET = /(?:http_bearer_token|bearer_token)\s*:\s*Option\s*<\s*String\s*>|(?:http_bearer_token|bearer_token)\.as_deref\s*\(/gu;
const SECRET_BEARING_DEBUG_TYPES = [
  "RuntimeProxyRequest",
  "ProviderTransformInput",
  "RuntimeAnthropicMcpServer",
  "RuntimeAnthropicTranslatedTools",
  "CopilotConfigFile",
  "OtlpHttpLogExportConfig",
  "ControlPlaneHttpHeaderFile",
];
const RAW_SECRET_DERIVED_DEBUG = new RegExp(
  `#\\s*\\[\\s*derive\\s*\\([^)]*\\bDebug\\b[^)]*\\)\\s*\\]\\s*(?:#\\s*\\[[^\\]]*\\]\\s*)*(?:pub(?:\\([^)]*\\))?\\s+)?struct\\s+(?<type>${SECRET_BEARING_DEBUG_TYPES.join("|")})\\b`,
  "gu",
);
const PROVIDER_CONFORMANCE_ALL_HEADERS =
  /input\.headers\s*=\s*request\.headers\.iter\(\)\.cloned\(\)\.collect\(\)/gu;
const DEVELOPMENT_GATEWAY_SECRET_CONSTRUCTION = /RuntimeGatewaySecret::development_compatibility\s*\(/gu;
const GATEWAY_SECRET_RESOLVER =
  "crates/prodex-app/src/app_commands/runtime_launch/gateway_secret_config.rs";
const DEFERRED_GATEWAY_SECRET_CONFIG_FILES = new Set([
  "crates/prodex-app/src/app_commands/runtime_launch/gateway_observability_config.rs",
  "crates/prodex-app/src/app_commands/runtime_launch/gateway_guardrail_config.rs",
]);
const TEST_PATH = /(?:^|\/)(?:tests?|fixtures?|[^/]+_tests?)(?:\/|$)|(?:^|\/)(?:tests?|[^/]+_tests?)\.rs$|\.test\.mjs$|^scripts\/ci\/secret-boundary-guard\.mjs$/u;

// Compatibility debt only: new occurrences exhaust these per-file budgets and fail.
const LEGACY_SECRET_FLAG_BUDGET = new Map([
  ["crates/prodex-cli/src/runtime_args.rs:api-key", 2],
  ["crates/prodex-cli/src/runtime_args.rs:auth-token", 1],
  ["crates/prodex-cli/src/runtime_args/super_tail_extract.rs:api-key", 3],
  ["crates/prodex-app/src/app_commands/runtime_launch/providers_env.rs:api-key", 4],
]);

function lineNumber(contents, index) {
  return contents.slice(0, index).split(/\r\n|\n|\r/u).length;
}

function normalizedFlag(value) {
  return value.toLowerCase().replaceAll("_", "-");
}

function isTestFixture(filePath, contents, index) {
  if (TEST_PATH.test(filePath)) return true;
  const testModule = contents.search(/^#\s*\[cfg\(test\)\]/mu);
  return testModule !== -1 && index >= testModule;
}

function pushViolation(violations, filePath, contents, index, kind) {
  if (isTestFixture(filePath, contents, index)) return;
  const end = contents.indexOf("\n", index);
  violations.push({
    filePath,
    line: lineNumber(contents, index),
    kind,
    snippet: contents.slice(index, end === -1 ? undefined : end).trim().slice(0, 240),
  });
}

export function validateFiles(files) {
  const violations = [];
  const legacyHits = new Map();
  const recordFlag = (filePath, contents, index, rawFlag) => {
    if (isTestFixture(filePath, contents, index)) return;
    const flag = normalizedFlag(rawFlag);
    const key = `${filePath}:${flag}`;
    legacyHits.set(key, (legacyHits.get(key) ?? 0) + 1);
    if ((legacyHits.get(key) ?? 0) > (LEGACY_SECRET_FLAG_BUDGET.get(key) ?? 0)) {
      pushViolation(violations, filePath, contents, index, `secret-bearing CLI flag --${flag}`);
    }
  };

  for (const { filePath, contents } of files) {
    LONG_OPTION.lastIndex = 0;
    for (const match of contents.matchAll(LONG_OPTION)) {
      recordFlag(filePath, contents, match.index, match[1]);
    }

    for (const match of contents.matchAll(ARG_BLOCK)) {
      if (EXPLICIT_LONG.test(match.groups.options) || !/\blong\b/u.test(match.groups.options)) continue;
      const inferred = normalizedFlag(match.groups.field);
      if (new RegExp(`^${SENSITIVE}$`, "iu").test(inferred)) {
        recordFlag(filePath, contents, match.index, inferred);
      }
    }

    for (const match of contents.matchAll(MANUAL_FLAG)) {
      recordFlag(filePath, contents, match.index, match[1]);
    }

    for (const [pattern, kind] of [
      [QUERY_CAPABILITY, "secret-bearing URL query"],
      [PATH_CAPABILITY, "secret-bearing URL path"],
      [USERINFO_CAPABILITY, "secret-bearing URL userinfo"],
      [DOCUMENTED_SECRET_ARG, "documented secret-bearing CLI argument"],
    ]) {
      pattern.lastIndex = 0;
      for (const match of contents.matchAll(pattern)) {
        pushViolation(violations, filePath, contents, match.index, kind);
      }
    }

    RAW_SECRET_DERIVED_DEBUG.lastIndex = 0;
    for (const match of contents.matchAll(RAW_SECRET_DERIVED_DEBUG)) {
      pushViolation(
        violations,
        filePath,
        contents,
        match.index,
        `derived Debug on secret-bearing DTO ${match.groups.type}`,
      );
    }

    PROVIDER_CONFORMANCE_ALL_HEADERS.lastIndex = 0;
    for (const match of contents.matchAll(PROVIDER_CONFORMANCE_ALL_HEADERS)) {
      pushViolation(
        violations,
        filePath,
        contents,
        match.index,
        "provider conformance copies credential-bearing headers",
      );
    }

    if (PRODUCTION_WEBSOCKET_URL_FILES.has(filePath)) {
      WEBSOCKET_SECRET_QUERY.lastIndex = 0;
      for (const match of contents.matchAll(WEBSOCKET_SECRET_QUERY)) {
        pushViolation(
          violations,
          filePath,
          contents,
          match.index,
          "secret-bearing production websocket URL query",
        );
      }
    }

    DEVELOPMENT_GATEWAY_SECRET_CONSTRUCTION.lastIndex = 0;
    if (filePath !== GATEWAY_SECRET_RESOLVER) {
      for (const match of contents.matchAll(DEVELOPMENT_GATEWAY_SECRET_CONSTRUCTION)) {
        pushViolation(
          violations,
          filePath,
          contents,
          match.index,
          "ungated development gateway secret material",
        );
      }
    }

    if (DEFERRED_GATEWAY_SECRET_CONFIG_FILES.has(filePath)) {
      const eagerResolution = contents.indexOf("resolver.resolve(");
      if (eagerResolution !== -1) {
        violations.push({
          filePath,
          line: lineNumber(contents, eagerResolution),
          kind: "eager outbound gateway secret resolution",
          snippet: "resolver.resolve(",
        });
      }
    }

    const requiredBoundarySnippets = RUNTIME_GATEWAY_SECRET_BOUNDARIES.get(filePath);
    if (requiredBoundarySnippets) {
      for (const snippet of requiredBoundarySnippets) {
        if (!contents.includes(snippet)) {
          violations.push({
            filePath,
            line: 1,
            kind: "runtime gateway secret boundary",
            snippet: `missing required boundary '${snippet}'`,
          });
        }
      }
      RAW_RUNTIME_GATEWAY_SECRET.lastIndex = 0;
      for (const match of contents.matchAll(RAW_RUNTIME_GATEWAY_SECRET)) {
        violations.push({
          filePath,
          line: lineNumber(contents, match.index),
          kind: "cloneable raw runtime gateway secret",
          snippet: match[0],
        });
      }
    }
  }

  return violations;
}

function selfTest() {
  const safe = validateFiles([
    {
      filePath: "src/safe.rs",
      contents: 'let header = format!("Authorization: Bearer {token}");\nlet url = "/expose#bootstrap=opaque";',
    },
    {
      filePath: "crates/example/tests/redaction.rs",
      contents: 'let fixture = format!("https://example.test/run?token={secret}");',
    },
    {
      filePath: "crates/example/src/runtime_tests/nested_fixture.rs",
      contents: 'let fixture = format!("https://example.test/run?token={secret}");',
    },
    {
      filePath: "src/embedded.rs",
      contents: '#[cfg(test)]\nmod tests { fn redacts() { let _ = format!("/run/{capability}"); } }',
    },
  ]);
  assert.deepEqual(safe, []);

  for (const [contents, expected] of [
    ['#[derive(Args)] struct Bad { #[arg(long = "secret")] pub secret: String }', "CLI flag"],
    ['match arg { "--password" => take(), _ => {} }', "CLI flag"],
    ['let url = format!("https://api.invalid/run?token={token}");', "URL query"],
    ['let url = format!("https://api.invalid/run/{capability}");', "URL path"],
    ['let url = format!("postgres://user:{password}@db.invalid/app");', "URL userinfo"],
    ['prodex gateway --auth-token "$PRODEX_GATEWAY_TOKEN"', "CLI argument"],
  ]) {
    const hits = validateFiles([{ filePath: "src/bad.rs", contents }]);
    assert.equal(hits.length, 1, `${expected} fixture was not rejected`);
    assert.match(hits[0].kind, new RegExp(expected, "iu"));
  }

  for (const typeName of SECRET_BEARING_DEBUG_TYPES) {
    const hits = validateFiles([
      {
        filePath: "src/bad.rs",
        contents: `#[derive(Clone, Debug)] struct ${typeName} { value: String }`,
      },
    ]);
    assert.ok(
      hits.some((hit) => hit.kind.includes("derived Debug on secret-bearing DTO")),
      `${typeName} derived Debug fixture was not rejected`,
    );
  }

  const providerHeaderHits = validateFiles([
    {
      filePath:
        "crates/prodex-app/src/runtime_launch/proxy_startup/provider_bridge_conformance.rs",
      contents: "input.headers = request.headers.iter().cloned().collect();",
    },
  ]);
  assert.ok(
    providerHeaderHits.some((hit) => hit.kind.includes("credential-bearing headers")),
    "provider conformance all-header fixture was not rejected",
  );

  const websocketHits = validateFiles([
    {
      filePath:
        "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gemini_live/connection.rs",
      contents: 'let url = format!("{endpoint}{separator}key={api_key}");',
    },
  ]);
  assert.equal(websocketHits.length, 1);
  assert.ok(
    websocketHits.some((hit) => hit.kind.includes("websocket URL query")),
    "production websocket query fixture was not rejected",
  );

  const rawSnapshotHits = validateFiles([
    {
      filePath:
        "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_config.rs",
      contents:
        "http_bearer_token: Option<String>\nbearer_token: Option<RuntimeGatewaySecret>",
    },
  ]);
  assert.ok(
    rawSnapshotHits.some((hit) => hit.kind.includes("cloneable raw runtime gateway secret")),
    "raw runtime snapshot fixture was not rejected",
  );

  const eagerResolutionHits = validateFiles([
    {
      filePath:
        "crates/prodex-app/src/app_commands/runtime_launch/gateway_observability_config.rs",
      contents: "resolver.runtime_secret(\nresolver.resolve(",
    },
  ]);
  assert.ok(
    eagerResolutionHits.some((hit) => hit.kind.includes("eager outbound")),
    "eager outbound resolution fixture was not rejected",
  );
}

async function repositoryFiles() {
  const result = await git(
    ["ls-files", "--cached", "--others", "--exclude-standard", "--", "*.rs", "*.md", "*.html", "*.mjs", "*.js", "*.toml", "*.yaml", "*.yml"],
    { cwd: repoRoot },
  );
  const paths = result.stdout.split(/\r?\n/u).filter(Boolean).sort();
  const files = await Promise.all(
    paths.map(async (filePath) => ({
      filePath,
      contents: await fs.readFile(path.join(repoRoot, filePath), "utf8").catch((error) => {
        if (error.code === "ENOENT") return null;
        throw error;
      }),
    })),
  );
  return files.filter((file) => file.contents !== null);
}

async function main() {
  if (process.argv.includes("--self-test")) selfTest();
  const violations = validateFiles(await repositoryFiles());
  if (violations.length === 0) {
    process.stdout.write("secret boundary guard: ok\n");
    return;
  }
  process.stderr.write("secret boundary guard failed:\n");
  for (const hit of violations) {
    process.stderr.write(`  - ${hit.filePath}:${hit.line}: ${hit.kind}: ${hit.snippet}\n`);
  }
  process.exitCode = 1;
}

main().catch((error) => {
  process.stderr.write(`secret-boundary-guard: ${error.message}\n`);
  process.exitCode = 1;
});
