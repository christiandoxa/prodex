#!/usr/bin/env node
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { repoRoot } from "../npm/common.mjs";

const DOC_URL = "https://docs.anthropic.com/en/docs/claude-code/overview";
const CODEX_RELEASE = Object.freeze({
  tag_name: "codex-v1.2.3",
  name: "Codex 1.2.3",
  published_at: "2026-01-02T03:04:05Z",
  html_url: "https://github.com/openai/codex/releases/tag/codex-v1.2.3",
});
const CLAUDE_RELEASE = Object.freeze({
  tag_name: "claude-code-v4.5.6",
  name: "Claude Code 4.5.6",
  published_at: "2026-01-03T03:04:05Z",
  html_url: "https://github.com/anthropics/claude-code/releases/tag/claude-code-v4.5.6",
});

function baseline() {
  return {
    codex: {
      latestRelease: CODEX_RELEASE,
      compatibility: {
        upstream_repository: "https://github.com/openai/codex",
        baseline_source: "offline fixture",
        critical_files: [
          {
            path: "codex-rs/core/src/client.rs",
            reason: "fixture client contract",
            required_contains: ["RESPONSES_ENDPOINT", "/responses", "x-codex-turn-state"],
          },
          {
            path: "codex-rs/core/src/compact_remote.rs",
            reason: "fixture compact contract",
            required_contains: ["run_remote_compact_task"],
          },
        ],
        semantic_checks: [
          {
            id: "client.responses-route",
            kind: "route",
            file: "codex-rs/core/src/client.rs",
            reason: "fixture route grouping",
            file_contains_all: ["RESPONSES_ENDPOINT", "/responses"],
          },
          {
            id: "client.turn-state-header",
            kind: "header_group",
            file: "codex-rs/core/src/client.rs",
            reason: "fixture header grouping",
            file_contains_all: ["build_conversation_headers", "x-codex-turn-state"],
          },
        ],
      },
    },
    claude: {
      latestRelease: CLAUDE_RELEASE,
      docs: [
        {
          url: DOC_URL,
          title: "Claude Code fixture",
          description: "Use Claude Code in terminal.",
          required_contains: ["Claude Code", "terminal"],
        },
      ],
    },
  };
}

const SYNC_CLIENT = [
  "const RESPONSES_ENDPOINT: &str = \"/responses\";",
  "fn build_conversation_headers() {",
  "  let _ = \"x-codex-turn-state\";",
  "}",
].join("\n");
const SYNC_COMPACT = "fn run_remote_compact_task() {}\n";
const DRIFT_CLIENT = [
  "const RESPONSES_ENDPOINT: &str = \"/renamed_responses\";",
  "fn build_conversation_headers() {}",
].join("\n");

const FETCH_FIXTURES = Object.freeze({
  sync: {
    codexRelease: CODEX_RELEASE,
    claudeRelease: CLAUDE_RELEASE,
    files: {
      "codex-rs/core/src/client.rs": SYNC_CLIENT,
      "codex-rs/core/src/compact_remote.rs": SYNC_COMPACT,
    },
    docHtml:
      '<html><head><title>Claude Code fixture</title><meta name="description" content="Use Claude Code in terminal."></head><body>Claude Code works in terminal.</body></html>',
  },
  releaseOnly: {
    codexRelease: {
      tag_name: "codex-v1.2.4",
      name: "Codex 1.2.4",
      published_at: "2026-01-05T03:04:05Z",
      html_url: "https://github.com/openai/codex/releases/tag/codex-v1.2.4",
    },
    claudeRelease: {
      tag_name: "claude-code-v4.5.7",
      name: "Claude Code 4.5.7",
      published_at: "2026-01-06T03:04:05Z",
      html_url: "https://github.com/anthropics/claude-code/releases/tag/claude-code-v4.5.7",
    },
    files: {
      "codex-rs/core/src/client.rs": SYNC_CLIENT,
      "codex-rs/core/src/compact_remote.rs": SYNC_COMPACT,
    },
    docHtml:
      '<html><head><title>Claude Code fixture</title><meta name="description" content="Use Claude Code in terminal."></head><body>Claude Code works in terminal.</body></html>',
  },
  drift: {
    codexRelease: {
      tag_name: "codex-v1.2.4",
      name: "Codex 1.2.4",
      published_at: "2026-01-05T03:04:05Z",
      html_url: "https://github.com/openai/codex/releases/tag/codex-v1.2.4",
    },
    claudeRelease: {
      tag_name: "claude-code-v4.5.7",
      name: "Claude Code 4.5.7",
      published_at: "2026-01-06T03:04:05Z",
      html_url: "https://github.com/anthropics/claude-code/releases/tag/claude-code-v4.5.7",
    },
    files: {
      "codex-rs/core/src/client.rs": DRIFT_CLIENT,
      "codex-rs/core/src/compact_remote.rs": SYNC_COMPACT,
    },
    docHtml:
      '<html><head><title>Claude Tools fixture</title><meta name="description" content="Use renamed docs."></head><body>renamed docs only</body></html>',
  },
  fallback: {
    codexRelease: CODEX_RELEASE,
    claudeRelease: CLAUDE_RELEASE,
    fallbackRawRef: CODEX_RELEASE.tag_name,
    files: {
      "codex-rs/core/src/client.rs": SYNC_CLIENT,
      "codex-rs/core/src/compact_remote.rs": SYNC_COMPACT,
    },
    docHtml:
      '<html><head><title>Claude Code fixture</title><meta name="description" content="Use Claude Code in terminal."></head><body>Claude Code works in terminal.</body></html>',
  },
});

function mockFetchModule() {
  return `const DOC_URL = ${JSON.stringify(DOC_URL)};
const FIXTURES = ${JSON.stringify(FETCH_FIXTURES)};
const mode = process.env.WATCH_UPSTREAM_FIXTURE_MODE;
const fixture = FIXTURES[mode];

if (!fixture) {
  throw new Error(\`unknown WATCH_UPSTREAM_FIXTURE_MODE: \${mode}\`);
}

function jsonResponse(body) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
}

function textResponse(body, headers = {}) {
  return new Response(body, {
    status: 200,
    headers: { "content-type": "text/plain", ...headers },
  });
}

function rawRequestParts(url) {
  const parsed = new URL(url);
  const [, owner, repo, ref, ...fileParts] = parsed.pathname.split("/");
  if (owner !== "openai" || repo !== "codex" || !ref || fileParts.length === 0) {
    return null;
  }
  return {
    ref: decodeURIComponent(ref),
    filePath: fileParts.map((part) => decodeURIComponent(part)).join("/"),
  };
}

globalThis.fetch = async (url) => {
  const href = String(url);
  if (href === "https://api.github.com/repos/openai/codex/releases/latest") {
    return jsonResponse(fixture.codexRelease);
  }
  if (href === "https://api.github.com/repos/anthropics/claude-code/releases/latest") {
    return jsonResponse(fixture.claudeRelease);
  }
  if (href === DOC_URL) {
    return textResponse(fixture.docHtml, { "content-type": "text/html" });
  }
  if (href.startsWith("https://raw.githubusercontent.com/openai/codex/")) {
    const parts = rawRequestParts(href);
    if (parts?.ref === fixture.fallbackRawRef) {
      return new Response("not found", { status: 404, statusText: "Not Found" });
    }
    if (parts && Object.hasOwn(fixture.files, parts.filePath)) {
      return textResponse(fixture.files[parts.filePath]);
    }
    return new Response(\`unmocked raw fixture: \${href}\`, { status: 404, statusText: "Not Found" });
  }
  return new Response(\`unmocked fixture: \${href}\`, { status: 404, statusText: "Not Found" });
};
`;
}

function runWatchdog({ mode, baselinePath, reportPath, mockPath }) {
  return new Promise((resolve, reject) => {
    const args = [
      "--import",
      mockPath,
      "scripts/compat/watch-upstream.mjs",
      "--baseline",
      baselinePath,
      "--report",
      reportPath,
    ];
    const child = spawn(process.execPath, args, {
      cwd: repoRoot,
      env: {
        ...process.env,
        GITHUB_TOKEN: "",
        WATCH_UPSTREAM_FIXTURE_MODE: mode,
      },
      stdio: ["ignore", "pipe", "pipe"],
    });

    let stdout = "";
    let stderr = "";
    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    child.on("error", reject);
    child.on("close", (code, signal) => {
      resolve({
        code: signal ? null : code,
        command: [process.execPath, ...args].join(" "),
        signal,
        stdout,
        stderr,
      });
    });
  });
}

async function runFixture(mode) {
  const tempDir = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-watch-upstream-fixtures-"));
  const baselinePath = path.join(tempDir, "baseline.json");
  const reportPath = path.join(tempDir, "report.json");
  const mockPath = path.join(tempDir, "mock-fetch.mjs");

  try {
    await fs.writeFile(baselinePath, `${JSON.stringify(baseline(), null, 2)}\n`);
    await fs.writeFile(mockPath, mockFetchModule());
    const result = await runWatchdog({ mode, baselinePath, reportPath, mockPath });
    let report = null;
    try {
      report = JSON.parse(await fs.readFile(reportPath, "utf8"));
    } catch (error) {
      if (result.code === 0 || result.code === 1) {
        throw error;
      }
    }
    return { ...result, report };
  } finally {
    await fs.rm(tempDir, { recursive: true, force: true });
  }
}

function diffPaths(report) {
  return report.diffs.map((diff) => diff.path);
}

function diffFor(report, pathLabel) {
  return report.diffs.find((diff) => diff.path === pathLabel);
}

function assertCleanExit(name, result, expectedExit) {
  assert.equal(
    result.signal,
    null,
    `${name}: command signalled ${result.signal}\nstdout:\n${result.stdout}\nstderr:\n${result.stderr}`,
  );
  assert.equal(
    result.code,
    expectedExit,
    `${name}: expected exit ${expectedExit}, got ${result.code}\ncommand: ${result.command}\nstdout:\n${result.stdout}\nstderr:\n${result.stderr}`,
  );
}

const FIXTURES = [
  {
    name: "in-sync report stays clean",
    mode: "sync",
    expectedExit: 0,
    assert(result) {
      assert.match(result.stdout, /Status: in sync/);
      assert.deepEqual(result.report.diffs, []);
      assert.deepEqual(result.report.current.codex.compatibility.critical_files[0].required_contains_missing, []);
    },
  },
  {
    name: "drift report classifies release docs file and semantic diffs",
    mode: "drift",
    expectedExit: 1,
    assert(result) {
      assert.match(result.stdout, /Status: compatibility break/);
      assert.deepEqual(result.report.diff_summary, {
        status: "compatibility_break",
        exit_code: 1,
        total: 14,
        blocking: 4,
        non_blocking: 10,
        labels: {
          release_drift: 8,
          required_content_missing: 2,
          semantic_required_content_missing: 2,
          documentation_metadata_drift: 2,
        },
        categories: {
          release_metadata: 8,
          semantic_compatibility: 4,
          documentation_metadata: 2,
        },
        severities: {
          warning: 10,
          error: 4,
        },
      });
      assert.deepEqual(diffPaths(result.report), [
        "codex.latestRelease.tag_name",
        "codex.latestRelease.name",
        "codex.latestRelease.published_at",
        "codex.latestRelease.html_url",
        "codex.compatibility.critical_files.codex-rs/core/src/client.rs.required_contains",
        "codex.compatibility.semantic_checks.client.responses-route.file_contains_all",
        "codex.compatibility.semantic_checks.client.turn-state-header.file_contains_all",
        "claude.latestRelease.tag_name",
        "claude.latestRelease.name",
        "claude.latestRelease.published_at",
        "claude.latestRelease.html_url",
        `claude.docs.${DOC_URL}.title`,
        `claude.docs.${DOC_URL}.description`,
        `claude.docs.${DOC_URL}.required_contains`,
      ]);
      assert.deepEqual(
        diffFor(
          result.report,
          "codex.compatibility.critical_files.codex-rs/core/src/client.rs.required_contains",
        ).current.missing,
        ["/responses", "x-codex-turn-state"],
      );
      assert.equal(
        diffFor(
          result.report,
          "codex.compatibility.critical_files.codex-rs/core/src/client.rs.required_contains",
        ).blocking,
        true,
      );
      assert.deepEqual(
        diffFor(result.report, "codex.compatibility.semantic_checks.client.responses-route.file_contains_all").current
          .missing,
        ["/responses"],
      );
      assert.deepEqual(
        diffFor(result.report, "codex.compatibility.semantic_checks.client.turn-state-header.file_contains_all")
          .current.missing,
        ["x-codex-turn-state"],
      );
      assert.deepEqual(
        diffFor(result.report, `claude.docs.${DOC_URL}.required_contains`).current.missing,
        ["Claude Code", "terminal"],
      );
      assert.match(
        result.stdout,
        /codex\.compatibility\.semantic_checks\.client\.responses-route\.file_contains_all/,
      );
    },
  },
  {
    name: "release-only drift exits clean",
    mode: "releaseOnly",
    expectedExit: 0,
    assert(result) {
      assert.match(result.stdout, /Status: release drift/);
      assert.deepEqual(result.report.diff_summary, {
        status: "release_drift",
        exit_code: 0,
        total: 8,
        blocking: 0,
        non_blocking: 8,
        labels: {
          release_drift: 8,
        },
        categories: {
          release_metadata: 8,
        },
        severities: {
          warning: 8,
        },
      });
      assert.deepEqual(
        result.report.diffs.map((diff) => [diff.label, diff.blocking]),
        Array.from({ length: 8 }, () => ["release_drift", false]),
      );
    },
  },
  {
    name: "release raw 404 fallback is reported without drift",
    mode: "fallback",
    expectedExit: 0,
    assert(result) {
      assert.match(
        result.stdout,
        /codex-rs\/core\/src\/client\.rs: main \(fallback from codex-v1\.2\.3\)/,
      );
      assert.deepEqual(result.report.diffs, []);
      const file = result.report.current.codex.compatibility.critical_files[0];
      assert.equal(file.source_ref, "main");
      assert.equal(file.fallback_used, true);
      assert.deepEqual(
        file.fetch_attempts.map((attempt) => [attempt.ref, attempt.ok ?? false, attempt.status ?? null]),
        [
          ["codex-v1.2.3", false, 404],
          ["main", true, null],
        ],
      );
    },
  },
];

let failures = 0;
for (const fixture of FIXTURES) {
  try {
    const result = await runFixture(fixture.mode);
    assertCleanExit(fixture.name, result, fixture.expectedExit);
    fixture.assert(result);
    process.stdout.write(`ok - ${fixture.name}\n`);
  } catch (error) {
    failures += 1;
    process.stderr.write(`not ok - ${fixture.name}\n  ${error.stack ?? error.message}\n`);
  }
}

if (failures > 0) {
  process.stderr.write(`watch upstream fixtures: ${failures} failed\n`);
  process.exitCode = 1;
} else {
  process.stdout.write(`watch upstream fixtures: ${FIXTURES.length} passed\n`);
}
