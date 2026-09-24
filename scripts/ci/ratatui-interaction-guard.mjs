#!/usr/bin/env node

import fs from "node:fs";
import { execFileSync } from "node:child_process";

const ALLOWED_DIRECT_OUTPUT = new Map([
  [
    "crates/prodex-app/src/app_commands/log_upstream.rs",
    [
      "Waiting for processed upstream payload events",
      "serde_json::to_string(event)?",
      "println!(\"{line}\")",
    ],
  ],
  [
    "crates/prodex-app/src/app_commands/info_handler.rs",
    ["serde_json::to_string_pretty(&value)?"],
  ],
  [
    "crates/prodex-app/src/app_commands/log_stream.rs",
    [
      "log_stream_item_json(event)?",
      "serde_json::to_string(event)?",
      "println!(\"{line}\")",
    ],
  ],
  [
    "crates/prodex-app/src/app_commands/runtime_launch/session_delete.rs",
    ["prodex_runtime_timing stage=shutdown.session_maintenance_error"],
  ],
  [
    "crates/prodex-app/src/quota_support/watch_tui/runtime_profile.rs",
    ["quota_watch_tui_fallback_message(&err)"],
  ],
  [
    "crates/prodex-app/src/quota_support/watch_tui/runtime_all.rs",
    ["quota_watch_tui_fallback_message(&err)"],
  ],
  [
    "crates/prodex-app/src/runtime_launch/execution.rs",
    ["prodex_runtime_timing stage={stage} duration_ms="],
  ],
]);

function productionRustFiles() {
  return execFileSync("git", ["ls-files", "crates/prodex-app/src/**/*.rs", "crates/prodex-app/src/*.rs"], {
    encoding: "utf8",
  })
    .split(/\r?\n/u)
    .filter(Boolean)
    .filter((file) => !/(?:^|\/)(?:tests?|fixtures?|snapshots)(?:\/|$)/u.test(file))
    .filter((file) => !/(?:^|\/)[^/]*(?:_tests?|tests?)\.rs$/u.test(file));
}

function directOutputSites(file, source) {
  const lines = source.replace(/\r\n?/gu, "\n").split("\n");
  const sites = [];
  for (let index = 0; index < lines.length; index += 1) {
    if (!/\b(?:println|eprintln)!\s*\(/u.test(lines[index])) continue;
    sites.push({
      file,
      line: index + 1,
      context: lines.slice(index, index + 7).join("\n"),
      source: lines[index].trim(),
    });
  }
  return sites;
}

function allowed(site) {
  const markers = ALLOWED_DIRECT_OUTPUT.get(site.file) ?? [];
  return markers.some((marker) => site.context.includes(marker));
}

function selfTest() {
  const forbidden = directOutputSites(
    "crates/prodex-app/src/example.rs",
    "fn main() { println!(\"human status\"); }\n",
  );
  if (forbidden.length !== 1 || allowed(forbidden[0])) {
    throw new Error("ratatui interaction guard self-test failed to reject direct human output");
  }
  const permitted = directOutputSites(
    "crates/prodex-app/src/app_commands/log_upstream.rs",
    "fn emit(event: &Event) { println!(\"{}\", serde_json::to_string(event)?); }\n",
  );
  if (permitted.length !== 1 || !allowed(permitted[0])) {
    throw new Error("ratatui interaction guard self-test failed to permit machine JSON output");
  }
}

selfTest();

const files = productionRustFiles();
const violations = [];
for (const file of files) {
  const source = fs.readFileSync(file, "utf8");
  for (const site of directOutputSites(file, source)) {
    if (!allowed(site)) violations.push(site);
  }
}

if (violations.length > 0) {
  process.stderr.write("ratatui interaction guard: " + violations.length + " violation(s)\n");
  for (const violation of violations) {
    process.stderr.write(
      violation.file + ":" + violation.line + ": direct " + violation.source + "\n",
    );
  }
  process.stderr.write(
    "\nInteractive human-facing Prodex output must use Ratatui. Keep direct println!/eprintln! only for explicit machine-readable streams, telemetry diagnostics, or documented TUI failure fallbacks.\n",
  );
  process.exitCode = 1;
} else {
  process.stdout.write(
    "ratatui interaction guard: ok (" + files.length + " production Rust file(s))\n",
  );
}
