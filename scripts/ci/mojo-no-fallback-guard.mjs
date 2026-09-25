#!/usr/bin/env node

import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import { repoRoot } from "../npm/common.mjs";

const PROMOTED_FILES = [
  "crates/prodex-mojo-core/build.rs",
  "crates/prodex-mojo-core/src/lib.rs",
  "crates/prodex-mojo-core/src/quota.rs",
  "crates/prodex-mojo-core/src/routing.rs",
  "crates/prodex-mojo-core/src/runtime.rs",
  "crates/prodex-mojo-core/src/runtime/auto_redeem.rs",
  "crates/prodex-mojo-core/src/runtime_decisions.rs",
  "crates/prodex-mojo-core/src/provider_constraints.rs",
  "crates/prodex-mojo-core/src/policy.rs",
  "crates/prodex-mojo-core/src/context.rs",
  "crates/prodex-mojo-core/src/rich.rs",
  "crates/prodex-mojo-core/src/rich/catalog.rs",
  "crates/prodex-mojo-core/src/rich/catalog_planner.rs",
  "crates/prodex-mojo-core/src/rich/context_plan.rs",
  "crates/prodex-mojo-core/src/log.rs",
  "crates/prodex-mojo-core/src/rich/routing.rs",
  "crates/prodex-context/src/critical_signal.rs",
  "crates/prodex-quota/src/render/gemini.rs",
  "crates/prodex-runtime-proxy/src/mojo.rs",
  "crates/prodex-runtime-proxy/src/quota.rs",
  "crates/prodex-runtime-proxy/src/selection_plan.rs",
  "crates/prodex-runtime-proxy/src/smart_context/token_accounting.rs",
  "crates/prodex-runtime-proxy/src/smart_context/token_accounting/observed.rs",
  "crates/prodex-runtime-proxy/src/smart_context/token_accounting/estimation.rs",
  "crates/prodex-runtime-quota/src/selection/scoring.rs",
  "crates/prodex-runtime-quota/src/selection/scoring/profile_order.rs",
  "crates/prodex-runtime-policy/src/types/runtime_proxy_preset.rs",
  "crates/prodex-observability/src/lib.rs",
  "crates/prodex-observability/src/mojo.rs",
  "crates/prodex-runtime-tuning/src/lib.rs",
  "crates/prodex-runtime-tuning/src/mojo.rs",
  "crates/prodex-provider-core/src/fallback/chains.rs",
  "crates/prodex-provider-core/src/catalog.rs",
  "crates/prodex-provider-core/src/models.rs",
  "crates/prodex-provider-core/src/translators/anthropic/messages.rs",
  "crates/prodex-provider-core/src/translators/anthropic/messages/response.rs",
  "crates/prodex-runtime-quota/src/pressure.rs",
  "crates/prodex-runtime-store/src/continuations/status.rs",
  "crates/prodex-runtime-store/src/continuations/status/mojo.rs",
  "crates/prodex-runtime-store/src/profile_backoff/backoff.rs",
  "crates/prodex-runtime-proxy/src/health/backoff.rs",
  "crates/prodex-runtime-proxy/src/health/score.rs",
  "crates/prodex-runtime-proxy/src/health/latency.rs",
  "crates/prodex-runtime-proxy/src/health/inflight.rs",
  "crates/prodex-runtime-proxy/src/health/health_decisions.rs",
];

const UNCONDITIONAL_MOJO_FILES = new Set([
  "crates/prodex-runtime-quota/src/selection/scoring.rs",
  "crates/prodex-runtime-quota/src/selection/scoring/profile_order.rs",
]);
const FEATURE_OFF_RUST_PATH = /\bnot\s*\(\s*feature\s*=\s*"mojo"\s*\)/u;
const ANTHROPIC_RESPONSE_FILE = "crates/prodex-provider-core/src/translators/anthropic/messages/response.rs";
const ANTHROPIC_MESSAGES_FILE = "crates/prodex-provider-core/src/translators/anthropic/messages.rs";
const ANTHROPIC_RESPONSE_FORBIDDEN_PATTERNS = [
  [/\bfn\s+anthropic_response_block_input\s*\(/u, "Rust response block classifier"],
  [/\bfn\s+plan_with_rust\s*\(/u, "Rust response planner"],
  [/\b(?:Some\s*\(\s*)?"(?:text|tool_use|server_tool_use|web_search_tool_result|thinking)"(?:\s*\))?\s*=>/u,
    "Rust response block classification"],
  [/\bResponsePlanItem\s*\{\s*kind\s*:\s*ResponsePlanKind\s*::/u,
    "Rust response plan construction", true],
  [FEATURE_OFF_RUST_PATH, "feature-off Rust response path"],
];

const FORBIDDEN_MARKERS = [
  "prodex_mojo_fallback",
  "use_rust_fallback",
  "rust_fallback",
  "fallback-to-rust",
];

export function findViolations(files) {
  const markerViolations = files.flatMap(([filePath, contents]) =>
    FORBIDDEN_MARKERS.filter((marker) => contents.includes(marker)).map(
      (marker) => `${filePath}: promoted Mojo code contains ${marker}`,
    ),
  );
  const featureOffViolations = files
    .filter(([filePath, contents]) =>
      UNCONDITIONAL_MOJO_FILES.has(filePath) && FEATURE_OFF_RUST_PATH.test(contents),
    )
    .map(([filePath]) => `${filePath}: Mojo-owned quota scoring cannot have a feature-off Rust path`);
  const anthropicResponseViolations = files.flatMap(([filePath, contents]) =>
    filePath !== ANTHROPIC_RESPONSE_FILE
      ? []
      : ANTHROPIC_RESPONSE_FORBIDDEN_PATTERNS
        .filter(([pattern, , productionOnly]) => pattern.test(
          productionOnly ? contents.split("#[cfg(test)]", 1)[0] : contents,
        ))
        .map(([, reason]) => `${filePath}: contains ${reason}`),
  );
  const anthropicEnvelopeViolations = files
    .filter(([filePath, contents]) => filePath === ANTHROPIC_MESSAGES_FILE &&
      /\bfn\s+(?:anthropic_response_envelope_rust|anthropic_usage)\s*\(/u.test(contents))
    .map(([filePath]) => `${filePath}: contains a Rust response envelope implementation`);
  return [...markerViolations, ...featureOffViolations, ...anthropicResponseViolations,
    ...anthropicEnvelopeViolations];
}

async function promotedFiles() {
  return Promise.all(
    PROMOTED_FILES.map(async (filePath) => [
      filePath,
      await fs.readFile(path.join(repoRoot, filePath), "utf8"),
    ]),
  );
}

function selfTest() {
  assert.deepEqual(findViolations([["x.rs", "fn main() {}"]]), []);
  assert.equal(findViolations([["x.rs", "prodex_mojo_fallback();"]]).length, 1);
  assert.equal(
    findViolations([[
      "crates/prodex-runtime-quota/src/selection/scoring/profile_order.rs",
      '#[cfg(not(feature = "mojo"))] fn rust_order() {}',
    ]]).length,
    1,
  );
  const responseViolations = (contents) => findViolations([[ANTHROPIC_RESPONSE_FILE, contents]]);
  assert.deepEqual(responseViolations(`
    fn response_plan_with_mojo() {
      let kind = classified.kind;
      ResponsePlanItem { kind: match item.kind { _ => ResponsePlanKind::Message } }
    }
  `), []);
  assert.deepEqual(responseViolations(
    "#[cfg(test)] mod tests { assert_eq!(plan, ResponsePlanItem { kind: ResponsePlanKind::Message }); }",
  ), []);
  assert.match(responseViolations("fn anthropic_response_block_input() {}")[0], /Rust response block classifier/u);
  assert.match(responseViolations("fn plan_with_rust() {}")[0], /Rust response planner/u);
  assert.match(responseViolations("#[cfg(test)] fn plan_with_rust() {}")[0], /Rust response planner/u);
  assert.match(responseViolations('match kind { Some("tool_use") => (), _ => () }')[0],
    /Rust response block classification/u);
  assert.match(responseViolations("ResponsePlanItem { kind: ResponsePlanKind::Message }")[0],
    /Rust response plan construction/u);
  assert.match(responseViolations('#[cfg(not(feature = "mojo"))] fn fallback() {}')[0],
    /feature-off Rust response path/u);
  assert.match(findViolations([[ANTHROPIC_MESSAGES_FILE,
    "fn anthropic_response_envelope_rust() {}"]])[0], /Rust response envelope/u);
  for (const filePath of [
    "crates/prodex-provider-core/src/translators/anthropic/messages.rs",
    "crates/prodex-provider-core/src/translators/anthropic/messages/stream.rs",
  ]) {
    assert.deepEqual(findViolations([[
      filePath,
      '#[cfg(not(feature = "mojo"))] fn existing_path() { Some("text") => () }',
    ]]), []);
  }
}

async function main() {
  if (process.argv.includes("--self-test")) selfTest();
  const violations = findViolations(await promotedFiles());
  if (violations.length > 0) throw new Error(violations.join("\n"));
  process.stdout.write("mojo no-fallback guard: ok\n");
}

main().catch((error) => {
  process.stderr.write(`mojo no-fallback guard: ${error.message}\n`);
  process.exitCode = 1;
});
