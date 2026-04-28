#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { repoRoot } from "../npm/common.mjs";

const docsPath = path.join(repoRoot, "docs/runtime-policy.md");
const runtimePolicyTypesPath = path.join(repoRoot, "src/runtime_policy/types.rs");
const runtimeEnvSourcePaths = [
  path.join(repoRoot, "src/runtime_core_shared.rs"),
  path.join(repoRoot, "src/runtime_tuning.rs"),
];

const beginMarker = "<!-- BEGIN GENERATED RUNTIME_PROXY_KEYS -->";
const endMarker = "<!-- END GENERATED RUNTIME_PROXY_KEYS -->";

const runtimeProxyKeys = [
  {
    policy: "runtime_proxy.worker_count",
    env: "PRODEX_RUNTIME_PROXY_WORKER_COUNT",
    defaultValue: "CPU parallelism clamped to `4..12`",
    meaning: "Short-lived proxy worker pool size.",
  },
  {
    policy: "runtime_proxy.long_lived_worker_count",
    env: "PRODEX_RUNTIME_PROXY_LONG_LIVED_WORKER_COUNT",
    defaultValue: "`parallelism * 2` clamped to `8..24`",
    meaning: "Worker pool for long-lived streams and websocket work.",
  },
  {
    policy: "runtime_proxy.probe_refresh_worker_count",
    env: "PRODEX_RUNTIME_PROBE_REFRESH_WORKER_COUNT",
    defaultValue: "CPU parallelism clamped to `2..4`",
    meaning: "Background profile probe refresh workers.",
  },
  {
    policy: "runtime_proxy.async_worker_count",
    env: "PRODEX_RUNTIME_PROXY_ASYNC_WORKER_COUNT",
    defaultValue: "CPU parallelism clamped to `2..4`",
    meaning: "Async runtime worker count.",
  },
  {
    policy: "runtime_proxy.long_lived_queue_capacity",
    env: "PRODEX_RUNTIME_PROXY_LONG_LIVED_QUEUE_CAPACITY",
    defaultValue: "`long_lived_worker_count * 8` clamped to `128..1024`",
    meaning: "Queue capacity for long-lived proxy work.",
  },
  {
    policy: "runtime_proxy.active_request_limit",
    env: "PRODEX_RUNTIME_PROXY_ACTIVE_REQUEST_LIMIT",
    defaultValue: "`worker_count + long_lived_worker_count * 3` clamped to `64..512`",
    meaning: "Global local admission cap for fresh runtime proxy requests.",
  },
  {
    policy: "runtime_proxy.responses_active_limit",
    env: "PRODEX_RUNTIME_PROXY_RESPONSES_ACTIVE_LIMIT",
    defaultValue: "`75%` of global limit, clamped to `4..global`",
    meaning: "Lane cap for main Responses traffic.",
  },
  {
    policy: "runtime_proxy.compact_active_limit",
    env: "PRODEX_RUNTIME_PROXY_COMPACT_ACTIVE_LIMIT",
    defaultValue: "`25%` of global limit, clamped to `2..6`",
    meaning: "Lane cap for `/responses/compact`.",
  },
  {
    policy: "runtime_proxy.websocket_active_limit",
    env: "PRODEX_RUNTIME_PROXY_WEBSOCKET_ACTIVE_LIMIT",
    defaultValue: "`long_lived_worker_count` clamped to `2..global`",
    meaning: "Lane cap for websocket transport.",
  },
  {
    policy: "runtime_proxy.standard_active_limit",
    env: "PRODEX_RUNTIME_PROXY_STANDARD_ACTIVE_LIMIT",
    defaultValue: "`worker_count / 2` clamped to `2..8`",
    meaning: "Lane cap for other unary proxy traffic.",
  },
  {
    policy: "runtime_proxy.profile_inflight_soft_limit",
    env: "PRODEX_RUNTIME_PROXY_PROFILE_INFLIGHT_SOFT_LIMIT",
    defaultValue: "`4`",
    meaning: "Fresh selection starts penalizing profiles above this in-flight count.",
  },
  {
    policy: "runtime_proxy.profile_inflight_hard_limit",
    env: "PRODEX_RUNTIME_PROXY_PROFILE_INFLIGHT_HARD_LIMIT",
    defaultValue: "`8`",
    meaning: "Fresh selection avoids profiles above this in-flight count; hard affinity still wins.",
  },
  {
    policy: "runtime_proxy.admission_wait_budget_ms",
    env: "PRODEX_RUNTIME_PROXY_ADMISSION_WAIT_BUDGET_MS",
    defaultValue: "`750`",
    meaning: "Normal wait budget for local admission pressure.",
  },
  {
    policy: "runtime_proxy.pressure_admission_wait_budget_ms",
    env: "PRODEX_RUNTIME_PROXY_PRESSURE_ADMISSION_WAIT_BUDGET_MS",
    defaultValue: "`200`",
    meaning: "Shorter admission wait budget when proxy is already under pressure.",
  },
  {
    policy: "runtime_proxy.long_lived_queue_wait_budget_ms",
    env: "PRODEX_RUNTIME_PROXY_LONG_LIVED_QUEUE_WAIT_BUDGET_MS",
    defaultValue: "`750`",
    meaning: "Normal wait budget for long-lived queue pressure.",
  },
  {
    policy: "runtime_proxy.pressure_long_lived_queue_wait_budget_ms",
    env: "PRODEX_RUNTIME_PROXY_PRESSURE_LONG_LIVED_QUEUE_WAIT_BUDGET_MS",
    defaultValue: "`200`",
    meaning: "Shorter long-lived queue wait budget under pressure.",
  },
  {
    policy: "runtime_proxy.http_connect_timeout_ms",
    env: "PRODEX_RUNTIME_PROXY_HTTP_CONNECT_TIMEOUT_MS",
    defaultValue: "`5000`",
    meaning: "Upstream HTTP connect timeout.",
  },
  {
    policy: "runtime_proxy.stream_idle_timeout_ms",
    env: "PRODEX_RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS",
    defaultValue: "`300000`",
    meaning: "Responses stream idle timeout, aligned with Codex behavior.",
  },
  {
    policy: "runtime_proxy.sse_lookahead_timeout_ms",
    env: "PRODEX_RUNTIME_PROXY_SSE_LOOKAHEAD_TIMEOUT_MS",
    defaultValue: "`1000`",
    meaning: "Pre-commit SSE lookahead timeout.",
  },
  {
    policy: "runtime_proxy.prefetch_backpressure_retry_ms",
    env: "PRODEX_RUNTIME_PROXY_PREFETCH_BACKPRESSURE_RETRY_MS",
    defaultValue: "`10`",
    meaning: "Retry delay while stream prefetch is backpressured.",
  },
  {
    policy: "runtime_proxy.prefetch_backpressure_timeout_ms",
    env: "PRODEX_RUNTIME_PROXY_PREFETCH_BACKPRESSURE_TIMEOUT_MS",
    defaultValue: "`1000`",
    meaning: "Max wait for stream prefetch backpressure to clear.",
  },
  {
    policy: "runtime_proxy.prefetch_max_buffered_bytes",
    env: "PRODEX_RUNTIME_PROXY_PREFETCH_MAX_BUFFERED_BYTES",
    defaultValue: "`786432`",
    meaning: "Max buffered prefetch bytes before backpressure.",
  },
  {
    policy: "runtime_proxy.websocket_connect_timeout_ms",
    env: "PRODEX_RUNTIME_PROXY_WEBSOCKET_CONNECT_TIMEOUT_MS",
    defaultValue: "`15000`",
    meaning: "Upstream websocket connect timeout.",
  },
  {
    policy: "runtime_proxy.websocket_happy_eyeballs_delay_ms",
    env: "PRODEX_RUNTIME_PROXY_WEBSOCKET_HAPPY_EYEBALLS_DELAY_MS",
    defaultValue: "`200`",
    meaning: "Delay before alternate websocket TCP connect attempt.",
  },
  {
    policy: "runtime_proxy.websocket_precommit_progress_timeout_ms",
    env: "PRODEX_RUNTIME_PROXY_WEBSOCKET_PRECOMMIT_PROGRESS_TIMEOUT_MS",
    defaultValue: "`8000`",
    meaning: "Websocket pre-commit progress timeout.",
  },
  {
    policy: "runtime_proxy.websocket_connect_worker_count",
    env: "PRODEX_RUNTIME_WEBSOCKET_CONNECT_WORKER_COUNT",
    defaultValue: "CPU parallelism clamped to `4..16`",
    meaning: "Worker count for bounded websocket TCP connect executor.",
  },
  {
    policy: "runtime_proxy.websocket_connect_queue_capacity",
    env: "PRODEX_RUNTIME_WEBSOCKET_CONNECT_QUEUE_CAPACITY",
    defaultValue: "`websocket_connect_worker_count * 8` clamped to `32..128`",
    meaning:
      "Bounded queue capacity for websocket TCP connect work; effective value is at least the worker count.",
  },
  {
    policy: "runtime_proxy.websocket_connect_overflow_capacity",
    env: "PRODEX_RUNTIME_WEBSOCKET_CONNECT_OVERFLOW_CAPACITY",
    defaultValue: "`websocket_connect_queue_capacity * 4` clamped to `32..512`",
    meaning:
      "Overflow queue capacity for websocket TCP connect work after the bounded queue fills; `0` disables overflow buffering.",
  },
  {
    policy: "runtime_proxy.websocket_dns_worker_count",
    env: "PRODEX_RUNTIME_WEBSOCKET_DNS_WORKER_COUNT",
    defaultValue: "CPU parallelism clamped to `2..8`",
    meaning: "Worker count for bounded websocket DNS resolution executor.",
  },
  {
    policy: "runtime_proxy.websocket_dns_queue_capacity",
    env: "PRODEX_RUNTIME_WEBSOCKET_DNS_QUEUE_CAPACITY",
    defaultValue: "`websocket_dns_worker_count * 4` clamped to `16..64`",
    meaning:
      "Bounded queue capacity for websocket DNS resolution work; effective value is at least the worker count.",
  },
  {
    policy: "runtime_proxy.websocket_dns_overflow_capacity",
    env: "PRODEX_RUNTIME_WEBSOCKET_DNS_OVERFLOW_CAPACITY",
    defaultValue: "`websocket_dns_queue_capacity * 2` clamped to `16..128`",
    meaning:
      "Overflow queue capacity for websocket DNS resolution work after the bounded queue fills; `0` disables overflow buffering.",
  },
  {
    policy: "runtime_proxy.websocket_previous_response_reuse_stale_ms",
    env: "PRODEX_RUNTIME_PROXY_WEBSOCKET_PREVIOUS_RESPONSE_REUSE_STALE_MS",
    defaultValue: "`60000`",
    meaning:
      "Window for reusing a websocket previous-response binding before treating it as stale.",
  },
  {
    policy: "runtime_proxy.broker_ready_timeout_ms",
    env: "PRODEX_RUNTIME_BROKER_READY_TIMEOUT_MS",
    defaultValue: "`15000`",
    meaning: "Startup wait for the runtime broker to become ready.",
  },
  {
    policy: "runtime_proxy.broker_health_connect_timeout_ms",
    env: "PRODEX_RUNTIME_BROKER_HEALTH_CONNECT_TIMEOUT_MS",
    defaultValue: "`750`",
    meaning: "Broker health check connect timeout.",
  },
  {
    policy: "runtime_proxy.broker_health_read_timeout_ms",
    env: "PRODEX_RUNTIME_BROKER_HEALTH_READ_TIMEOUT_MS",
    defaultValue: "`1500`",
    meaning: "Broker health check read timeout.",
  },
  {
    policy: "runtime_proxy.sync_probe_pressure_pause_ms",
    env: "PRODEX_RUNTIME_PROXY_SYNC_PROBE_PRESSURE_PAUSE_MS",
    defaultValue: "`5`",
    meaning: "Pause before synchronous probe work when local pressure is detected.",
  },
  {
    policy: "runtime_proxy.responses_critical_floor_percent",
    env: "PRODEX_RUNTIME_PROXY_RESPONSES_CRITICAL_FLOOR_PERCENT",
    defaultValue: "`2`",
    meaning: "Minimum remaining Responses quota percentage treated as critical; valid range `1..10`.",
  },
  {
    policy: "runtime_proxy.startup_sync_probe_warm_limit",
    env: "PRODEX_RUNTIME_STARTUP_SYNC_PROBE_WARM_LIMIT",
    defaultValue: "`1`",
    meaning: "Startup synchronous quota probe warm-up limit, capped internally at `3`.",
  },
];

function parseArgs(argv) {
  const args = {
    write: false,
    check: false,
    print: false,
    help: false,
  };

  for (const value of argv.slice(2)) {
    if (value === "--write") {
      args.write = true;
    } else if (value === "--check") {
      args.check = true;
    } else if (value === "--print") {
      args.print = true;
    } else if (value === "--help" || value === "-h") {
      args.help = true;
    } else {
      throw new Error(`unknown argument: ${value}`);
    }
  }

  if (!args.write && !args.check && !args.print) {
    args.write = true;
  }

  return args;
}

function tableCell(value) {
  return value.replaceAll("|", "\\|");
}

function renderRuntimeProxyTable() {
  const rows = [
    beginMarker,
    "| Policy key | Environment override | Default | Meaning |",
    "| --- | --- | --- | --- |",
  ];

  for (const key of runtimeProxyKeys) {
    rows.push(
      `| \`${key.policy}\` | \`${key.env}\` | ${tableCell(key.defaultValue)} | ${tableCell(
        key.meaning,
      )} |`,
    );
  }

  rows.push(endMarker);
  return rows.join("\n");
}

function replaceGeneratedBlock(contents, generatedTable) {
  const beginIndex = contents.indexOf(beginMarker);
  const endIndex = contents.indexOf(endMarker);

  if (beginIndex >= 0 || endIndex >= 0) {
    if (beginIndex < 0 || endIndex < 0 || endIndex < beginIndex) {
      throw new Error(`invalid generated marker pair in ${path.relative(repoRoot, docsPath)}`);
    }
    return `${contents.slice(0, beginIndex)}${generatedTable}${contents.slice(
      endIndex + endMarker.length,
    )}`;
  }

  const headingIndex = contents.indexOf("## Runtime Proxy Keys");
  if (headingIndex < 0) {
    throw new Error("failed to find Runtime Proxy Keys heading");
  }
  const tableStart = contents.indexOf("| Policy key | Environment override | Default | Meaning |", headingIndex);
  if (tableStart < 0) {
    throw new Error("failed to find Runtime Proxy Keys table");
  }
  const nextParagraph = contents.indexOf("\n\n", tableStart);
  if (nextParagraph < 0) {
    throw new Error("failed to find end of Runtime Proxy Keys table");
  }
  return `${contents.slice(0, tableStart)}${generatedTable}${contents.slice(nextParagraph)}`;
}

function extractRuntimeProxyPolicyFields(typesSource) {
  const structMatch = typesSource.match(
    /pub\(crate\) struct RuntimePolicyProxySettings\s*\{([\s\S]*?)\n\}/m,
  );
  if (!structMatch) {
    throw new Error("failed to find RuntimePolicyProxySettings in src/runtime_policy/types.rs");
  }

  return [...structMatch[1].matchAll(/pub\(crate\)\s+([a-z0-9_]+):\s+Option</g)].map(
    (match) => match[1],
  );
}

function duplicateValues(values) {
  const seen = new Set();
  const duplicates = new Set();
  for (const value of values) {
    if (seen.has(value)) {
      duplicates.add(value);
    }
    seen.add(value);
  }
  return [...duplicates];
}

async function checkMetadataAgainstRust() {
  const errors = [];
  const metadataFields = runtimeProxyKeys.map((key) =>
    key.policy.replace(/^runtime_proxy\./, ""),
  );
  const duplicatePolicyKeys = duplicateValues(runtimeProxyKeys.map((key) => key.policy));
  const duplicateEnvKeys = duplicateValues(runtimeProxyKeys.map((key) => key.env));
  for (const duplicate of duplicatePolicyKeys) {
    errors.push(`duplicate policy metadata key: ${duplicate}`);
  }
  for (const duplicate of duplicateEnvKeys) {
    errors.push(`duplicate policy env key: ${duplicate}`);
  }

  const typesSource = await fs.readFile(runtimePolicyTypesPath, "utf8");
  const rustFields = extractRuntimeProxyPolicyFields(typesSource);
  const metadataFieldSet = new Set(metadataFields);
  const rustFieldSet = new Set(rustFields);

  for (const field of rustFields) {
    if (!metadataFieldSet.has(field)) {
      errors.push(`RuntimePolicyProxySettings.${field} is missing from docs metadata`);
    }
  }
  for (const field of metadataFields) {
    if (!rustFieldSet.has(field)) {
      errors.push(`docs metadata key runtime_proxy.${field} is missing from RuntimePolicyProxySettings`);
    }
  }

  const envSources = await Promise.all(
    runtimeEnvSourcePaths.map((sourcePath) => fs.readFile(sourcePath, "utf8")),
  );
  const envSource = envSources.join("\n");
  for (const key of runtimeProxyKeys) {
    if (!envSource.includes(`"${key.env}"`)) {
      errors.push(`${key.env} for ${key.policy} is missing from runtime env source`);
    }
  }

  return errors;
}

async function checkDocs(generatedTable) {
  const contents = await fs.readFile(docsPath, "utf8");
  const nextContents = replaceGeneratedBlock(contents, generatedTable);
  if (contents !== nextContents) {
    return [
      `${path.relative(
        repoRoot,
        docsPath,
      )} Runtime Proxy Keys table is stale; run npm run docs:runtime-policy`,
    ];
  }
  return [];
}

function printHelp() {
  process.stdout.write(
    [
      "Usage: node scripts/docs/runtime-policy.mjs [--write|--check|--print]",
      "",
      "Generates or checks the Runtime Proxy Keys table in docs/runtime-policy.md.",
      "The metadata table is checked against RuntimePolicyProxySettings and Rust env names.",
    ].join("\n") + "\n",
  );
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    printHelp();
    return;
  }

  const generatedTable = renderRuntimeProxyTable();
  if (args.print) {
    process.stdout.write(`${generatedTable}\n`);
  }

  const errors = await checkMetadataAgainstRust();

  if (args.write) {
    const contents = await fs.readFile(docsPath, "utf8");
    const nextContents = replaceGeneratedBlock(contents, generatedTable);
    if (contents !== nextContents) {
      await fs.writeFile(docsPath, nextContents);
      process.stdout.write(`updated ${path.relative(repoRoot, docsPath)}\n`);
    } else {
      process.stdout.write(`${path.relative(repoRoot, docsPath)} already up to date\n`);
    }
  }

  if (args.check) {
    errors.push(...(await checkDocs(generatedTable)));
  }

  if (errors.length > 0) {
    for (const error of errors) {
      process.stderr.write(`${error}\n`);
    }
    process.exitCode = 1;
    return;
  }

  if (args.check) {
    process.stdout.write("runtime policy docs: ok\n");
  }
}

try {
  await main();
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`runtime-policy-docs: ${message}\n`);
  process.exitCode = 1;
}
