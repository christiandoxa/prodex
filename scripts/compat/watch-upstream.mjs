#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { repoRoot } from "../npm/common.mjs";

const DEFAULT_BASELINE_PATH = path.join(repoRoot, "scripts/compat/upstream-baseline.json");
const CODEX_RAW_ROOT = "https://raw.githubusercontent.com/openai/codex";
const CODEX_FALLBACK_REF = "main";
const FETCH_TIMEOUT_MS = 15_000;
const FETCH_RETRY_BACKOFF_MS = [500, 1_500];
const GITHUB_API_ROOT = "https://api.github.com/";

const SEMANTIC_FILE_CONTAINS_FIELD = "file_contains_all";
const SEMANTIC_DIFF_FIELDS = [
  {
    field: SEMANTIC_FILE_CONTAINS_FIELD,
    found: "file_contains_found",
    missing: "file_contains_missing",
    source: "file",
  },
];

class FetchResponseError extends Error {
  constructor(url, response) {
    super(`failed to fetch ${url}: ${response.status} ${response.statusText}`);
    this.name = "FetchResponseError";
    this.url = url;
    this.status = response.status;
    this.statusText = response.statusText;
  }
}

function delay(ms) {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

function githubApiAuthHeaders(url) {
  const token = process.env.GITHUB_TOKEN?.trim();
  if (!token || !url.startsWith(GITHUB_API_ROOT)) {
    return {};
  }
  return { Authorization: `Bearer ${token}` };
}

function isTransientStatus(status) {
  return status === 429 || (status >= 500 && status <= 599);
}

function isTransientFetchError(error) {
  if (error instanceof FetchResponseError) {
    return isTransientStatus(error.status);
  }
  return error?.name === "AbortError" || error?.name === "TimeoutError" || error instanceof TypeError;
}

function fetchErrorMessage(url, error) {
  if (error?.name === "AbortError") {
    return `failed to fetch ${url}: timed out after ${FETCH_TIMEOUT_MS}ms`;
  }
  return `failed to fetch ${url}: ${error.message}`;
}

async function fetchWithRetries(url, headers, parser) {
  let lastError = null;
  const attempts = FETCH_RETRY_BACKOFF_MS.length + 1;

  for (let attempt = 0; attempt < attempts; attempt += 1) {
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), FETCH_TIMEOUT_MS);
    try {
      const response = await fetch(url, {
        signal: controller.signal,
        headers: {
          "User-Agent": "prodex-compat-watchdog",
          ...githubApiAuthHeaders(url),
          ...headers,
        },
      });
      if (!response.ok) {
        throw new FetchResponseError(url, response);
      }
      return await parser(response);
    } catch (error) {
      lastError = error;
      clearTimeout(timeout);
      if (attempt >= FETCH_RETRY_BACKOFF_MS.length || !isTransientFetchError(error)) {
        if (error instanceof FetchResponseError) {
          throw error;
        }
        throw new Error(fetchErrorMessage(url, error));
      }
      await delay(FETCH_RETRY_BACKOFF_MS[attempt]);
    } finally {
      clearTimeout(timeout);
    }
  }

  throw new Error(fetchErrorMessage(url, lastError));
}

function parseArgs(argv) {
  const args = {
    baseline: DEFAULT_BASELINE_PATH,
    report: null,
    writeBaseline: false,
    json: false,
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--baseline") {
      index += 1;
      if (!argv[index]) {
        throw new Error("--baseline requires a value");
      }
      args.baseline = argv[index];
      continue;
    }
    if (value === "--report") {
      index += 1;
      if (!argv[index]) {
        throw new Error("--report requires a value");
      }
      args.report = argv[index];
      continue;
    }
    if (value === "--write-baseline") {
      args.writeBaseline = true;
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

function cleanText(value) {
  return value
    .replace(/<script[\s\S]*?<\/script>/gi, " ")
    .replace(/<style[\s\S]*?<\/style>/gi, " ")
    .replace(/<[^>]+>/g, " ")
    .replace(/&nbsp;/g, " ")
    .replace(/&amp;/g, "&")
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/\s+/g, " ")
    .trim();
}

function parseMetaDescription(html) {
  const match = html.match(/<meta name="description" content="([^"]*)"/i);
  return match ? match[1] : "";
}

function parseTitle(html) {
  const match = html.match(/<title>(.*?)<\/title>/i);
  return match ? cleanText(match[1]) : "";
}

async function fetchText(url, headers = {}) {
  return fetchWithRetries(
    url,
    {
      Accept: "text/html,application/json;q=0.9,*/*;q=0.8",
      ...headers,
    },
    (response) => response.text(),
  );
}

async function fetchJson(url, headers = {}) {
  return fetchWithRetries(
    url,
    {
      Accept: "application/vnd.github+json",
      ...headers,
    },
    (response) => response.json(),
  );
}

async function fetchCodexRelease() {
  const data = await fetchJson("https://api.github.com/repos/openai/codex/releases/latest");
  const bodyHead = String(data.body ?? "")
    .split(/\r?\n/)
    .map((line) => line.trimEnd())
    .filter(Boolean)
    .slice(0, 8);
  return {
    tag_name: data.tag_name ?? "",
    name: data.name ?? "",
    published_at: data.published_at ?? "",
    html_url: data.html_url ?? "",
    body_head: bodyHead,
  };
}

async function fetchClaudeRelease() {
  const data = await fetchJson("https://api.github.com/repos/anthropics/claude-code/releases/latest");
  return {
    tag_name: data.tag_name ?? "",
    name: data.name ?? "",
    published_at: data.published_at ?? "",
    html_url: data.html_url ?? "",
  };
}

async function fetchClaudeDoc(page) {
  const html = await fetchText(page.url);
  const text = cleanText(html);
  const requiredContains = page.required_contains.filter((needle) =>
    text.toLowerCase().includes(needle.toLowerCase()),
  );
  const missingContains = page.required_contains.filter(
    (needle) => !text.toLowerCase().includes(needle.toLowerCase()),
  );
  return {
    url: page.url,
    title: parseTitle(html),
    description: parseMetaDescription(html),
    required_contains: page.required_contains,
    required_contains_found: requiredContains,
    required_contains_missing: missingContains,
  };
}

function stringArray(value) {
  if (!Array.isArray(value)) {
    return [];
  }
  return value.filter((item) => typeof item === "string");
}

function rawCodexFileUrl(ref, filePath) {
  return `${CODEX_RAW_ROOT}/${encodeURIComponent(ref)}/${filePath
    .split("/")
    .map((part) => encodeURIComponent(part))
    .join("/")}`;
}

function fetchAttemptSummary(attempts) {
  return attempts
    .map((attempt) => {
      const status = attempt.status ? ` status=${attempt.status}` : "";
      return `${attempt.ref}${status}: ${attempt.error}`;
    })
    .join("; ");
}

async function fetchCodexRawFile(filePath, releaseTag) {
  const primaryRef = releaseTag || CODEX_FALLBACK_REF;
  const refs = primaryRef === CODEX_FALLBACK_REF ? [CODEX_FALLBACK_REF] : [primaryRef, CODEX_FALLBACK_REF];
  const attempts = [];

  for (const [index, ref] of refs.entries()) {
    const url = rawCodexFileUrl(ref, filePath);
    try {
      const contents = await fetchText(url, { Accept: "text/plain,*/*;q=0.8" });
      return {
        ref,
        url,
        contents,
        attempts: [...attempts, { ref, url, ok: true }],
      };
    } catch (error) {
      const attempt = {
        ref,
        url,
        error: error.message,
        status: error.status ?? null,
      };
      attempts.push(attempt);
      const canFallbackToMain =
        index === 0 &&
        ref !== CODEX_FALLBACK_REF &&
        error instanceof FetchResponseError &&
        error.status === 404;
      if (canFallbackToMain) {
        continue;
      }
      throw new Error(
        `failed to fetch upstream Codex critical file ${filePath}; attempts: ${fetchAttemptSummary(attempts)}`,
      );
    }
  }

  throw new Error(`failed to fetch upstream Codex critical file ${filePath}; no refs attempted`);
}

function containsAllResult(tokens, contents) {
  const required = stringArray(tokens);
  return {
    required,
    found: required.filter((token) => contents.includes(token)),
    missing: required.filter((token) => !contents.includes(token)),
  };
}

function semanticChecksForFile(compat, filePath) {
  return (Array.isArray(compat?.semantic_checks) ? compat.semantic_checks : []).filter(
    (check) => check && typeof check === "object" && check.file === filePath,
  );
}

function buildCodexCriticalFileReport(file, compat, source, releaseTag) {
  const filePath = file.path;
  const requiredContains = containsAllResult(file.required_contains, source.contents);
  const semanticChecks = semanticChecksForFile(compat, filePath).map((check) => {
    const fileContains = containsAllResult(check[SEMANTIC_FILE_CONTAINS_FIELD], source.contents);
    return {
      id: check.id,
      kind: check.kind,
      reason: check.reason,
      [SEMANTIC_FILE_CONTAINS_FIELD]: fileContains.required,
      file_contains_found: fileContains.found,
      file_contains_missing: fileContains.missing,
    };
  });

  return {
    path: filePath,
    reason: file.reason,
    source_ref: source.ref,
    source_url: source.url,
    fallback_used: Boolean(releaseTag) && source.ref !== releaseTag,
    fetch_attempts: source.attempts.map(({ ref, url, ok, status, error }) => ({ ref, url, ok, status, error })),
    required_contains: requiredContains.required,
    required_contains_found: requiredContains.found,
    required_contains_missing: requiredContains.missing,
    semantic_checks: semanticChecks,
  };
}

async function fetchCodexCompatibility(compat, releaseTag) {
  const criticalFiles = Array.isArray(compat?.critical_files) ? compat.critical_files : [];
  const fetchableFiles = criticalFiles.filter((file) => file && typeof file.path === "string");
  const fetchedFiles = await Promise.all(
    fetchableFiles.map(async (file) => ({
      file,
      source: await fetchCodexRawFile(file.path, releaseTag),
    })),
  );
  const files = fetchedFiles.map(({ file, source }) =>
    buildCodexCriticalFileReport(file, compat, source, releaseTag),
  );

  return {
    upstream_repository: compat?.upstream_repository,
    baseline_source: compat?.baseline_source,
    source_ref_preferred: releaseTag || CODEX_FALLBACK_REF,
    source_ref_fallback: CODEX_FALLBACK_REF,
    critical_files: files,
  };
}

function diffField(pathLabel, baselineValue, currentValue) {
  if (JSON.stringify(baselineValue) === JSON.stringify(currentValue)) {
    return null;
  }
  return {
    path: pathLabel,
    baseline: baselineValue,
    current: currentValue,
  };
}

function formatList(value) {
  if (Array.isArray(value)) {
    return value
      .map((item) => `- ${typeof item === "string" ? item : JSON.stringify(item, null, 2)}`)
      .join("\n");
  }
  if (value && typeof value === "object") {
    return JSON.stringify(value, null, 2);
  }
  return String(value);
}

function renderReport(snapshot, diffs) {
  const lines = [];
  lines.push("Upstream compatibility watchdog");
  lines.push(`Generated at: ${snapshot.generated_at}`);
  lines.push("");
  lines.push(`Codex latest release: ${snapshot.codex.latestRelease.tag_name} (${snapshot.codex.latestRelease.name})`);
  if (snapshot.codex.compatibility?.critical_files?.length > 0) {
    lines.push("Codex critical files:");
    for (const file of snapshot.codex.compatibility.critical_files) {
      const fallback = file.fallback_used ? ` (fallback from ${snapshot.codex.compatibility.source_ref_preferred})` : "";
      lines.push(`- ${file.path}: ${file.source_ref}${fallback}`);
    }
  }
  lines.push(`Claude latest release: ${snapshot.claude.latestRelease.tag_name} (${snapshot.claude.latestRelease.name})`);
  lines.push("");
  if (diffs.length === 0) {
    lines.push("Status: in sync");
    return lines.join("\n") + "\n";
  }

  lines.push("Status: drift detected");
  for (const diff of diffs) {
    lines.push(`- ${diff.path}`);
    lines.push(`  baseline: ${formatList(diff.baseline)}`);
    lines.push(`  current: ${formatList(diff.current)}`);
  }
  return lines.join("\n") + "\n";
}

function buildDiffs(baseline, current) {
  const diffs = [];

  if (baseline.codex?.latestRelease) {
    const codexPaths = ["tag_name", "name", "published_at", "html_url"];
    for (const key of codexPaths) {
      if (!(key in baseline.codex.latestRelease)) {
        continue;
      }
      const diff = diffField(
        `codex.latestRelease.${key}`,
        baseline.codex.latestRelease[key],
        current.codex.latestRelease[key],
      );
      if (diff) {
        diffs.push(diff);
      }
    }
  }

  for (const baselineFile of baseline.codex?.compatibility?.critical_files ?? []) {
    const currentFile = current.codex.compatibility.critical_files.find((file) => file.path === baselineFile.path);
    if (!currentFile) {
      diffs.push({
        path: `codex.compatibility.critical_files.${baselineFile.path}`,
        baseline: "file present",
        current: "file missing from watchdog fetch set",
      });
      continue;
    }
    if (currentFile.required_contains_missing.length > 0) {
      diffs.push({
        path: `codex.compatibility.critical_files.${baselineFile.path}.required_contains`,
        baseline: baselineFile.required_contains,
        current: {
          source_ref: currentFile.source_ref,
          source_url: currentFile.source_url,
          missing: currentFile.required_contains_missing,
          found: currentFile.required_contains_found,
        },
      });
    }
  }

  for (const baselineCheck of baseline.codex?.compatibility?.semantic_checks ?? []) {
    if (!baselineCheck?.id || !baselineCheck?.file) {
      continue;
    }
    const currentFile = current.codex.compatibility.critical_files.find((file) => file.path === baselineCheck.file);
    const currentCheck = currentFile?.semantic_checks.find((check) => check.id === baselineCheck.id);
    if (!currentCheck) {
      diffs.push({
        path: `codex.compatibility.semantic_checks.${baselineCheck.id}`,
        baseline: "semantic check present",
        current: "semantic check missing from watchdog fetch set",
      });
      continue;
    }
    for (const field of SEMANTIC_DIFF_FIELDS) {
      const missing = currentCheck[field.missing] ?? [];
      if (missing.length === 0) {
        continue;
      }
      const currentDetails =
        field.source === "file"
          ? {
              file: baselineCheck.file,
              source_ref: currentFile.source_ref,
              source_url: currentFile.source_url,
              missing,
              found: currentCheck[field.found],
            }
          : {
              source_files: current.codex.compatibility.critical_files.map((file) => ({
                path: file.path,
                source_ref: file.source_ref,
              })),
              missing,
              found: currentCheck[field.found],
            };
      diffs.push({
        path: `codex.compatibility.semantic_checks.${baselineCheck.id}.${field.field}`,
        baseline: baselineCheck[field.field],
        current: currentDetails,
      });
    }
  }

  const claudePaths = ["tag_name", "name", "published_at", "html_url"];
  for (const key of claudePaths) {
    const diff = diffField(
      `claude.latestRelease.${key}`,
      baseline.claude.latestRelease[key],
      current.claude.latestRelease[key],
    );
    if (diff) {
      diffs.push(diff);
    }
  }

  for (const baselinePage of baseline.claude.docs) {
    const currentPage = current.claude.docs.find((page) => page.url === baselinePage.url);
    if (!currentPage) {
      diffs.push({
        path: `claude.docs.${baselinePage.url}`,
        baseline: "page present",
        current: "page missing",
      });
      continue;
    }
    for (const key of ["title", "description"]) {
      const diff = diffField(`claude.docs.${baselinePage.url}.${key}`, baselinePage[key], currentPage[key]);
      if (diff) {
        diffs.push(diff);
      }
    }
    if (currentPage.required_contains_missing.length > 0) {
      diffs.push({
        path: `claude.docs.${baselinePage.url}.required_contains`,
        baseline: baselinePage.required_contains,
        current: {
          missing: currentPage.required_contains_missing,
          found: currentPage.required_contains_found,
        },
      });
    }
  }

  return diffs;
}

function baselineSnapshotFromCurrent(baseline, current) {
  return {
    ...baseline,
    codex: {
      ...(baseline.codex ?? {}),
      latestRelease: current.codex.latestRelease,
    },
    claude: {
      latestRelease: current.claude.latestRelease,
      docs: current.claude.docs.map(
        ({ url, title, description, required_contains }) => ({
          url,
          title,
          description,
          required_contains,
        }),
      ),
    },
  };
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    process.stdout.write(
      [
        "Usage: node scripts/compat/watch-upstream.mjs [--baseline <path>] [--report <path>] [--write-baseline] [--json]",
        "",
        "Checks Codex and Claude Code upstream state against a saved compatibility baseline.",
      ].join("\n") + "\n",
    );
    return;
  }

  const baselineText = await fs.readFile(args.baseline, "utf8");
  const baseline = JSON.parse(baselineText);
  const codexRelease = await fetchCodexRelease();
  const current = {
    generated_at: new Date().toISOString(),
    codex: {
      latestRelease: codexRelease,
      compatibility: await fetchCodexCompatibility(baseline.codex?.compatibility, codexRelease.tag_name),
    },
    claude: {
      latestRelease: await fetchClaudeRelease(),
      docs: await Promise.all(baseline.claude.docs.map(fetchClaudeDoc)),
    },
  };
  const diffs = buildDiffs(baseline, current);

  if (args.writeBaseline) {
    await fs.writeFile(args.baseline, `${JSON.stringify(baselineSnapshotFromCurrent(baseline, current), null, 2)}\n`);
  }

  const report = {
    baselinePath: args.baseline,
    generated_at: current.generated_at,
    current,
    diffs,
  };

  if (args.report) {
    await fs.writeFile(args.report, `${JSON.stringify(report, null, 2)}\n`);
  }

  const rendered = renderReport(current, diffs);
  if (args.json) {
    process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
  } else {
    process.stdout.write(rendered);
  }

  if (diffs.length > 0) {
    process.exitCode = 1;
  }
}

try {
  await main();
} catch (error) {
  process.stderr.write(`Upstream compatibility watchdog failed: ${error.message}\n`);
  process.exitCode = 1;
}
