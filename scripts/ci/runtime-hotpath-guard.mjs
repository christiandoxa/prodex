#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { repoRoot } from "../npm/common.mjs";

const DEFAULT_SCAN_TARGETS = Object.freeze([
  "crates/prodex-app/src/runtime_proxy",
  "crates/prodex-app/src/runtime_launch/proxy_startup.rs",
  "crates/prodex-app/src/runtime_launch/proxy_startup",
  "crates/prodex-runtime-proxy/src",
]);

const DEFAULT_EXCLUDED_PATH_PREFIXES = Object.freeze([
  "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_tests.rs",
  "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_tests/",
]);

const FORBIDDEN_PATTERNS = Object.freeze([
  {
    id: "blocking-disk-io",
    description: "blocking std::fs/fs:: disk operation in runtime hot path",
    pattern:
      /\b(?:std::fs|fs)::(?:read|read_to_string|write|create_dir_all|remove_file|remove_dir|remove_dir_all|rename|copy|canonicalize|read_dir|metadata|symlink_metadata|set_permissions)\s*\(/,
  },
  {
    id: "blocking-file-open",
    description: "blocking std::fs/fs:: file open in runtime hot path",
    pattern: /\b(?:std::fs|fs)::(?:File|OpenOptions)\b/,
  },
  {
    id: "blocking-thread-spawn",
    description: "OS thread spawn in runtime hot path",
    pattern: /\b(?:std::thread|thread)::spawn\s*\(/,
  },
  {
    id: "blocking-thread-builder-spawn",
    description: "OS thread Builder spawn in runtime hot path",
    pattern:
      /\b(?:std::thread|thread)::Builder\s*::\s*new\s*\(\s*\)(?:\s*\.\s*[A-Za-z_][A-Za-z0-9_]*\s*\([^;\n]*\))*\s*\.\s*spawn\s*\(/,
  },
  {
    id: "spawn-blocking",
    description: "blocking task pool use in runtime hot path",
    pattern: /\bspawn_blocking\s*\(/,
  },
]);

const ALLOWLIST = Object.freeze([
  {
    name: "chain-log-metadata-fingerprint",
    file: "crates/prodex-app/src/runtime_proxy/chain_log.rs",
    id: "blocking-disk-io",
    pattern: /\bfs::metadata\s*\(/,
    maxHits: 2,
    reason: "bounded continuity metrics fingerprint cache; no broad reads/writes",
  },
  {
    name: "launch-worker-pool-threads",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup.rs",
    id: "blocking-thread-spawn",
    pattern: /\bworker_threads\.push\(thread::spawn\s*\(/,
    maxHits: 2,
    reason:
      "bounded rotation acceptor and long-lived proxy worker pools created during launch, outside request commit path",
  },
  {
    name: "local-rewrite-launch-worker-pool-threads",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite.rs",
    id: "blocking-thread-spawn",
    pattern: /\bworker_threads\.push\(thread::spawn\s*\(/,
    maxHits: 1,
    reason:
      "bounded local rewrite worker pool created during launch, outside request commit path",
  },
  {
    name: "local-rewrite-gateway-openoptions-import",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite.rs",
    id: "blocking-file-open",
    pattern: /\buse std::fs::OpenOptions;/,
    maxHits: 1,
    reason:
      "OpenOptions is used by gateway admin and background state stores, not by upstream stream forwarding",
  },
  {
    name: "local-rewrite-gateway-file-state-io",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite.rs",
    id: "blocking-disk-io",
    pattern: /\bstd::fs::(?:read|write|create_dir_all|rename)\s*\(/,
    maxHits: 17,
    reason:
      "gateway file backend I/O is limited to admin/config loading, ledger reconciliation, and background usage persistence outside stream commit",
  },
  {
    name: "local-rewrite-gateway-background-save",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite.rs",
    id: "spawn-blocking",
    pattern: /\bspawn_blocking\s*\(/,
    maxHits: 2,
    reason:
      "gateway ledger and usage saves are moved onto the bounded blocking pool after request admission",
  },
  {
    name: "local-rewrite-gateway-admin-store-openoptions-import",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_store_mutation.rs",
    id: "blocking-file-open",
    pattern: /\buse std::fs::OpenOptions;/,
    maxHits: 1,
    reason:
      "OpenOptions is used by gateway admin store mutations, not by upstream stream forwarding",
  },
  {
    name: "local-rewrite-gateway-admin-store-directory-create",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_admin_store_mutation.rs",
    id: "blocking-disk-io",
    pattern: /\bstd::fs::create_dir_all\s*\(/,
    maxHits: 1,
    reason:
      "gateway admin audit directory creation is limited to admin mutation persistence outside stream commit",
  },
  {
    name: "local-rewrite-gateway-backend-directory-create",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_backend_connection.rs",
    id: "blocking-disk-io",
    pattern: /\bstd::fs::create_dir_all\s*\(/,
    maxHits: 1,
    reason: "gateway backend state directory creation runs for file-backed gateway state",
  },
  {
    name: "local-rewrite-gateway-file-ledger-openoptions-import",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_file_ledger.rs",
    id: "blocking-file-open",
    pattern: /\buse std::fs::OpenOptions;/,
    maxHits: 1,
    reason: "OpenOptions is used by the file ledger backend outside upstream stream forwarding",
  },
  {
    name: "local-rewrite-gateway-file-ledger-io",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_file_ledger.rs",
    id: "blocking-disk-io",
    pattern: /\bstd::fs::(?:read|create_dir_all|rename|remove_file|symlink_metadata)\s*\(/,
    maxHits: 5,
    reason:
      "gateway file ledger I/O is limited to bounded ledger loading, symlink rejection, append setup, atomic summary persistence, and best-effort temp-file cleanup",
  },
  {
    name: "local-rewrite-gateway-ledger-background-save",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_ledger.rs",
    id: "spawn-blocking",
    pattern: /\bspawn_blocking\s*\(/,
    maxHits: 1,
    reason: "gateway ledger saves run on the bounded blocking pool after request admission",
  },
  {
    name: "local-rewrite-transport-observability-background-sinks",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_transport.rs",
    id: "spawn-blocking",
    pattern: /\bspawn_blocking\s*\(/,
    maxHits: 2,
    reason:
      "gateway observability sinks run on the bounded blocking pool after spend-event emission instead of blocking the request path inline",
  },
  {
    name: "local-rewrite-gateway-store-file-io",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_store_file.rs",
    id: "blocking-disk-io",
    pattern: /\bstd::fs::(?:read|write|create_dir_all|rename|remove_file|symlink_metadata)\s*\(/,
    maxHits: 6,
    reason:
      "gateway file-store I/O is limited to bounded admin/config state loading, symlink rejection, atomic saves, and temp cleanup",
  },
  {
    name: "local-rewrite-gateway-usage-openoptions-import",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_usage.rs",
    id: "blocking-file-open",
    pattern: /\buse std::fs::OpenOptions;/,
    maxHits: 1,
    reason: "OpenOptions is used by gateway usage persistence outside upstream stream forwarding",
  },
  {
    name: "local-rewrite-gateway-usage-io",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_usage.rs",
    id: "blocking-disk-io",
    pattern: /\bstd::fs::(?:read|write|create_dir_all|rename)\s*\(/,
    maxHits: 6,
    reason: "gateway usage I/O is limited to file-backed usage loading and atomic background persistence",
  },
  {
    name: "local-rewrite-gateway-usage-background-save",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gateway_usage.rs",
    id: "spawn-blocking",
    pattern: /\bspawn_blocking\s*\(/,
    maxHits: 1,
    reason: "gateway usage saves run on the bounded blocking pool after request admission",
  },
  {
    name: "gemini-live-sidecar-launch-thread",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_gemini_live.rs",
    id: "blocking-thread-spawn",
    pattern: /\bworker_threads\.push\(thread::spawn\s*\(/,
    maxHits: 1,
    reason:
      "single bounded Gemini Live sidecar acceptor created during launch and owned by the runtime shutdown handle",
  },
  {
    name: "kiro-overlay-cleanup-background",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_kiro.rs",
    id: "blocking-disk-io",
    pattern: /\bfs::remove_dir_all\s*\(/,
    maxHits: 2,
    reason:
      "Kiro per-request temp overlay cleanup runs on the bounded runtime blocking pool after request work completes",
  },
  {
    name: "kiro-bounded-runtime-blocking-work",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_kiro.rs",
    id: "spawn-blocking",
    pattern: /\bspawn_blocking\s*\(/,
    maxHits: 2,
    reason:
      "Kiro streaming worker and temp overlay cleanup use the bounded runtime blocking pool instead of unbounded OS thread spawning",
  },
  {
    name: "gemini-bounded-context-directory-scans",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request.rs",
    id: "blocking-disk-io",
    pattern: /\bfs::read_dir\s*\(/,
    maxHits: 5,
    reason:
      "Gemini context, ignore, extension, and policy discovery enforce explicit file and scan-count limits",
  },
  {
    name: "gemini-bounded-ignore-file-read",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request.rs",
    id: "blocking-disk-io",
    pattern: /\bfs::read_to_string\s*\(/,
    maxHits: 1,
    reason: "bounded .gitignore compatibility read used only for explicit Gemini context collection",
  },
  {
    name: "gemini-bounded-local-file-metadata",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request.rs",
    id: "blocking-disk-io",
    pattern: /\bfs::metadata\s*\(/,
    maxHits: 1,
    reason: "metadata check precedes Gemini local-file reads capped by file count and total bytes",
  },
  {
    name: "gemini-bounded-local-file-canonicalize",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request.rs",
    id: "blocking-disk-io",
    pattern: /\bfs::canonicalize\s*\(/,
    maxHits: 1,
    reason: "canonicalization deduplicates Gemini local-file reads before consuming the bounded budget",
  },
  {
    name: "gemini-bounded-local-file-read",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request.rs",
    id: "blocking-disk-io",
    pattern: /\bfs::read\s*\(/,
    maxHits: 1,
    reason: "Gemini local-file content is capped by explicit per-request file and byte budgets",
  },
  {
    name: "gemini-bounded-text-file-open",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request.rs",
    id: "blocking-file-open",
    pattern: /\bfs::File::open\s*\(/,
    maxHits: 1,
    reason: "Gemini memory and import text reads use a take-limited reader",
  },
  {
    name: "gemini-explicit-output-directory-creation",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request.rs",
    id: "blocking-disk-io",
    pattern: /\bfs::create_dir_all\s*\(/,
    maxHits: 2,
    reason:
      "directory creation is limited to explicit checkpoint export and bounded masked tool-output persistence",
  },
  {
    name: "gemini-explicit-output-write",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request.rs",
    id: "blocking-disk-io",
    pattern: /\bfs::write\s*\(/,
    maxHits: 2,
    reason:
      "writes are limited to explicit checkpoint export and bounded masked tool-output persistence",
  },
  {
    name: "gemini-context-bounded-directory-scans",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request_context.rs",
    id: "blocking-disk-io",
    pattern: /\bfs::(?:read_dir|symlink_metadata)\s*\(/,
    maxHits: 5,
    reason:
      "Gemini context discovery enforces explicit file and scan-count limits and rejects symlink entries",
  },
  {
    name: "gemini-context-bounded-ignore-file-read",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request_context.rs",
    id: "blocking-disk-io",
    pattern: /\bfs::read_to_string\s*\(/,
    maxHits: 1,
    reason: "bounded .gitignore compatibility read used only for explicit Gemini context collection",
  },
  {
    name: "gemini-context-bounded-ignore-file-open",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request_context.rs",
    id: "blocking-file-open",
    pattern: /\bfs::File::open\s*\(/,
    maxHits: 1,
    reason: "Gemini ignore files are opened only after a symlink and size check and read with a byte cap",
  },
  {
    name: "gemini-extension-bounded-directory-scan",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request_extensions.rs",
    id: "blocking-disk-io",
    pattern: /\bfs::read_dir\s*\(/,
    maxHits: 1,
    reason: "Gemini extension discovery is bounded by configured extension roots",
  },
  {
    name: "gemini-bounded-text-file-open-split",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request_io.rs",
    id: "blocking-file-open",
    pattern: /\bfs::File::open\s*\(/,
    maxHits: 1,
    reason: "Gemini memory and import text reads use a take-limited reader",
  },
  {
    name: "gemini-bounded-text-file-symlink-check-split",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request_io.rs",
    id: "blocking-disk-io",
    pattern: /\bfs::symlink_metadata\s*\(/,
    maxHits: 2,
    reason: "Gemini local text reads reject symlink paths before opening byte-capped files",
  },
  {
    name: "gemini-bounded-local-file-metadata-split",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request_local_context.rs",
    id: "blocking-disk-io",
    pattern: /\bfs::(?:metadata|symlink_metadata)\s*\(/,
    maxHits: 2,
    reason:
      "metadata and symlink checks precede Gemini local-file reads capped by file count and total bytes",
  },
  {
    name: "gemini-bounded-local-file-canonicalize-split",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request_local_context.rs",
    id: "blocking-disk-io",
    pattern: /\bfs::canonicalize\s*\(/,
    maxHits: 1,
    reason: "canonicalization deduplicates Gemini local-file reads before consuming the bounded budget",
  },
  {
    name: "gemini-bounded-local-file-read-split",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request_local_context.rs",
    id: "blocking-disk-io",
    pattern: /\bfs::read\s*\(/,
    maxHits: 1,
    reason: "Gemini local-file content is capped by explicit per-request file and byte budgets",
  },
  {
    name: "gemini-bounded-local-file-open-split",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request_local_context.rs",
    id: "blocking-file-open",
    pattern: /\bfs::File::open\s*\(/,
    maxHits: 1,
    reason: "Gemini local files are opened only after bounded candidate and metadata checks",
  },
  {
    name: "gemini-bounded-local-directory-scan-split",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request_local_context.rs",
    id: "blocking-disk-io",
    pattern: /\bfs::read_dir\s*\(/,
    maxHits: 1,
    reason: "Gemini local directory reads are capped by explicit file and scan-count budgets",
  },
  {
    name: "gemini-policy-bounded-directory-scan",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request_policy.rs",
    id: "blocking-disk-io",
    pattern: /\bfs::read_dir\s*\(/,
    maxHits: 1,
    reason: "Gemini policy discovery is bounded by configured policy directories",
  },
  {
    name: "gemini-checkpoint-explicit-output-io",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request_session.rs",
    id: "blocking-disk-io",
    pattern: /\bfs::(?:create_dir_all|write)\s*\(/,
    maxHits: 2,
    reason: "checkpoint export writes only when explicitly requested by Gemini checkpoint metadata",
  },
  {
    name: "gemini-tool-output-explicit-output-io",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/gemini_request_tool_output.rs",
    id: "blocking-disk-io",
    pattern: /\bfs::(?:create_dir_all|write)\s*\(/,
    maxHits: 2,
    reason: "masked tool-output persistence is bounded and uses explicit output directories",
  },
  {
    name: "runtime-websocket-tcp-dns-bounded-executor-threads",
    file: "crates/prodex-runtime-proxy/src/websocket_tcp_connect_executor/executor.rs",
    id: "blocking-thread-builder-spawn",
    pattern: /\bthread::Builder::new\(\)\.name\(name\)\.spawn\(job\)\.is_ok\(\)/,
    maxHits: 1,
    reason:
      "bounded websocket TCP/DNS worker and dispatcher pools; spawn-outcome helper is cfg(test)",
  },
  {
    name: "local-rewrite-test-fixture-disk-io",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_tests.rs",
    id: "blocking-disk-io",
    pattern:
      /\bfs::(?:read|read_to_string|write|remove_dir_all|create_dir_all|metadata|set_permissions)\s*\(/,
    maxHits: 71,
    reason:
      "test fixtures inspect audit logs, ledgers, and isolated temp state after proxy requests complete",
  },
  {
    name: "local-rewrite-split-test-fixture-disk-io",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_tests/support.rs",
    id: "blocking-disk-io",
    pattern: /\bfs::(?:read|remove_dir_all|create_dir_all)\s*\(/,
    maxHits: 5,
    reason: "split test fixtures inspect isolated temp state after proxy requests complete",
  },
  {
    name: "local-rewrite-split-test-ledger-read",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_tests/gateway_state.rs",
    id: "blocking-disk-io",
    pattern: /\bfs::(?:read_to_string|write)\s*\(/,
    maxHits: 4,
    reason: "split gateway state tests inspect ledger output after proxy requests complete",
  },
  {
    name: "local-rewrite-split-test-admin-auth-state",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_tests/gateway_admin_auth.rs",
    id: "blocking-disk-io",
    pattern: /\bfs::write\s*\(/,
    maxHits: 1,
    reason: "split gateway admin auth tests write isolated malformed fixture state",
  },
  {
    name: "local-rewrite-split-test-audit-read",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_tests/gateway_admin_crud.rs",
    id: "blocking-disk-io",
    pattern: /\bfs::read_to_string\s*\(/,
    maxHits: 1,
    reason: "split gateway admin tests inspect audit output after proxy requests complete",
  },
  {
    name: "local-rewrite-test-mock-server-threads",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_tests.rs",
    id: "blocking-thread-spawn",
    pattern: /\bthread::spawn\s*\(/,
    maxHits: 3,
    reason: "test-only mock upstream servers run on bounded fixture threads",
  },
  {
    name: "local-rewrite-split-test-mock-server-threads",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_tests/support.rs",
    id: "blocking-thread-spawn",
    pattern: /\bthread::spawn\s*\(/,
    maxHits: 3,
    reason: "split test-only mock upstream servers run on bounded fixture threads",
  },
  {
    name: "local-rewrite-split-test-gateway-usage-thread",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_tests/gateway_usage.rs",
    id: "blocking-thread-spawn",
    pattern: /\bthread::spawn\s*\(/,
    maxHits: 1,
    reason: "split gateway usage test mock server runs on one fixture thread",
  },
  {
    name: "local-rewrite-transport-log-dir-create",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_transport.rs",
    id: "blocking-disk-io",
    pattern: /\bstd::fs::create_dir_all\s*\(/,
    maxHits: 1,
    reason: "best-effort observability directory creation runs on the bounded blocking pool",
  },
  {
    name: "local-rewrite-transport-log-open",
    file: "crates/prodex-app/src/runtime_launch/proxy_startup/local_rewrite_transport.rs",
    id: "blocking-file-open",
    pattern: /\bstd::fs::OpenOptions::new\s*\(/,
    maxHits: 1,
    reason: "best-effort observability JSONL append runs on the bounded blocking pool",
  },
]);

function parseArgs(argv) {
  const args = {
    targets: [],
    json: false,
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--target") {
      index += 1;
      if (!argv[index]) {
        throw new Error(`${value} requires a value`);
      }
      args.targets.push(argv[index]);
      continue;
    }
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
      "Usage: node scripts/ci/runtime-hotpath-guard.mjs [--target <path>] [--json]",
      "",
      "Scans runtime proxy hot-path Rust files for blocking disk I/O and unbounded OS thread spawns.",
      "Test-only #[cfg(test)] items are ignored; production matches require an explicit allowlist entry.",
      "",
      "Default targets:",
      ...DEFAULT_SCAN_TARGETS.map((target) => `  - ${target}`),
    ].join("\n") + "\n",
  );
}

function normalizePath(filePath) {
  return filePath.replaceAll("\\", "/").replace(/^\.\//, "");
}

async function pathExists(filePath) {
  try {
    await fs.access(filePath);
    return true;
  } catch {
    return false;
  }
}

async function rustFilesUnder(targetPath) {
  const absolutePath = path.join(repoRoot, targetPath);
  if (!(await pathExists(absolutePath))) {
    throw new Error(`target does not exist: ${targetPath}`);
  }

  const stat = await fs.stat(absolutePath);
  if (stat.isFile()) {
    return targetPath.endsWith(".rs") ? [normalizePath(targetPath)] : [];
  }

  const files = [];
  const entries = await fs.readdir(absolutePath, { withFileTypes: true });
  for (const entry of entries) {
    const child = normalizePath(path.join(targetPath, entry.name));
    if (entry.isDirectory()) {
      files.push(...(await rustFilesUnder(child)));
      continue;
    }
    if (entry.isFile() && child.endsWith(".rs")) {
      files.push(child);
    }
  }
  return files.sort();
}

function stripStringsAndLineComment(line) {
  let output = "";
  let inString = false;
  let escaped = false;
  for (let index = 0; index < line.length; index += 1) {
    const character = line[index];
    const next = line[index + 1];
    if (!inString && character === "/" && next === "/") {
      break;
    }
    if (character === '"' && !escaped) {
      inString = !inString;
      output += " ";
      continue;
    }
    if (inString) {
      escaped = character === "\\" && !escaped;
      output += " ";
      continue;
    }
    escaped = false;
    output += character;
  }
  return output;
}

function braceDelta(line) {
  let delta = 0;
  for (const character of line) {
    if (character === "{") {
      delta += 1;
    } else if (character === "}") {
      delta -= 1;
    }
  }
  return delta;
}

function cfgTestLine(line) {
  return /^#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]/.test(line.trim());
}

function fullFileCfgTest(contents) {
  for (const line of contents.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (trimmed === "" || trimmed.startsWith("//")) {
      continue;
    }
    return /^#!\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]/.test(trimmed);
  }
  return false;
}

function scannableLines(contents) {
  const lines = contents.split(/\r?\n/);
  const result = [];
  let pendingCfgTest = false;
  let skipDepth = null;

  for (let index = 0; index < lines.length; index += 1) {
    const rawLine = lines[index];
    const trimmed = rawLine.trim();
    const code = stripStringsAndLineComment(rawLine);

    if (skipDepth !== null) {
      skipDepth += braceDelta(code);
      if (skipDepth <= 0) {
        skipDepth = null;
      }
      continue;
    }

    if (cfgTestLine(rawLine)) {
      pendingCfgTest = true;
      continue;
    }

    if (pendingCfgTest) {
      if (trimmed === "" || trimmed.startsWith("#") || trimmed.startsWith("//")) {
        continue;
      }
      if (code.includes("{")) {
        skipDepth = braceDelta(code);
        if (skipDepth <= 0) {
          skipDepth = null;
        }
      }
      pendingCfgTest = false;
      continue;
    }

    result.push({ lineNumber: index + 1, rawLine, code });
  }

  return result;
}

function allowlistEntryFor(filePath, pattern, code) {
  return ALLOWLIST.find(
    (entry) => entry.file === filePath && entry.id === pattern.id && entry.pattern.test(code),
  );
}

async function scanFile(filePath) {
  const contents = await fs.readFile(path.join(repoRoot, filePath), "utf8");
  if (fullFileCfgTest(contents)) {
    return { violations: [], allowlistHits: [] };
  }
  const violations = [];
  const allowlistHits = [];

  for (const line of scannableLines(contents)) {
    for (const pattern of FORBIDDEN_PATTERNS) {
      pattern.pattern.lastIndex = 0;
      if (!pattern.pattern.test(line.code)) {
        continue;
      }
      const allowlistEntry = allowlistEntryFor(filePath, pattern, line.code);
      if (allowlistEntry) {
        allowlistHits.push({
          filePath,
          lineNumber: line.lineNumber,
          id: pattern.id,
          allowlist: allowlistEntry.name,
          reason: allowlistEntry.reason,
        });
        continue;
      }
      violations.push({
        filePath,
        lineNumber: line.lineNumber,
        id: pattern.id,
        description: pattern.description,
        line: line.rawLine.trim(),
      });
    }
  }

  return { violations, allowlistHits };
}

function allowlistOveruseViolations(allowlistHits) {
  const counts = new Map();
  for (const hit of allowlistHits) {
    counts.set(hit.allowlist, (counts.get(hit.allowlist) ?? 0) + 1);
  }

  return ALLOWLIST.flatMap((entry) => {
    const count = counts.get(entry.name) ?? 0;
    if (entry.maxHits === undefined || count <= entry.maxHits) {
      return [];
    }
    return [
      {
        filePath: entry.file,
        lineNumber: 0,
        id: "allowlist-overuse",
        description: `allowlist "${entry.name}" matched ${count} time(s), expected at most ${entry.maxHits}`,
        line: entry.reason,
      },
    ];
  });
}

async function collectScanFiles(targets, excludedPathPrefixes = []) {
  const files = [];
  for (const target of targets) {
    files.push(...(await rustFilesUnder(normalizePath(target))));
  }
  return [...new Set(files)]
    .filter(
      (filePath) =>
        !excludedPathPrefixes.some((prefix) => filePath.startsWith(prefix)),
    )
    .sort();
}

function printHuman(files, violations, allowlistHits) {
  if (violations.length === 0) {
    process.stdout.write(
      `runtime hot-path guard: ok (${files.length} file(s), ${allowlistHits.length} allowlist hit(s))\n`,
    );
    return;
  }

  process.stderr.write(`runtime hot-path guard: ${violations.length} violation(s)\n`);
  for (const violation of violations) {
    process.stderr.write(
      `${violation.filePath}:${violation.lineNumber}: ${violation.id}: ${violation.description}\n`,
    );
    process.stderr.write(`  ${violation.line}\n`);
  }
  process.stderr.write(
    "\nMove blocking work out of the request/stream hot path, or add a narrow allowlist entry with rationale.\n",
  );
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }

  const usingDefaultTargets = args.targets.length === 0;
  const targets = usingDefaultTargets ? [...DEFAULT_SCAN_TARGETS] : args.targets;
  const files = await collectScanFiles(
    targets,
    usingDefaultTargets ? DEFAULT_EXCLUDED_PATH_PREFIXES : [],
  );
  const violations = [];
  const allowlistHits = [];
  for (const filePath of files) {
    const result = await scanFile(filePath);
    violations.push(...result.violations);
    allowlistHits.push(...result.allowlistHits);
  }
  violations.push(...allowlistOveruseViolations(allowlistHits));

  if (args.json) {
    process.stdout.write(`${JSON.stringify({ files, violations, allowlistHits }, null, 2)}\n`);
  } else {
    printHuman(files, violations, allowlistHits);
  }

  if (violations.length > 0) {
    process.exitCode = 1;
  }
}

try {
  await main();
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`runtime-hotpath-guard: ${message}\n`);
  process.exitCode = 1;
}
