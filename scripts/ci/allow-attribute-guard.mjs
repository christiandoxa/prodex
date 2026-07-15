#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { git } from "./guard-common.mjs";
import { repoRoot } from "../npm/common.mjs";

export const ALLOW_ATTRIBUTE_CAPS = Object.freeze({
  dead_code: 10,
  "unused_imports": 8,
  "clippy::large_enum_variant": 5,
  "clippy::result_large_err": 2,
  "clippy::too_many_arguments": 30,
  "clippy::type_complexity": 1,
});

export const TEST_ONLY_DEAD_CODE_ALLOW_CAP = 34;

export const ALLOW_ATTRIBUTE_LOCATION_KEYS = Object.freeze([
  "dead_code|crates/prodex-app/src/runtime_anthropic.rs|pub(super) fn runtime_anthropic_error_message_from_parts(",
  "dead_code|crates/prodex-app/src/runtime_proxy/lifecycle.rs|pub(crate) fn try_acquire_runtime_proxy_active_request_slot(",
  "dead_code|crates/prodex-app/src/runtime_proxy/selection/policy.rs|pub(crate) fn new(",
  "dead_code|crates/prodex-app/src/runtime_proxy/selection_plan.rs|provider_priority: usize,",
  "dead_code|crates/prodex-app/src/runtime_proxy/selection_plan.rs|pub(crate) fn runtime_prompt_cache_affinity_sort_key(",
  "dead_code|crates/prodex-app/src/runtime_proxy/websocket_message/auth.rs|pub(in crate::runtime_proxy) fn runtime_profile_auth_summary_for_selection(",
  "dead_code|crates/prodex-app/src/runtime_launch/proxy_startup/deepseek_sse.rs|pub(super) fn new(",
  "dead_code|crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite.rs|impl RuntimeLocalRewriteRequestContext {",
  "dead_code|crates/prodex-app/src/runtime_launch/proxy_startup/provider_tools.rs|pub(super) fn runtime_provider_flatten_namespace_tool_name(namespace: &str, name: &str) -> String {",
  "dead_code|crates/prodex-app/src/lib.rs|mod runtime_kiro_acp;",
  "unused_imports|crates/prodex-app/src/runtime_anthropic.rs|pub(super) use anthropic::{",
  "unused_imports|crates/prodex-app/src/runtime_background/probe_refresh.rs|pub(crate) use queue::note_runtime_probe_refresh_progress;",
  "unused_imports|crates/prodex-app/src/runtime_background/probe_refresh.rs|pub(crate) use startup::runtime_profiles_needing_startup_probe_refresh;",
  "unused_imports|crates/prodex-app/src/runtime_claude/config.rs|pub(crate) use prodex_runtime_claude::{",
  "unused_imports|crates/prodex-app/src/runtime_claude/state_merge.rs|pub(crate) use prodex_runtime_claude::{",
  "unused_imports|crates/prodex-app/src/runtime_launch/proxy_startup/deepseek_rewrite.rs|pub(super) use self::request_validation::{",
  "unused_imports|crates/prodex-app/src/runtime_launch/proxy_startup/deepseek_rewrite.rs|pub(super) use super::deepseek_reasoning::{",
  "unused_imports|crates/prodex-app/src/runtime_proxy/smart_context/static_context.rs|pub(super) use self::sections::{",
  "clippy::large_enum_variant|crates/prodex-app/src/runtime_proxy_shared.rs|pub(super) enum RuntimeResponsesReply {",
  "clippy::large_enum_variant|crates/prodex-app/src/runtime_proxy_shared.rs|pub(super) enum RuntimeUpstreamFailureResponse {",
  "clippy::large_enum_variant|crates/prodex-app/src/runtime_proxy_shared.rs|pub(super) enum RuntimeWebsocketConnectResult {",
  "clippy::large_enum_variant|crates/prodex-runtime-anthropic/src/lib.rs|pub enum RuntimeResponsesReply {",
  "clippy::large_enum_variant|crates/prodex-runtime-state/src/background.rs|pub enum RuntimeStateSavePayload<S, Shared> {",
  "clippy::result_large_err|crates/prodex-app/src/runtime_proxy/lifecycle.rs|pub(crate) fn enqueue_runtime_proxy_long_lived_request_with_wait(",
  "clippy::result_large_err|crates/prodex-app/tests/support/main_internal/runtime_proxy_backend/websocket/handler/accepted.rs|pub(super) fn accept_runtime_proxy_backend_websocket(",
  "clippy::too_many_arguments|crates/prodex-app/src/runtime_launch/proxy_startup/deepseek_rewrite/response.rs|pub(in crate::runtime_launch::proxy_startup) fn runtime_deepseek_chat_buffered_response_parts(",
  "clippy::too_many_arguments|crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_kiro.rs|fn runtime_kiro_stream_notification(",
  "clippy::too_many_arguments|crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_kiro.rs|fn runtime_kiro_stream_tool_call(",
  "clippy::too_many_arguments|crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_kiro.rs|fn runtime_kiro_streaming_reader(",
  "clippy::too_many_arguments|crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_kiro.rs|fn runtime_kiro_streaming_worker(",
  "clippy::too_many_arguments|crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_response_copilot.rs|pub(super) fn respond_runtime_copilot_rewrite(",
  "clippy::too_many_arguments|crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_response_passthrough.rs|pub(super) fn respond_runtime_passthrough_rewrite(",
  "clippy::too_many_arguments|crates/prodex-app/src/runtime_launch/proxy_startup/provider_bridge_spend.rs|pub(super) fn runtime_provider_gateway_response_spend_event_from_tokens(",
  "clippy::too_many_arguments|crates/prodex-app/src/runtime_launch/proxy_startup/provider_bridge_spend.rs|pub(super) fn runtime_provider_gateway_response_spend_event(",
  "clippy::too_many_arguments|crates/prodex-app/src/runtime_launch/proxy_startup/provider_bridge_spend.rs|pub(super) fn runtime_provider_gateway_spend_event(",
  "clippy::too_many_arguments|crates/prodex-domain/src/governance/approval.rs|pub fn pending(",
  "clippy::too_many_arguments|crates/prodex-provider-core/src/gemini_bridge/request.rs|pub fn gemini_provider_core_generate_content_request(",
  "clippy::too_many_arguments|crates/prodex-provider-core/src/translators/gemini/response/build.rs|pub(super) fn gemini_build_response_value(",
  "clippy::too_many_arguments|crates/prodex-provider-core/src/translators/kiro/acp.rs|pub fn kiro_provider_core_acp_metadata(",
  "clippy::too_many_arguments|crates/prodex-provider-core/src/translators/kiro/stream.rs|pub fn kiro_provider_core_acp_responses_tool_call_item(",
  "clippy::too_many_arguments|crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_application_data_plane.rs|fn runtime_gateway_mandatory_governance_audit(",
  "clippy::too_many_arguments|crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_application_data_plane.rs|fn runtime_gateway_obligation_execution(",
  "clippy::too_many_arguments|crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_policies.rs|fn activation_response(",
  "clippy::too_many_arguments|crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_policies.rs|fn vote_response(",
  "clippy::too_many_arguments|crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gemini_live/session.rs|fn runtime_gemini_live_drain_upstream<S>(",
  "clippy::too_many_arguments|crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_governance_session.rs|pub(super) fn remember(",
  "clippy::too_many_arguments|crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_response_dispatch.rs|pub(super) fn respond_runtime_local_rewrite_live_response(",
  "clippy::too_many_arguments|crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_tests/gateway_admin_policy_lifecycle.rs|fn activate(",
  "clippy::too_many_arguments|crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_tests/gateway_admin_policy_lifecycle.rs|fn vote(",
  "clippy::too_many_arguments|crates/prodex-app/src/runtime_proxy/presidio/http.rs|pub(crate) fn apply_runtime_presidio_redaction_to_request_with_rules(",
  "clippy::too_many_arguments|crates/prodex-app/src/runtime_proxy/presidio/websocket.rs|pub(crate) fn apply_runtime_presidio_redaction_to_websocket_text_with_rules<'a>(",
  "clippy::too_many_arguments|crates/prodex-storage-sqlite-runtime/src/governance_repository/revisions.rs|pub(super) fn store_pointer(",
  "clippy::too_many_arguments|crates/prodex-storage-sqlite-runtime/tests/governance_repository.rs|fn activation_request(",
  "clippy::too_many_arguments|crates/prodex-storage-sqlite-runtime/tests/governance_repository.rs|fn prepare_approval_for_existing(",
  "clippy::too_many_arguments|crates/prodex-storage-sqlite-runtime/tests/governance_repository.rs|fn prepare_approved_revision(",
  "clippy::type_complexity|crates/prodex-bench-support/src/lib.rs|pub fn run_runtime_proxy_hot_path_case_suite<",
]);

export const TEST_ONLY_DEAD_CODE_ALLOW_LOCATION_KEYS = Object.freeze([
  "crates/prodex-app/src/app_commands.rs|pub(crate) fn resolve_runtime_launch_profile_name(",
  "crates/prodex-app/src/app_commands/selection.rs|pub(crate) fn required_main_window_snapshot(",
  "crates/prodex-app/src/profile_commands/kiro.rs|pub(crate) fn read_kiro_auth_secret(codex_home: &Path) -> Result<KiroAuthSecret> {",
  "crates/prodex-app/src/runtime_anthropic.rs|pub(super) fn runtime_anthropic_sse_response_parts_from_responses_sse_bytes(",
  "crates/prodex-app/src/runtime_anthropic.rs|pub(super) fn runtime_anthropic_sse_response_parts_from_message_value(",
  "crates/prodex-app/src/runtime_anthropic.rs|pub(super) fn runtime_request_for_anthropic_server_tool_followup(",
  "crates/prodex-app/src/runtime_background/probe_refresh/worker.rs|pub(crate) fn runtime_probe_refresh_take_next_job(",
  "crates/prodex-app/src/runtime_background/scheduled_save.rs|pub(crate) fn runtime_state_save_debounce(reason: &str) -> Duration {",
  "crates/prodex-app/src/runtime_background/scheduled_save.rs|pub(crate) fn runtime_state_save_reason_requires_continuation_journal(reason: &str) -> bool {",
  "crates/prodex-app/src/runtime_background/scheduled_save.rs|pub(crate) fn runtime_state_save_sections_for_reason(reason: &str) -> RuntimeStateSaveSections {",
  "crates/prodex-app/src/runtime_persistence/continuation_store.rs|pub(crate) fn save_runtime_continuation_journal(",
  "crates/prodex-app/src/runtime_proxy/lineage/release.rs|pub(crate) fn clear_runtime_stale_previous_response_binding(",
  "crates/prodex-app/src/runtime_proxy/selection/policy.rs|pub(crate) fn runtime_candidate_no_rotate_affinity(",
  "crates/prodex-app/src/runtime_proxy/selection/policy.rs|pub(crate) fn runtime_quota_blocked_affinity_release_policy(",
  "crates/prodex-app/src/runtime_proxy/selection/policy.rs|pub(crate) struct RuntimeQuotaBlockedAffinityReleaseRequest<'a> {",
  "crates/prodex-app/src/runtime_proxy/websocket_message/auth.rs|pub(in crate::runtime_proxy) fn runtime_profile_auth_summary_for_selection_with_policy(",
  "crates/prodex-app/src/runtime_store/backoffs.rs|pub(crate) fn save_runtime_profile_backoffs(",
  "crates/prodex-app/src/runtime_store/scores.rs|pub(crate) fn save_runtime_profile_scores(",
  "crates/prodex-app/src/runtime_store/usage_snapshots.rs|pub(crate) fn save_runtime_usage_snapshots(",
  "crates/prodex-runtime-anthropic/src/output.rs|pub fn runtime_anthropic_response_from_json_value(",
  "crates/prodex-runtime-anthropic/src/output/buffered_sse.rs|pub fn runtime_anthropic_response_from_sse_bytes(",
  "crates/prodex-secret-store/src/locations.rs|pub fn auth_json_keyring_account(codex_home: impl AsRef<Path>) -> String {",
  "crates/prodex-secret-store/src/locations.rs|pub fn auth_json_location_for_backend(",
  "crates/prodex-secret-store/src/locations.rs|pub fn describe_secret_location(location: &SecretLocation) -> String {",
  "crates/prodex-secret-store/src/model.rs|Keyring {",
  "crates/prodex-secret-store/src/model.rs|fn delete(&self, location: &SecretLocation) -> Result<(), SecretError>;",
  "crates/prodex-secret-store/src/model.rs|pub fn bytes(value: impl Into<Vec<u8>>) -> Self {",
  "crates/prodex-secret-store/src/model.rs|pub fn delete(&self, location: &SecretLocation) -> Result<(), SecretError> {",
  "crates/prodex-secret-store/src/model.rs|pub fn keyring(service: impl Into<String>, account: impl Into<String>) -> Self {",
  "crates/prodex-secret-store/src/model.rs|pub fn modified_at(&self) -> Option<SystemTime> {",
  "crates/prodex-secret-store/src/model.rs|pub fn read(&self, location: &SecretLocation) -> Result<Option<SecretValue>, SecretError> {",
  "crates/prodex-secret-store/src/model.rs|pub fn size_bytes(&self) -> u64 {",
  "crates/prodex-secret-store/src/model.rs|pub fn write(&self, location: &SecretLocation, value: SecretValue) -> Result<(), SecretError> {",
  "crates/prodex-secret-store/src/selection.rs|pub fn into_manager(self) -> SecretManager<Self> {",
]);

function parseArgs(argv) {
  const args = { json: false };
  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--json") {
      args.json = true;
      continue;
    }
    if (value === "--help" || value === "-h") {
      args.help = true;
      continue;
    }
    throw new Error(`unknown argument: ${value}`);
  }
  return args;
}

function printHelp() {
  process.stdout.write(
    [
      "Usage: node scripts/ci/allow-attribute-guard.mjs [--json]",
      "",
      "Fails when Rust #[allow(...)] or test-only dead_code allowances drift.",
      "Lower caps and remove allowlist entries when refactors remove lint allowances.",
    ].join("\n") + "\n",
  );
}

async function rustFiles() {
  const result = await git(
    ["ls-files", "--cached", "--others", "--exclude-standard", "--", "*.rs"],
    { cwd: repoRoot },
  );
  return result.stdout.split(/\r?\n/).filter(Boolean).sort();
}

const ALLOW_ATTRIBUTE_PATTERN = /#\s*\[\s*allow\s*\(([^)]*)\)\s*\]/g;
const TEST_ONLY_DEAD_CODE_ALLOW_PATTERN =
  /#\s*\[\s*cfg_attr\s*\(\s*not\s*\(\s*test\s*\)\s*,\s*allow\s*\(\s*dead_code\s*\)\s*\)\s*\]/;

function nextRustItemSignature(lines, attributeLineIndex) {
  for (let index = attributeLineIndex + 1; index < lines.length; index += 1) {
    const line = lines[index].trim();
    if (!line || line.startsWith("#[")) {
      continue;
    }
    return line;
  }
  return "";
}

function allowLocationKey(occurrence) {
  return `${occurrence.name}|${occurrence.filePath}|${occurrence.signature}`;
}

function testOnlyDeadCodeAllowLocationKey(occurrence) {
  return `${occurrence.filePath}|${occurrence.signature}`;
}

function scanRustFile(filePath, contents) {
  const lines = contents.split(/\r?\n/);
  const allowOccurrences = [];
  const testOnlyDeadCodeAllowOccurrences = [];

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    const lineNumber = index + 1;
    for (const match of line.matchAll(ALLOW_ATTRIBUTE_PATTERN)) {
      for (const rawName of match[1].split(",")) {
        const name = rawName.trim();
        if (!name) {
          continue;
        }
        allowOccurrences.push({
          filePath,
          line: lineNumber,
          name,
          signature: nextRustItemSignature(lines, index),
        });
      }
    }
    if (TEST_ONLY_DEAD_CODE_ALLOW_PATTERN.test(line)) {
      testOnlyDeadCodeAllowOccurrences.push({
        filePath,
        line: lineNumber,
        signature: nextRustItemSignature(lines, index),
      });
    }
  }

  return { allowOccurrences, testOnlyDeadCodeAllowOccurrences };
}

function normalizePolicy(policy = {}) {
  return {
    allowAttributeCaps: policy.allowAttributeCaps ?? ALLOW_ATTRIBUTE_CAPS,
    allowAttributeLocationKeys:
      policy.allowAttributeLocationKeys ?? ALLOW_ATTRIBUTE_LOCATION_KEYS,
    testOnlyDeadCodeAllowCap:
      policy.testOnlyDeadCodeAllowCap ?? TEST_ONLY_DEAD_CODE_ALLOW_CAP,
    testOnlyDeadCodeAllowLocationKeys:
      policy.testOnlyDeadCodeAllowLocationKeys ?? TEST_ONLY_DEAD_CODE_ALLOW_LOCATION_KEYS,
  };
}

function locationViolation(type, occurrence, key) {
  return {
    type,
    key,
    name: occurrence.name,
    filePath: occurrence.filePath,
    line: occurrence.line,
    signature: occurrence.signature,
  };
}

function staleLocationViolation(type, key) {
  return { type, key };
}

export function scanRustFileEntries(entries, policyOverrides = {}) {
  const policy = normalizePolicy(policyOverrides);
  const counts = new Map();
  const allowOccurrences = [];
  const testOnlyDeadCodeAllowOccurrences = [];

  for (const [filePath, contents] of entries) {
    const fileReport = scanRustFile(filePath, contents);
    allowOccurrences.push(...fileReport.allowOccurrences);
    testOnlyDeadCodeAllowOccurrences.push(...fileReport.testOnlyDeadCodeAllowOccurrences);
  }

  for (const occurrence of allowOccurrences) {
    counts.set(occurrence.name, (counts.get(occurrence.name) ?? 0) + 1);
  }

  const violations = [];
  for (const [name, count] of [...counts.entries()].sort()) {
    const cap = policy.allowAttributeCaps[name];
    if (cap === undefined) {
      violations.push({ name, count, cap: 0, type: "uncapped-allow" });
      continue;
    }
    if (count > cap) {
      violations.push({ name, count, cap, type: "cap-exceeded" });
    } else if (count < cap) {
      violations.push({ name, count, cap, type: "cap-not-ratcheted" });
    }
  }
  for (const [name, cap] of Object.entries(policy.allowAttributeCaps)) {
    if (!counts.has(name)) {
      counts.set(name, 0);
      if (cap > 0) {
        violations.push({ name, count: 0, cap, type: "cap-not-ratcheted" });
      }
    }
  }

  const allowedLocationKeys = new Set(policy.allowAttributeLocationKeys);
  const seenLocationKeys = new Set();
  for (const occurrence of allowOccurrences) {
    const key = allowLocationKey(occurrence);
    seenLocationKeys.add(key);
    if (!allowedLocationKeys.has(key)) {
      violations.push(locationViolation("unlisted-allow-location", occurrence, key));
    }
  }
  for (const key of allowedLocationKeys) {
    if (!seenLocationKeys.has(key)) {
      violations.push(staleLocationViolation("stale-allow-location", key));
    }
  }

  const testOnlyDeadCodeAllowCount = testOnlyDeadCodeAllowOccurrences.length;
  if (testOnlyDeadCodeAllowCount > policy.testOnlyDeadCodeAllowCap) {
    violations.push({
      name: "cfg_attr(not(test), allow(dead_code))",
      count: testOnlyDeadCodeAllowCount,
      cap: policy.testOnlyDeadCodeAllowCap,
      type: "test-only-dead-code-cap-exceeded",
    });
  } else if (testOnlyDeadCodeAllowCount < policy.testOnlyDeadCodeAllowCap) {
    violations.push({
      name: "cfg_attr(not(test), allow(dead_code))",
      count: testOnlyDeadCodeAllowCount,
      cap: policy.testOnlyDeadCodeAllowCap,
      type: "test-only-dead-code-cap-not-ratcheted",
    });
  }

  const allowedTestOnlyLocationKeys = new Set(policy.testOnlyDeadCodeAllowLocationKeys);
  const seenTestOnlyLocationKeys = new Set();
  for (const occurrence of testOnlyDeadCodeAllowOccurrences) {
    const key = testOnlyDeadCodeAllowLocationKey(occurrence);
    seenTestOnlyLocationKeys.add(key);
    if (!allowedTestOnlyLocationKeys.has(key)) {
      violations.push(locationViolation("unlisted-test-only-dead-code-allow", occurrence, key));
    }
  }
  for (const key of allowedTestOnlyLocationKeys) {
    if (!seenTestOnlyLocationKeys.has(key)) {
      violations.push(staleLocationViolation("stale-test-only-dead-code-allow", key));
    }
  }

  return {
    caps: policy.allowAttributeCaps,
    counts: Object.fromEntries([...counts.entries()].sort()),
    testOnlyDeadCodeAllow: {
      cap: policy.testOnlyDeadCodeAllowCap,
      count: testOnlyDeadCodeAllowCount,
    },
    violations: violations.sort((left, right) => {
      const leftKey = `${left.type}|${left.name ?? ""}|${left.key ?? ""}`;
      const rightKey = `${right.type}|${right.name ?? ""}|${right.key ?? ""}`;
      return leftKey.localeCompare(rightKey);
    }),
  };
}

export async function scan() {
  const entries = [];
  for (const filePath of await rustFiles()) {
    let contents;
    try {
      contents = await fs.readFile(path.join(repoRoot, filePath), "utf8");
    } catch (error) {
      if (error?.code === "ENOENT") continue;
      throw error;
    }
    entries.push([filePath, contents]);
  }
  return scanRustFileEntries(entries);
}

function printHuman(report) {
  if (report.violations.length === 0) {
    process.stdout.write(
      `allow attribute guard: ok (${Object.keys(report.counts).length} allow kind(s), ${report.testOnlyDeadCodeAllow.count} test-only dead_code allow(s))\n`,
    );
    return;
  }

  process.stderr.write("allow attribute guard failed:\n");
  for (const violation of report.violations) {
    if (violation.key) {
      process.stderr.write(`  - ${violation.type}: ${violation.key}\n`);
      continue;
    }
    process.stderr.write(`  - ${violation.name}: ${violation.count} / ${violation.cap} (${violation.type})\n`);
  }
  process.stderr.write("\nRemove the allowance, or deliberately update the cap and allowlist with rationale.\n");
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }
  const report = await scan();
  if (args.json) {
    process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
  } else {
    printHuman(report);
  }
  if (report.violations.length > 0) {
    process.exitCode = 1;
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    process.stderr.write(`allow-attribute-guard: ${error.message}\n`);
    process.exitCode = 1;
  });
}
