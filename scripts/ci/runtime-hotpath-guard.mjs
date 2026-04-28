#!/usr/bin/env node
import fs from "node:fs/promises";
import path from "node:path";
import { repoRoot } from "../npm/common.mjs";

const DEFAULT_SCAN_TARGETS = Object.freeze([
  "src/runtime_proxy",
  "src/runtime_launch/proxy_startup.rs",
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
    id: "spawn-blocking",
    description: "blocking task pool use in runtime hot path",
    pattern: /\bspawn_blocking\s*\(/,
  },
]);

const ALLOWLIST = Object.freeze([
  {
    name: "chain-log-metadata-fingerprint",
    file: "src/runtime_proxy/chain_log.rs",
    id: "blocking-disk-io",
    pattern: /\bfs::metadata\s*\(/,
    maxHits: 2,
    reason: "bounded continuity metrics fingerprint cache; no broad reads/writes",
  },
  {
    name: "launch-worker-pool-threads",
    file: "src/runtime_launch/proxy_startup.rs",
    id: "blocking-thread-spawn",
    pattern: /\bthread::spawn\s*\(/,
    maxHits: 2,
    reason: "bounded proxy worker pools created during launch, outside request commit path",
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

async function collectScanFiles(targets) {
  const files = [];
  for (const target of targets) {
    files.push(...(await rustFilesUnder(normalizePath(target))));
  }
  return [...new Set(files)].sort();
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

  const targets = args.targets.length > 0 ? args.targets : [...DEFAULT_SCAN_TARGETS];
  const files = await collectScanFiles(targets);
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
