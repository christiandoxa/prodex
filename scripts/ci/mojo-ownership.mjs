#!/usr/bin/env node

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { repoRoot } from "../npm/common.mjs";

const manifestPath = path.join(repoRoot, "migration", "mojo-ownership.json");
const COUNTED_CLASSIFICATIONS = new Set(["DETERMINISTIC_DOMAIN", "MIXED"]);
const SEMANTIC_ROLES = new Set(["semantic", undefined]);
const sourceCache = new Map();

function parseArgs(argv) {
  const args = {
    baseline: null,
    check: false,
    json: false,
    releaseSha: "WORKTREE",
    selfTest: false,
  };
  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--baseline") {
      args.baseline = argv[++index];
      if (!args.baseline) throw new Error("--baseline requires a SHA");
      continue;
    }
    if (value === "--release-sha") {
      args.releaseSha = argv[++index];
      if (!args.releaseSha) throw new Error("--release-sha requires a SHA");
      continue;
    }
    if (value === "--check") {
      args.check = true;
      continue;
    }
    if (value === "--json") {
      args.json = true;
      continue;
    }
    if (value === "--self-test") {
      args.selfTest = true;
      continue;
    }
    throw new Error(`unknown argument: ${value}`);
  }
  return args;
}

function sourceText(manifest, revision, relativePath) {
  const cacheKey = `${revision}:${relativePath}`;
  if (sourceCache.has(cacheKey)) return sourceCache.get(cacheKey);
  if (revision === "WORKTREE") {
    const contents = readFileSync(path.join(repoRoot, relativePath), "utf8");
    sourceCache.set(cacheKey, contents);
    return contents;
  }
  try {
    const contents = execFileSync("git", ["show", `${revision}:${relativePath}`], {
      cwd: repoRoot,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"],
    });
    sourceCache.set(cacheKey, contents);
    return contents;
  } catch (error) {
    if (manifest.baseline_optional_sources?.includes(relativePath)) return "";
    throw error;
  }
}

function sourceExists(manifest, revision, relativePath) {
  try {
    sourceText(manifest, revision, relativePath);
    return true;
  } catch {
    return false;
  }
}

function isCommentOrDirective(line, language) {
  const trimmed = line.trim();
  if (!trimmed || trimmed.startsWith("//") || trimmed.startsWith("/*") || trimmed === "*") {
    return true;
  }
  if (language === "rust") {
    return trimmed.startsWith("*") || trimmed.startsWith("*/") || trimmed.startsWith("use ") ||
      trimmed.startsWith("pub use ") || trimmed.startsWith("#[") || trimmed.startsWith("#![");
  }
  return trimmed.startsWith("#") || trimmed.startsWith("from ") || trimmed.startsWith("import ") ||
    trimmed.startsWith("@export") || trimmed.startsWith("abi(\"");
}

function rustProductionLines(text) {
  const lines = text.split(/\r?\n/);
  const output = [];
  let skippedDepth = 0;
  let pendingSkip = false;
  for (const line of lines) {
    const trimmed = line.trim();
    if (skippedDepth > 0) {
      skippedDepth += (line.match(/{/g) ?? []).length;
      skippedDepth -= (line.match(/}/g) ?? []).length;
      continue;
    }
    if (trimmed.startsWith("#[cfg(") &&
        (trimmed.includes("test") || trimmed.includes("not(feature"))) {
      pendingSkip = true;
      continue;
    }
    if (pendingSkip) {
      if (trimmed.includes("{")) {
        skippedDepth = (line.match(/{/g) ?? []).length - (line.match(/}/g) ?? []).length;
        pendingSkip = false;
      }
      continue;
    }
    if (!isCommentOrDirective(line, "rust")) output.push(line);
  }
  return output;
}

export function countSemanticLines(text, language, ranges = []) {
  const lines = text.split(/\r?\n/);
  const selected = ranges.length === 0
    ? lines
    : ranges.flatMap(({ start, end }) => lines.slice(start - 1, end));
  if (language === "rust") return rustProductionLines(selected.join("\n")).length;
  return selected.filter((line) => !isCommentOrDirective(line, "mojo")).length;
}

function digest(text) {
  return createHash("sha256").update(text).digest("hex");
}

function readManifest() {
  return JSON.parse(readFileSync(manifestPath, "utf8"));
}

function legacyInventory(manifest, language) {
  const key = language === "rust" ? "rust_deterministic_sources" : "mojo_deterministic_sources";
  return (manifest[key] ?? []).map((filePath) => ({
    path: filePath,
    language,
    classification: "MIXED",
    production_reachable: true,
  }));
}

function entriesFor(manifest, kind, language) {
  const declared = manifest[`${kind}_inventory`];
  if (Array.isArray(declared)) return declared.filter((entry) => entry.language === language);
  if (declared?.inherit_baseline) {
    const baseline = manifest.baseline_inventory ?? [];
    const overrides = declared.overrides ?? {};
    const removed = new Set(declared.removed ?? []);
    const inherited = baseline.filter((entry) => !removed.has(entry.path)).map((entry) => ({
      ...entry,
      ...(overrides[entry.path] ?? {}),
    }));
    return [...inherited, ...(declared.additions ?? [])]
      .filter((entry) => entry.language === language);
  }
  return legacyInventory(manifest, language);
}

function inventory(manifest, revision, kind) {
  const isBaseline = kind === "baseline";
  const entries = [
    ...entriesFor(manifest, kind, "rust"),
    ...entriesFor(manifest, kind, "mojo"),
  ];
  const counts = { mojo: 0, rust: 0 };
  const seen = new Set();
  for (const entry of entries) {
    assert.equal(typeof entry.path, "string", `${kind} inventory entry needs path`);
    assert.equal(entry.language === "rust" || entry.language === "mojo", true,
      `${entry.path} has an invalid language`);
    assert(!seen.has(entry.path), `${kind} inventory contains duplicate ${entry.path}`);
    seen.add(entry.path);
    let contents;
    try {
      contents = sourceText(manifest, revision, entry.path);
    } catch (error) {
      const baselinePaths = new Set((manifest.baseline_inventory ?? []).map((item) => item.path));
      if (!isBaseline && revision === manifest.baseline_sha && !baselinePaths.has(entry.path)) {
        continue;
      }
      throw error;
    }
    if (isBaseline && entry.source_sha256) {
      assert.equal(digest(contents), entry.source_sha256, `${entry.path} baseline source changed`);
    }
    if (!COUNTED_CLASSIFICATIONS.has(entry.classification ?? "MIXED") ||
        entry.production_reachable === false || !SEMANTIC_ROLES.has(entry.role)) {
      continue;
    }
    const computed = countSemanticLines(contents, entry.language, entry.semantic_ranges ?? []);
    if (isBaseline && entry.semantic_loc !== undefined) {
      assert.equal(computed, entry.semantic_loc, `${entry.path} baseline semantic LOC changed`);
    }
    counts[entry.language] += isBaseline && entry.semantic_loc !== undefined
      ? entry.semantic_loc
      : computed;
  }
  const total = counts.mojo + counts.rust;
  return {
    mojo_loc: counts.mojo,
    mojo_percent: total === 0 ? 0 : (counts.mojo * 100) / total,
    rust_loc: counts.rust,
    total_loc: total,
  };
}

function selectedMojoSources(manifest, revision) {
  const build = sourceText(manifest, revision, "crates/prodex-mojo-core/build.rs");
  return [...build.matchAll(/\.\.\/\.\.\/(mojo\/prodex_core\/[A-Za-z0-9_]+\.mojo)/g)]
    .map((match) => match[1]);
}

function importedMojoSources(manifest, revision, root) {
  const reachable = new Set();
  const queue = [root];
  while (queue.length > 0) {
    const current = queue.pop();
    if (reachable.has(current)) continue;
    reachable.add(current);
    if (!sourceExists(manifest, revision, current)) continue;
    const contents = sourceText(manifest, revision, current);
    for (const [, module] of contents.matchAll(/^from\s+([A-Za-z0-9_]+)\s+import/gm)) {
      const imported = `mojo/prodex_core/${module}.mojo`;
      if (sourceExists(manifest, revision, imported)) queue.push(imported);
    }
  }
  return reachable;
}

function mojoProductionReachable(manifest, revision, source) {
  return selectedMojoSources(manifest, revision).some((root) =>
    importedMojoSources(manifest, revision, root).has(source));
}

function operationSource(manifest, operation) {
  const entry = [...entriesFor(manifest, "release", "mojo")].find((candidate) =>
    candidate.path === operation.mojo_source && candidate.language === "mojo");
  assert(entry, `${operation.name} Mojo source is absent from release inventory`);
  assert.notEqual(entry.production_reachable, false, `${operation.name} Mojo source is not marked reachable`);
  const operations = entry.operations ?? (entry.operation ? [entry.operation] : []);
  assert(operations.includes(operation.name), `${operation.name} must own its Mojo source entry`);
  return entry;
}

function validateOperations(manifest, revision) {
  const operations = manifest.authoritative_operations ?? [];
  assert(operations.length >= 6, "at least six Mojo operations are required");
  assert(new Set(operations.map((operation) => operation.domain.split("/")[0])).size >= 4,
    "Mojo operations must span at least four deterministic domains");
  const names = new Set();
  for (const operation of operations) {
    assert(!names.has(operation.name), `duplicate authoritative operation ${operation.name}`);
    names.add(operation.name);
    assert.equal(operation.final_state, "authoritative", `${operation.name} is not final-authoritative`);
    assert.equal(typeof operation.mojo_entry, "string", `${operation.name} needs a Mojo entry`);
    assert.equal(typeof operation.mojo_source, "string", `${operation.name} needs a Mojo source`);
    assert.equal(typeof operation.consumer, "string", `${operation.name} needs a production consumer`);
    assert.equal(typeof operation.production_reachability_test, "string",
      `${operation.name} needs a reachability test`);
    if (operation.baseline_state === "authoritative") {
      assert.notEqual(operation.introduced_in, manifest.release_target,
        `${operation.name} is baseline work, not a new release migration`);
    }
    if (revision === manifest.baseline_sha && operation.introduced_in === manifest.release_target) {
      continue;
    }
    operationSource(manifest, operation);
    const mojo = sourceText(manifest, revision, operation.mojo_source);
    assert(mojo.includes(`@export("${operation.mojo_entry}")`),
      `${operation.name} Mojo entry is not exported by its source`);
    assert(mojoProductionReachable(manifest, revision, operation.mojo_source),
      `${operation.name} Mojo source is not reachable from build.rs`);
    const consumer = sourceText(manifest, revision, operation.consumer);
    assert(consumer.includes("prodex_mojo_core"), `${operation.name} consumer does not call prodex-mojo-core`);
    if (operation.consumer_marker) {
      assert(consumer.includes(operation.consumer_marker),
        `${operation.name} consumer marker is missing`);
    }
    const reachabilityTest = sourceText(manifest, revision, operation.production_reachability_test);
    assert(reachabilityTest.includes("prodex_mojo_core") || reachabilityTest.includes(operation.mojo_entry),
      `${operation.name} reachability test does not exercise Mojo`);
  }
}

function validateInventory(manifest, baselineRevision, releaseRevision) {
  const baselineEntries = [
    ...entriesFor(manifest, "baseline", "rust"),
    ...entriesFor(manifest, "baseline", "mojo"),
  ];
  const releaseEntries = [
    ...entriesFor(manifest, "release", "rust"),
    ...entriesFor(manifest, "release", "mojo"),
  ];
  const baselinePaths = new Set(baselineEntries.map((entry) => entry.path));
  const releaseEntriesAtRevision = releaseEntries.filter((entry) =>
    !(releaseRevision === baselineRevision && !baselinePaths.has(entry.path) &&
      !sourceExists(manifest, releaseRevision, entry.path)));
  const releaseByPath = new Map(releaseEntriesAtRevision.map((entry) => [entry.path, entry]));
  const reductions = manifest.rust_semantic_reductions ?? [];
  const operationNames = new Set([
    ...(manifest.authoritative_operations ?? []).map((operation) => operation.name),
    ...(manifest.supporting_operations ?? []),
  ]);
  for (const baseline of baselineEntries) {
    if (!COUNTED_CLASSIFICATIONS.has(baseline.classification ?? "MIXED") ||
        baseline.production_reachable === false || !SEMANTIC_ROLES.has(baseline.role)) continue;
    const release = releaseByPath.get(baseline.path);
    if (release) continue;
    assert(reductions.some((reduction) => reduction.file === baseline.path),
      `baseline production source ${baseline.path} was removed from the release manifest without a Rust reduction record`);
    if (baseline.language === "rust") {
      assert(reductions.some((reduction) => reduction.file === baseline.path &&
        ["deleted", "adapter-only", "test-oracle-only"].includes(reduction.final_state)),
      `${baseline.path} removal is not a declared Rust semantic reduction`);
    }
  }
  for (const entry of releaseEntriesAtRevision.filter((candidate) => candidate.language === "mojo")) {
    if (entry.production_reachable === false || !COUNTED_CLASSIFICATIONS.has(entry.classification ?? "MIXED") ||
        !SEMANTIC_ROLES.has(entry.role)) continue;
    const operations = entry.operations ?? (entry.operation ? [entry.operation] : []);
    assert(operations.length > 0 && operations.every((operation) => operationNames.has(operation)),
      `${entry.path} is counted without a declared production operation`);
    assert(sourceExists(manifest, releaseRevision, entry.path), `${entry.path} is absent from release source`);
    assert(mojoProductionReachable(manifest, releaseRevision, entry.path),
      `${entry.path} is counted but not reachable from selected Mojo production sources`);
  }
  for (const reduction of reductions) {
    assert(typeof reduction.file === "string", "Rust reduction needs a file");
    assert(typeof reduction.symbol === "string", `${reduction.file} Rust reduction needs a symbol`);
    assert(typeof reduction.previous_responsibility === "string",
      `${reduction.file} Rust reduction needs its previous responsibility`);
    assert(["deleted", "adapter-only", "test-oracle-only"].includes(reduction.final_state),
      `${reduction.file}:${reduction.symbol} has an invalid final Rust state`);
    if (releaseRevision === baselineRevision && !sourceExists(manifest, releaseRevision, reduction.file)) {
      continue;
    }
    const source = sourceText(manifest, releaseRevision, reduction.file);
    assert(source.includes(reduction.symbol),
      `${reduction.file}:${reduction.symbol} reduction is not traceable in release source`);
  }
  validateOperations(manifest, releaseRevision);
  return { baselineEntries, releaseEntries };
}

export function validateManifest(manifest, baselineRevision = manifest.baseline_sha, releaseRevision = "WORKTREE") {
  if (!manifest.baseline_inventory) return;
  assert.equal(manifest.baseline_sha, baselineRevision,
    `requested baseline ${baselineRevision} differs from frozen manifest baseline ${manifest.baseline_sha}`);
  validateInventory(manifest, baselineRevision, releaseRevision);
}

export function calculateOwnership(manifest, baselineRevision, releaseRevision) {
  const strict = Boolean(manifest.baseline_inventory);
  if (strict) validateManifest(manifest, baselineRevision, releaseRevision);
  const baseline = inventory(manifest, baselineRevision, "baseline");
  const final = inventory(manifest, releaseRevision, "release");
  const operations = manifest.authoritative_operations ?? [];
  const available = (operation) => {
    if (!sourceExists(manifest, releaseRevision, operation.mojo_source)) return false;
    const source = sourceText(manifest, releaseRevision, operation.mojo_source);
    return source.includes(`@export("${operation.mojo_entry}")`);
  };
  const authoritative = operations.filter((operation) =>
    operation.final_state === "authoritative" && available(operation) &&
    (releaseRevision !== baselineRevision || operation.baseline_state === "authoritative"));
  const baselineAuthoritative = operations.filter((operation) =>
    operation.baseline_state === "authoritative" &&
    sourceExists(manifest, baselineRevision, operation.mojo_source) &&
    sourceText(manifest, baselineRevision, operation.mojo_source)
      .includes(`@export("${operation.mojo_entry}")`));
  const newOperations = releaseRevision === baselineRevision
    ? []
    : authoritative.filter((operation) => operation.introduced_in === manifest.release_target);
  const expandedOperations = releaseRevision === baselineRevision
    ? []
    : authoritative.filter((operation) => operation.expanded_in === manifest.release_target);
  const migrationUnits = authoritative.filter((operation) =>
    operation.introduced_in === manifest.release_target || operation.expanded_in === manifest.release_target);
  return {
    baseline,
    final,
    percentage_point_increase: final.mojo_percent - baseline.mojo_percent,
    authoritative_operation_count: authoritative.length,
    baseline_authoritative_operation_count: baselineAuthoritative.length,
    new_migration_unit_count: migrationUnits.length,
    new_authoritative_operations: newOperations,
    expanded_authoritative_operations: expandedOperations,
    authoritative_operations: operations,
  };
}

export function ownershipMeetsMinimum(result, minimumPercent) {
  return result.final.mojo_percent >= minimumPercent;
}

function selfTest() {
  assert.equal(countSemanticLines("// comment\nuse std::x;\n#[cfg(test)]\nmod tests {\nfn ignored() {}\n}\nfn production() {}\n", "rust"), 1);
  assert.equal(countSemanticLines("# comment\nfrom std import Pointer\n@export(\"x\")\ndef production():\n    return 1\n", "mojo"), 2);
  const result = calculateOwnership(
    {
      rust_deterministic_sources: [],
      mojo_deterministic_sources: [],
      authoritative_operations: [],
    },
    "HEAD",
    "HEAD",
  );
  assert.equal(result.final.mojo_percent, 0);
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.selfTest) selfTest();
  const manifest = readManifest();
  const baseline = args.baseline ?? manifest.baseline_sha;
  const result = calculateOwnership(manifest, baseline, args.releaseSha);
  if (args.check && !ownershipMeetsMinimum(result, manifest.minimum_percent)) {
    throw new Error(
      `Mojo deterministic ownership ${result.final.mojo_percent.toFixed(2)}% is below ${manifest.minimum_percent}%`,
    );
  }
  if (args.check && manifest.baseline_inventory && result.new_migration_unit_count < 8) {
    throw new Error(`Mojo migration has ${result.new_migration_unit_count} new units; at least 8 are required`);
  }
  if (args.json) {
    process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
    return;
  }
  process.stdout.write(
    [
      `baseline: ${result.baseline.mojo_percent.toFixed(2)}% Mojo (${result.baseline.mojo_loc}/${result.baseline.total_loc})`,
      `final: ${result.final.mojo_percent.toFixed(2)}% Mojo (${result.final.mojo_loc}/${result.final.total_loc})`,
      `increase: ${result.percentage_point_increase.toFixed(2)} percentage points`,
      `authoritative operations: ${result.authoritative_operation_count}`,
    ].join("\n") + "\n",
  );
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    process.stderr.write(`mojo-ownership: ${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  });
}
