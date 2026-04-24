#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { repoRoot } from "../npm/common.mjs";

const DEFAULT_BASELINE_PATH = path.join(repoRoot, "scripts/compat/upstream-baseline.json");

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
  const response = await fetch(url, {
    headers: {
      "User-Agent": "prodex-compat-watchdog",
      Accept: "text/html,application/json;q=0.9,*/*;q=0.8",
      ...headers,
    },
  });
  if (!response.ok) {
    throw new Error(`failed to fetch ${url}: ${response.status} ${response.statusText}`);
  }
  return response.text();
}

async function fetchJson(url, headers = {}) {
  const response = await fetch(url, {
    headers: {
      "User-Agent": "prodex-compat-watchdog",
      Accept: "application/vnd.github+json",
      ...headers,
    },
  });
  if (!response.ok) {
    throw new Error(`failed to fetch ${url}: ${response.status} ${response.statusText}`);
  }
  return response.json();
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
    ...(baseline.codex ? { codex: baseline.codex } : {}),
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
        "Checks Claude Code releases/docs against a saved baseline and reports the current Codex release.",
      ].join("\n") + "\n",
    );
    return;
  }

  const baselineText = await fs.readFile(args.baseline, "utf8");
  const baseline = JSON.parse(baselineText);
  const current = {
    generated_at: new Date().toISOString(),
    codex: {
      latestRelease: await fetchCodexRelease(),
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

await main();
