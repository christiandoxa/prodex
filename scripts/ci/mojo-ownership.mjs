#!/usr/bin/env node

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { repoRoot } from "../npm/common.mjs";

const manifestPath = path.join(repoRoot, "migration", "mojo-ownership.json");
const COUNTING_RULES_VERSION = 1;
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

function canonicalJson(value) {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
  if (value && typeof value === "object") {
    return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(",")}}`;
  }
  return JSON.stringify(value);
}

function digestJson(value) {
  return digest(canonicalJson(value));
}

function isEligible(entry) {
  return COUNTED_CLASSIFICATIONS.has(entry.classification ?? "MIXED") &&
    entry.production_reachable !== false && SEMANTIC_ROLES.has(entry.role);
}

function accountingShape(entry) {
  return {
    classification: entry.classification ?? "MIXED",
    language: entry.language,
    production_reachable: entry.production_reachable !== false,
    role: entry.role ?? null,
    semantic_ranges: entry.semantic_ranges ?? [],
  };
}

function requiredMigrationVolume(rustLoc, ratePercent) {
  assert(Number.isInteger(rustLoc) && rustLoc >= 0, "baseline Rust semantic LOC must be non-negative");
  assert(Number.isInteger(ratePercent) && ratePercent >= 0 && ratePercent <= 100,
    "migration rate must be an integer percentage from 0 to 100");
  return Math.ceil((rustLoc * ratePercent) / 100);
}

function snapshotPath(manifest) {
  assert.equal(typeof manifest.baseline_snapshot, "string", "frozen baseline snapshot is required");
  return path.join(repoRoot, manifest.baseline_snapshot);
}

function readBaselineSnapshot(manifest) {
  return JSON.parse(readFileSync(snapshotPath(manifest), "utf8"));
}

function revisionTree(revision) {
  return execFileSync("git", ["rev-parse", `${revision}^{tree}`], {
    cwd: repoRoot,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "ignore"],
  }).trim();
}

function baselineOperations(manifest) {
  return (manifest.authoritative_operations ?? [])
    .filter((operation) => operation.baseline_state === "authoritative");
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
    if (!isEligible(entry)) continue;
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

function semanticLocAtRevision(manifest, revision, entry) {
  if (!sourceExists(manifest, revision, entry.path)) return 0;
  return countSemanticLines(
    sourceText(manifest, revision, entry.path),
    entry.language,
    entry.semantic_ranges ?? [],
  );
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

function validateOperationContinuity(manifest, snapshot) {
  const expectedNames = snapshot.baseline_authoritative_operations;
  assert(Array.isArray(expectedNames) && expectedNames.length > 0,
    "frozen baseline needs authoritative operation names");
  const operations = new Map((manifest.authoritative_operations ?? [])
    .map((operation) => [operation.name, operation]));
  for (const name of expectedNames) {
    const operation = operations.get(name);
    assert(operation, `baseline authoritative operation ${name} is missing from the release manifest`);
    assert.equal(operation.baseline_state, "authoritative",
      `baseline authoritative operation ${name} lost continuity`);
    assert.equal(operation.final_state, "authoritative",
      `baseline authoritative operation ${name} is not authoritative in the release`);
  }
  assert.deepEqual(
    baselineOperations(manifest).map((operation) => operation.name),
    expectedNames,
    "baseline authoritative operation set changed",
  );
  assert.equal(
    digestJson(baselineOperations(manifest)),
    snapshot.baseline_operations_sha256,
    "baseline authoritative operation contract changed",
  );
}

function validateBaselineSnapshot(manifest, baselineRevision, baseline) {
  const snapshot = readBaselineSnapshot(manifest);
  assert.equal(snapshot.schema_version, 1, "unsupported frozen baseline snapshot schema");
  assert.equal(snapshot.baseline_sha, manifest.baseline_sha, "snapshot baseline SHA does not match manifest");
  assert.equal(snapshot.release_target, manifest.release_target,
    "snapshot release target does not match manifest");
  assert.equal(snapshot.baseline_tree, revisionTree(baselineRevision), "frozen baseline tree changed");
  assert.equal(snapshot.counting_rules_version, COUNTING_RULES_VERSION,
    "frozen baseline uses a different LOC counting rule version");
  assert.equal(snapshot.baseline_inventory_sha256, digestJson(manifest.baseline_inventory),
    "frozen baseline inventory changed");
  const report = snapshot.baseline_report;
  assert(report, "frozen baseline report is required");
  assert.equal(report.eligible_rust_deterministic_production_semantic_loc, baseline.rust_loc,
    "frozen baseline Rust semantic LOC changed");
  assert.equal(report.eligible_mojo_deterministic_production_semantic_loc, baseline.mojo_loc,
    "frozen baseline Mojo semantic LOC changed");
  assert.equal(report.total_semantic_loc, baseline.total_loc, "frozen baseline total semantic LOC changed");
  assert.equal(report.mojo_percent, baseline.mojo_percent, "frozen baseline ownership percentage changed");
  assert.equal(report.migration_volume_loc, baseline.mojo_loc,
    "frozen baseline migration volume must equal eligible Mojo semantic LOC");
  const ratePercent = manifest.migration_rate_percent;
  assert.equal(snapshot.migration_rate_percent, ratePercent, "frozen migration rate changed");
  const requiredVolume = requiredMigrationVolume(baseline.rust_loc, ratePercent);
  assert.equal(snapshot.required_migration_volume_loc, requiredVolume, "frozen migration-volume floor changed");
  assert.equal(manifest.minimum_migration_loc, requiredVolume, "manifest migration-volume floor changed");
  validateOperationContinuity(manifest, snapshot);
  return snapshot;
}

function reductionFor(reductions, file) {
  return reductions.find((reduction) => reduction.file === file);
}

function requireReduction(reductions, file, message) {
  const reduction = reductionFor(reductions, file);
  assert(reduction, message);
  assert(["deleted", "adapter-only", "test-oracle-only"].includes(reduction.final_state),
    `${file} has an invalid Rust semantic reduction state`);
  return reduction;
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
    if (!isEligible(baseline)) continue;
    const release = releaseByPath.get(baseline.path);
    if (!release) {
      const reduction = requireReduction(
        reductions,
        baseline.path,
        `baseline production source ${baseline.path} was removed from the release manifest without a Rust reduction record`,
      );
      if (baseline.language === "rust") {
        assert(["deleted", "adapter-only", "test-oracle-only"].includes(reduction.final_state),
          `${baseline.path} removal is not a declared Rust semantic reduction`);
      }
      continue;
    }
    const releaseLoc = semanticLocAtRevision(manifest, releaseRevision, release);
    const accountingChanged = canonicalJson(accountingShape(baseline)) !==
      canonicalJson(accountingShape(release));
    if (baseline.language === "rust" && accountingChanged) {
      requireReduction(
        reductions,
        baseline.path,
        `${baseline.path} changed its eligibility without a traceable reduction`,
      );
      assert(releaseLoc < baseline.semantic_loc,
        `${baseline.path} eligibility change is not backed by a semantic LOC reduction`);
    }
    if (baseline.language === "rust" && releaseLoc < baseline.semantic_loc) {
      requireReduction(
        reductions,
        baseline.path,
        `${baseline.path} semantic LOC decreased without a traceable Rust reduction`,
      );
    }
  }
  for (const entry of releaseEntriesAtRevision.filter((candidate) => candidate.language === "mojo")) {
    if (!isEligible(entry)) continue;
    const operations = entry.operations ?? (entry.operation ? [entry.operation] : []);
    assert(operations.length > 0 && operations.every((operation) => operationNames.has(operation)),
      `${entry.path} is counted without a declared production operation`);
    assert(sourceExists(manifest, releaseRevision, entry.path), `${entry.path} is absent from release source`);
    assert(mojoProductionReachable(manifest, releaseRevision, entry.path),
      `${entry.path} is counted but not reachable from selected Mojo production sources`);
  }
  for (const reduction of reductions) {
    assert(typeof reduction.file === "string", "Rust reduction needs a file");
    assert(reduction.file.endsWith(".rs"), `${reduction.file} Rust reduction must point to Rust source`);
    assert(typeof reduction.symbol === "string", `${reduction.file} Rust reduction needs a symbol`);
    assert(typeof reduction.operation === "string", `${reduction.file} Rust reduction needs an operation`);
    assert(operationNames.has(reduction.operation),
      `${reduction.file}:${reduction.symbol} names an unknown operation`);
    assert(typeof reduction.previous_responsibility === "string",
      `${reduction.file} Rust reduction needs its previous responsibility`);
    assert(["deleted", "adapter-only", "test-oracle-only"].includes(reduction.final_state),
      `${reduction.file}:${reduction.symbol} has an invalid final Rust state`);
    assert(sourceExists(manifest, baselineRevision, reduction.file),
      `${reduction.file}:${reduction.symbol} is not traceable in the frozen baseline source`);
    const baselineSource = sourceText(manifest, baselineRevision, reduction.file);
    assert(baselineSource.includes(reduction.symbol),
      `${reduction.file}:${reduction.symbol} is not traceable in the frozen baseline source`);
    if (releaseRevision === baselineRevision && !sourceExists(manifest, releaseRevision, reduction.file)) {
      continue;
    }
    if (reduction.final_state === "deleted" && !sourceExists(manifest, releaseRevision, reduction.file)) continue;
    assert(sourceExists(manifest, releaseRevision, reduction.file),
      `${reduction.file}:${reduction.symbol} reduction source is absent from release`);
    const source = sourceText(manifest, releaseRevision, reduction.file);
    if (reduction.final_state === "deleted") {
      assert(!source.includes(reduction.symbol),
        `${reduction.file}:${reduction.symbol} deleted reduction remains in release source`);
    } else {
      assert(source.includes(reduction.symbol),
        `${reduction.file}:${reduction.symbol} reduction is not traceable in release source`);
    }
  }
  validateOperations(manifest, releaseRevision);
  return { baselineEntries, releaseEntries };
}

export function validateManifest(manifest, baselineRevision = manifest.baseline_sha, releaseRevision = "WORKTREE") {
  if (!manifest.baseline_inventory) return;
  assert.equal(manifest.baseline_sha, baselineRevision,
    `requested baseline ${baselineRevision} differs from frozen manifest baseline ${manifest.baseline_sha}`);
  const baseline = inventory(manifest, baselineRevision, "baseline");
  validateBaselineSnapshot(manifest, baselineRevision, baseline);
  validateInventory(manifest, baselineRevision, releaseRevision);
  return baseline;
}

export function calculateOwnership(manifest, baselineRevision, releaseRevision) {
  const strict = Boolean(manifest.baseline_inventory);
  const baseline = strict
    ? validateManifest(manifest, baselineRevision, releaseRevision)
    : inventory(manifest, baselineRevision, "baseline");
  const final = inventory(manifest, releaseRevision, "release");
  if (strict) {
    assert(final.mojo_loc >= baseline.mojo_loc,
      `Mojo semantic ownership regressed from ${baseline.mojo_loc} to ${final.mojo_loc} LOC`);
  }
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
  const requiredMigrationVolumeLoc = strict
    ? requiredMigrationVolume(baseline.rust_loc, manifest.migration_rate_percent)
    : 0;
  return {
    baseline,
    final,
    percentage_point_increase: final.mojo_percent - baseline.mojo_percent,
    migration_volume_loc: final.mojo_loc,
    new_migration_volume_loc: Math.max(0, final.mojo_loc - baseline.mojo_loc),
    required_migration_volume_loc: requiredMigrationVolumeLoc,
    rust_semantic_reduction_loc: Math.max(0, baseline.rust_loc - final.rust_loc),
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
  if (args.check && manifest.baseline_inventory &&
      result.migration_volume_loc < result.required_migration_volume_loc) {
    throw new Error(
      `Mojo migration volume ${result.migration_volume_loc} LOC is below the required ` +
      `${result.required_migration_volume_loc} LOC floor`,
    );
  }
  if (args.json) {
    process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
    return;
  }
  process.stdout.write(
    [
      `baseline: ${result.baseline.mojo_percent.toFixed(2)}% Mojo (${result.baseline.mojo_loc}/${result.baseline.total_loc})`,
      `final: ${result.final.mojo_percent.toFixed(2)}% Mojo (${result.final.mojo_loc}/${result.final.total_loc})`,
      `migration volume: ${result.migration_volume_loc} LOC (required ${result.required_migration_volume_loc})`,
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
