#!/usr/bin/env node
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import { repoRoot } from "../npm/common.mjs";

const paths = {
  rollout: "crates/prodex-runtime-proxy/src/smart_context/rollout.rs",
  hash: "crates/prodex-runtime-proxy/src/smart_context/normalization/artifacts.rs",
  adapter: "crates/prodex-app/src/runtime_proxy/smart_context.rs",
  sticky: "crates/prodex-app/src/runtime_proxy/smart_context/rollout.rs",
  body: "crates/prodex-app/src/runtime_proxy/smart_context/body.rs",
  transform: "crates/prodex-app/src/runtime_proxy/smart_context/body/transform.rs",
  artifact: "crates/prodex-app/src/runtime_state_shared/artifact_store/content.rs",
  manifest: "crates/prodex-app/src/runtime_proxy/smart_context/artifact_manifest.rs",
  corpus: "crates/prodex-runtime-proxy/tests/fixtures/smart_context_replay_corpus.json",
};

function section(source, start, end) {
  const from = source.indexOf(start);
  const to = source.indexOf(end, from + start.length);
  return from >= 0 && to > from ? source.slice(from, to) : "";
}

export function validateSmartContext(sources) {
  const errors = [];
  const rollout = sources[paths.rollout] ?? "";
  const bucket = section(rollout, "pub fn smart_context_rollout_bucket", "}");
  if (!bucket.includes("smart_context_sha256_digest") || !bucket.includes("% 10_000")) {
    errors.push("rollout bucket must use binary SHA-256 bytes and basis-point buckets");
  }
  for (const forbidden of ["parse::<", "unwrap_or(0)"]) {
    if (bucket.includes(forbidden)) errors.push(`rollout bucket contains ${forbidden}`);
  }

  const sticky = section(
    sources[paths.sticky] ?? "",
    "pub(super) fn runtime_smart_context_rollout_stable_key",
    "fn runtime_smart_context_rollout_scope_hash",
  );
  if (!sticky.includes("session_id") || !sticky.includes("workspace") || !sticky.includes("profile_name")) {
    errors.push("rollout stable key must bind session, workspace, and profile scope");
  }
  for (const forbidden of ["request_id", "SystemTime", "getrandom", "Uuid"]) {
    if (sticky.includes(forbidden)) errors.push(`rollout stable key contains entropy source ${forbidden}`);
  }

  const adapter = sources[paths.adapter] ?? "";
  const prepare = section(
    adapter,
    "fn prepare_runtime_smart_context_body_safely",
    "fn runtime_smart_context_exact_passthrough",
  );
  const exactIndex = prepare.indexOf("runtime_smart_context_exact_passthrough(request)");
  const enabledIndex = prepare.indexOf("runtime_smart_context_enabled(shared)");
  if (exactIndex < 0 || enabledIndex < 0 || exactIndex > enabledIndex) {
    errors.push("explicit exact mode must return before state lookup or rollout work");
  }
  if (adapter.includes("OnceLock<Mutex<BTreeMap<PathBuf, RuntimeSmartContextProxyState")) {
    errors.push("process-global Smart Context proxy-state registry is forbidden");
  }

  const body = sources[paths.body] ?? "";
  const shadowIndex = body.indexOf("if shadow {");
  const observeIndex = body.lastIndexOf("observe_runtime_smart_context_rewrite_safety_with_state(");
  const commitIndex = body.lastIndexOf("commit_runtime_smart_context_proxy_state_for_scope(");
  const fallbackIndex = body.lastIndexOf("runtime_smart_context_fallback_exact_reason(");
  if (!(shadowIndex > fallbackIndex && shadowIndex < observeIndex && observeIndex < commitIndex)) {
    errors.push("shadow must return after validation and before any live-state commit");
  }

  const transform = sources[paths.transform] ?? "";
  if (!transform.includes("runtime_smart_context_append_inline_reference_protocol") || transform.includes("insert_text")) {
    errors.push("active rewrites must use resolvable inline references, not pending artifacts");
  }

  const artifact = sources[paths.artifact] ?? "";
  const insert = section(artifact, "pub(crate) fn insert_text", "pub(crate) fn get_text");
  if (!insert.includes("next_artifact_order") || /request_id|\.sequence/u.test(insert)) {
    errors.push("artifact ordering must use its persisted order counter, never request IDs");
  }

  const hash = sources[paths.hash] ?? "";
  const identity = section(hash, "pub fn smart_context_hash_text", "pub fn smart_context_hash_matches_text");
  if (!identity.includes("sc2:") || !identity.includes("smart_context_sha256_hex") || identity.includes("fnv")) {
    errors.push("correctness-critical artifact identity must be versioned SHA-256");
  }

  const manifest = sources[paths.manifest] ?? "";
  if (/"role"\s*:\s*"user"|role\s*[:=]\s*"user"/u.test(manifest)) {
    errors.push("Smart Context must not synthesize a user message");
  }

  const corpus = JSON.parse(sources[paths.corpus] ?? "{}");
  const forbiddenCorpusKeys = new Set([
    "input_tokens",
    "success",
    "integrity_percent",
    "fallback_count",
    "rewrite_latency",
  ]);
  const visit = (value) => {
    if (Array.isArray(value)) value.forEach(visit);
    else if (value && typeof value === "object") {
      for (const [key, child] of Object.entries(value)) {
        if (forbiddenCorpusKeys.has(key)) errors.push(`replay corpus contains generated field ${key}`);
        visit(child);
      }
    }
  };
  visit(corpus);
  return errors;
}

function selfTest() {
  const valid = Object.fromEntries(Object.values(paths).map((file) => [file, fs.readFileSync(path.join(repoRoot, file), "utf8")]));
  assert.deepEqual(validateSmartContext(valid), []);
  const broken = { ...valid, [paths.rollout]: "pub fn smart_context_rollout_bucket(_: &str) -> u16 { \"sc:x\".parse::<u16>().unwrap_or(0) }" };
  assert(validateSmartContext(broken).some((error) => error.includes("rollout bucket")));
  assert(
    validateSmartContext({ ...valid, [paths.manifest]: 'json!({"role": "user"})' }).some((error) =>
      error.includes("synthesize a user"),
    ),
  );
}

if (process.argv.includes("--self-test")) selfTest();
const sources = Object.fromEntries(
  Object.values(paths).map((file) => [file, fs.readFileSync(path.join(repoRoot, file), "utf8")]),
);
const errors = validateSmartContext(sources);
if (errors.length > 0) {
  throw new Error(errors.join("\n"));
}
process.stdout.write("Smart Context guard: ok\n");
