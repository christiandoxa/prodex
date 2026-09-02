#!/usr/bin/env node

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { cargoTomlPath, parseCargoVersion, repoRoot } from "../npm/common.mjs";

const manifestPath = path.join(repoRoot, "migration", "mojo-production-share.json");
const COUNTING_RULES_VERSION = 1;
const REQUIRED_BASELINE_SHA = "2531c7a345f1607a18aa926e204b4d02cc322167";
const REQUIRED_MOJO_NON_REGRESSION_BASELINE_SHA = "43768659073cc1ab5c5686d3d58f2af68eebdef2";
const REQUIRED_HISTORICAL_RELEASE_TARGET = "0.421.0";
const REQUIRED_RELEASE_FLOOR_PERCENT = 7;
const REQUIRED_PROJECT_TARGET_PERCENT = 10;
const REQUIRED_PRODUCTION_BUILD_FEATURE = "mojo-core";
const DEFAULT_SNAPSHOT_PATH = "migration/mojo-production-share-baseline-0.419.2.json";
const EXCLUDED_COMPONENTS = /^(?:tests?|benches|examples|fixtures?|snapshots|generated|vendor|target|fuzz|test-support|bench_support|prodex-bench-support)$/iu;
const TEST_COMPONENT = /(?:^|[_-])tests?(?:$|[_-])|^testing$/iu;
const PRODUCTION_FEATURES = new Map([
  ["mojo", "true"],
  ["mojo-core", "true"],
  ["mojo-quota", "true"],
  ["mojo-runtime", "true"],
  ["mojo-routing", "true"],
  ["mojo-rich", "true"],
  ["mojo-provider-constraints", "true"],
  ["bench-support", "false"],
  ["allocation-bench-support", "false"],
]);
const sourceCache = new Map();
const revisionArchiveCache = new Map();

function parseArgs(argv) {
  const args = {
    baseline: null,
    check: false,
    json: false,
    releaseSha: "WORKTREE",
    selfTest: false,
    writeSnapshot: false,
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
    if (value === "--write-snapshot") {
      args.writeSnapshot = true;
      continue;
    }
    throw new Error(`unknown argument: ${value}`);
  }
  return args;
}

export function validateManifestMetadata(manifest) {
  if (manifest.schema_version !== 1) {
    throw new Error("unsupported broad production-share manifest schema");
  }
  if (manifest.baseline_sha !== REQUIRED_BASELINE_SHA) {
    throw new Error(`broad production-share baseline must be ${REQUIRED_BASELINE_SHA}`);
  }
  if (manifest.release_target !== REQUIRED_HISTORICAL_RELEASE_TARGET) {
    throw new Error(`historical broad production-share release target must be ${REQUIRED_HISTORICAL_RELEASE_TARGET}`);
  }
  if (manifest.counting_rules_version !== COUNTING_RULES_VERSION) {
    throw new Error(`broad production-share counting rules must be ${COUNTING_RULES_VERSION}`);
  }
  if (manifest.release_floor_percent !== REQUIRED_RELEASE_FLOOR_PERCENT) {
    throw new Error(`broad production-share release floor must remain ${REQUIRED_RELEASE_FLOOR_PERCENT}%`);
  }
  if (manifest.project_target_percent !== REQUIRED_PROJECT_TARGET_PERCENT) {
    throw new Error(`broad production-share project target must remain ${REQUIRED_PROJECT_TARGET_PERCENT}%`);
  }
  if (manifest.release_floor_percent >= manifest.project_target_percent) {
    throw new Error("broad production-share release floor must be below project target");
  }
  const nonRegression = manifest.mojo_non_regression;
  if (!nonRegression || typeof nonRegression !== "object" || Array.isArray(nonRegression)) {
    throw new Error("Mojo production non-regression policy is required");
  }
  if (nonRegression.baseline_sha !== REQUIRED_MOJO_NON_REGRESSION_BASELINE_SHA) {
    throw new Error(
      `Mojo production non-regression baseline must be ${REQUIRED_MOJO_NON_REGRESSION_BASELINE_SHA}`,
    );
  }
  if (typeof nonRegression.baseline_sha !== "string" || !/^[0-9a-f]{40}$/u.test(nonRegression.baseline_sha)) {
    throw new Error("Mojo production non-regression baseline must be a full commit SHA");
  }
  if (typeof nonRegression.baseline_source_inventory_sha256 !== "string" ||
      !/^[0-9a-f]{64}$/u.test(nonRegression.baseline_source_inventory_sha256)) {
    throw new Error("Mojo production non-regression baseline inventory digest must be a SHA-256");
  }
  if (!Number.isInteger(nonRegression.baseline_mojo_production_loc) ||
      nonRegression.baseline_mojo_production_loc < 0) {
    throw new Error("Mojo production non-regression baseline Mojo LOC must be non-negative");
  }
  if (!Array.isArray(nonRegression.approved_reductions)) {
    throw new Error("Mojo production non-regression reductions must be an array");
  }
  const reductionPaths = new Set();
  for (const reduction of nonRegression.approved_reductions) {
    if (!reduction || typeof reduction !== "object" || Array.isArray(reduction) ||
        typeof reduction.path !== "string" || typeof reduction.reason !== "string" ||
        reduction.reason.trim() === "") {
      throw new Error("Mojo production non-regression reductions need path and reason");
    }
    if (reductionPaths.has(reduction.path)) {
      throw new Error(`duplicate Mojo production non-regression reduction ${reduction.path}`);
    }
    reductionPaths.add(reduction.path);
  }
  if (manifest.production_build_feature !== REQUIRED_PRODUCTION_BUILD_FEATURE) {
    throw new Error(
      `broad production-share build feature must be ${REQUIRED_PRODUCTION_BUILD_FEATURE}`,
    );
  }
  const waiver = manifest.temporary_release_waiver;
  if (waiver !== undefined) {
    if (!waiver || typeof waiver !== "object" || Array.isArray(waiver)) {
      throw new Error("temporary release waiver must be an object");
    }
    if (waiver.release_target !== REQUIRED_HISTORICAL_RELEASE_TARGET) {
      throw new Error("temporary release waiver release target must be 0.421.0");
    }
    if (waiver.baseline_sha !== manifest.baseline_sha || waiver.baseline_sha !== REQUIRED_BASELINE_SHA) {
      throw new Error("temporary release waiver baseline SHA does not match the frozen baseline");
    }
    if (waiver.temporary_release_floor_percent !== REQUIRED_RELEASE_FLOOR_PERCENT) {
      throw new Error("temporary release waiver floor must be exactly 7%");
    }
    if (waiver.scope !== "0.421.0 only" || waiver.expiration !== "immediately after 0.421.0" ||
        waiver.status !== "expired") {
      throw new Error("temporary release waiver scope or expiration is invalid");
    }
    if (typeof waiver.reason !== "string" || waiver.reason.trim() === "") {
      throw new Error("temporary release waiver reason is required");
    }
  }
  return manifest;
}

function readManifest() {
  return validateManifestMetadata(JSON.parse(fs.readFileSync(manifestPath, "utf8")));
}

function canonicalJson(value) {
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`;
  if (value && typeof value === "object") {
    return `{${Object.keys(value).sort().map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`).join(",")}}`;
  }
  return JSON.stringify(value);
}

function digest(value) {
  return createHash("sha256").update(value).digest("hex");
}

function digestJson(value) {
  return digest(canonicalJson(value));
}

function git(args) {
  return execFileSync("git", args, {
    cwd: repoRoot,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "ignore"],
  });
}

function sourceText(revision, relativePath) {
  const cacheKey = `${revision}:${relativePath}`;
  if (sourceCache.has(cacheKey)) return sourceCache.get(cacheKey);
  const contents = revision === "WORKTREE"
    ? fs.readFileSync(path.join(repoRoot, relativePath), "utf8")
    : revisionArchive(revision).get(relativePath);
  if (contents === undefined) throw new Error(`source is absent at ${revision}: ${relativePath}`);
  sourceCache.set(cacheKey, contents);
  return contents;
}

function sourceExists(revision, relativePath) {
  try {
    sourceText(revision, relativePath);
    return true;
  } catch {
    return false;
  }
}

function sourcePaths(revision) {
  const paths = revision === "WORKTREE"
    ? git(["ls-files", "--cached", "--others", "--exclude-standard", "-z"]).split("\0").filter(Boolean)
    : [...revisionArchive(revision).keys()];
  return [...new Set(paths)]
    .filter((relativePath) => revision !== "WORKTREE" || fs.existsSync(path.join(repoRoot, relativePath)))
    .sort((left, right) => left < right ? -1 : left > right ? 1 : 0);
}

function stripComments(line, state) {
  let output = "";
  let inString = false;
  let escaped = false;
  for (let index = 0; index < line.length; index += 1) {
    const character = line[index];
    const next = line[index + 1];
    if (state.blockComment) {
      if (character === "*" && next === "/") {
        state.blockComment = false;
        index += 1;
      }
      continue;
    }
    if (inString) {
      output += character;
      if (escaped) escaped = false;
      else if (character === "\\") escaped = true;
      else if (character === '"') inString = false;
      continue;
    }
    if (character === '"') {
      inString = true;
      output += character;
    } else if (character === "/" && next === "*") {
      state.blockComment = true;
      index += 1;
    } else if (character === "/" && next === "/") {
      break;
    } else {
      output += character;
    }
  }
  return output;
}

function splitTopLevel(value) {
  const parts = [];
  let depth = 0;
  let start = 0;
  let inString = false;
  let escaped = false;
  for (let index = 0; index < value.length; index += 1) {
    const character = value[index];
    if (inString) {
      if (escaped) escaped = false;
      else if (character === "\\") escaped = true;
      else if (character === '"') inString = false;
      continue;
    }
    if (character === '"') inString = true;
    else if (character === "(") depth += 1;
    else if (character === ")") depth -= 1;
    else if (character === "," && depth === 0) {
      parts.push(value.slice(start, index).trim());
      start = index + 1;
    }
  }
  parts.push(value.slice(start).trim());
  return parts.filter(Boolean);
}

function cfgState(expression) {
  const value = expression.trim();
  const feature = value.match(/^feature\s*=\s*["']([^"']+)["']$/u);
  if (feature) return PRODUCTION_FEATURES.get(feature[1]) ?? "unknown";
  if (value === "test") return "false";
  if (value === "unix" || value === "windows" || value.startsWith("target_") || value === "debug_assertions") {
    return "unknown";
  }
  const call = value.match(/^(any|all|not)\((.*)\)$/su);
  if (!call) return "unknown";
  const states = splitTopLevel(call[2]).map(cfgState);
  if (call[1] === "not") {
    if (states.length !== 1) return "unknown";
    return states[0] === "true" ? "false" : states[0] === "false" ? "true" : "unknown";
  }
  if (call[1] === "any") {
    if (states.includes("true")) return "true";
    return states.every((state) => state === "false") ? "false" : "unknown";
  }
  if (states.includes("false")) return "false";
  return states.every((state) => state === "true") ? "true" : "unknown";
}

function braceDelta(line) {
  return line
    .replace(/"(?:\\.|[^"\\])*"/gu, "")
    .replace(/'(?:\\.|[^'\\])*'/gu, "")
    .split("")
    .reduce((delta, character) => delta + (character === "{" ? 1 : character === "}" ? -1 : 0), 0);
}

function isRustImportLine(line) {
  return /^(?:(?:pub(?:\([^)]*\))?\s+)?use|extern crate)\b/u.test(line);
}

function consumeLeadingAttributes(line) {
  let remainder = line.trim();
  const attributes = [];
  while (remainder.startsWith("#[")) {
    const end = remainder.indexOf("]");
    if (end < 0) return { attributes, incomplete: true, remainder: "" };
    attributes.push(remainder.slice(0, end + 1));
    remainder = remainder.slice(end + 1).trim();
  }
  return { attributes, incomplete: false, remainder };
}

function rustCfgDisabled(attribute) {
  const match = attribute.match(/^#\[cfg\((.*)\)\]$/su);
  return match ? cfgState(match[1]) === "false" : false;
}

export function countRustProductionLines(text) {
  let count = 0;
  let excludedDepth = 0;
  let pendingExcludedItem = false;
  let pendingAttribute = "";
  let skippingImport = false;
  const commentState = { blockComment: false };
  for (const rawLine of text.replace(/\r\n?/gu, "\n").split("\n")) {
    const line = stripComments(rawLine, commentState);
    if (skippingImport) {
      if (line.includes(";")) skippingImport = false;
      continue;
    }
    if (excludedDepth > 0) {
      excludedDepth += braceDelta(line);
      if (excludedDepth < 0) excludedDepth = 0;
      continue;
    }
    const attributeInput = pendingAttribute + line.trim();
    const parsed = consumeLeadingAttributes(attributeInput);
    if (pendingAttribute || line.trim().startsWith("#[")) {
      if (parsed.incomplete) {
        pendingAttribute = attributeInput;
        continue;
      }
      pendingAttribute = "";
      pendingExcludedItem ||= parsed.attributes.some(rustCfgDisabled);
    }
    if (pendingExcludedItem) {
      if (!parsed.remainder) continue;
      const delta = braceDelta(parsed.remainder);
      if (delta > 0) excludedDepth = delta;
      else if (isRustImportLine(parsed.remainder) && !parsed.remainder.includes(";")) skippingImport = true;
      pendingExcludedItem = false;
      continue;
    }
    if (isRustImportLine(parsed.remainder)) {
      if (!parsed.remainder.includes(";")) skippingImport = true;
      continue;
    }
    if (parsed.remainder.length > 0 && !parsed.remainder.startsWith("#![") && !parsed.remainder.startsWith("#[")) count += 1;
  }
  return count;
}

export function countMojoProductionLines(text) {
  let count = 0;
  const commentState = { blockComment: false };
  for (const rawLine of text.replace(/\r\n?/gu, "\n").split("\n")) {
    const line = stripComments(rawLine, commentState).trim();
    if (!line || line.startsWith("#") || line.startsWith("from ") || line.startsWith("import ") ||
        line.startsWith("@") || line.startsWith('abi("')) continue;
    count += 1;
  }
  return count;
}

function revisionArchive(revision) {
  if (revisionArchiveCache.has(revision)) return revisionArchiveCache.get(revision);
  const archive = execFileSync("git", ["archive", "--format=tar", revision], {
    cwd: repoRoot,
    encoding: null,
    maxBuffer: 512 * 1024 * 1024,
    stdio: ["ignore", "pipe", "ignore"],
  });
  const files = new Map();
  let pendingPath = null;
  let pendingLongName = null;
  for (let offset = 0; offset + 512 <= archive.length;) {
    const header = archive.subarray(offset, offset + 512);
    if (header.every((byte) => byte === 0)) break;
    const name = header.subarray(0, 100).toString("utf8").replace(/\0.*$/u, "");
    const prefix = header.subarray(345, 500).toString("utf8").replace(/\0.*$/u, "");
    const archiveName = prefix ? `${prefix}/${name}` : name;
    const size = Number.parseInt(header.subarray(124, 136).toString("ascii").replace(/\0.*$/u, "").trim() || "0", 8);
    const type = header[156];
    offset += 512;
    const contents = archive.subarray(offset, offset + size).toString("utf8");
    if (type === 120 || type === 103) {
      for (const record of contents.split("\n")) {
        const separator = record.indexOf("=");
        const space = record.indexOf(" ");
        if (separator > space && space >= 0) {
          const key = record.slice(record.indexOf(" ") + 1, separator);
          if (key === "path") pendingPath = record.slice(separator + 1);
        }
      }
    } else if (type === 76) {
      pendingLongName = contents.replace(/\0.*$/u, "").replace(/\n$/u, "");
    } else if (type === 0 || type === 48) {
      files.set(pendingPath ?? pendingLongName ?? archiveName, contents);
      pendingPath = null;
      pendingLongName = null;
    }
    offset += Math.ceil(size / 512) * 512;
  }
  revisionArchiveCache.set(revision, files);
  return files;
}

export function isProductionRustPath(relativePath) {
  if (!relativePath.endsWith(".rs")) return false;
  const components = relativePath.split("/");
  const normalizedComponents = components.map((component) => component.replace(/\.rs$/iu, ""));
  if (normalizedComponents.some((component) => EXCLUDED_COMPONENTS.test(component) || TEST_COMPONENT.test(component))) return false;
  const fileName = components.at(-1) ?? "";
  if (/^(?:tests?|test_[^/]*|[^/]*_tests)\.rs$/iu.test(fileName)) return false;
  return (relativePath.startsWith("src/") && components.length >= 2) ||
    (relativePath.startsWith("crates/") && components.includes("src"));
}

function selectedMojoSourcesFromBuild(build) {
  const commentState = { blockComment: false };
  const uncommented = build
    .replace(/\r\n?/gu, "\n")
    .split("\n")
    .map((line) => stripComments(line, commentState))
    .join("\n");
  return [...new Set(
    [...uncommented.matchAll(/^\s*sources\.push\(\s*["']\.\.\/\.\.\/(mojo\/prodex_core\/[A-Za-z0-9_]+\.mojo)["']\s*\);/gmu)]
      .map((match) => match[1]),
  )].sort();
}

function selectedMojoSources(revision) {
  return selectedMojoSourcesFromBuild(sourceText(revision, "crates/prodex-mojo-core/build.rs"));
}

function reachableMojoSources(revision) {
  const reachable = new Set();
  const queue = selectedMojoSources(revision);
  while (queue.length > 0) {
    const current = queue.pop();
    if (reachable.has(current) || !sourceExists(revision, current)) continue;
    reachable.add(current);
    for (const [, module] of sourceText(revision, current).matchAll(/^from\s+([A-Za-z0-9_]+)\s+import/gm)) {
      const imported = `mojo/prodex_core/${module}.mojo`;
      if (sourceExists(revision, imported)) queue.push(imported);
    }
  }
  return reachable;
}

function isProductionMojoPath(relativePath, reachable) {
  return relativePath.startsWith("mojo/prodex_core/") &&
    relativePath.endsWith(".mojo") && reachable.has(relativePath);
}

function rustEntries(revision) {
  return sourcePaths(revision)
    .filter(isProductionRustPath)
    .map((relativePath) => {
      const contents = sourceText(revision, relativePath);
      return {
        language: "rust",
        loc: countRustProductionLines(contents),
        path: relativePath,
        source_sha256: digest(contents),
      };
    });
}

function mojoEntries(revision, reachable) {
  return [...reachable].sort().map((relativePath) => {
    const contents = sourceText(revision, relativePath);
    return {
      language: "mojo",
      loc: countMojoProductionLines(contents),
      path: relativePath,
      source_sha256: digest(contents),
    };
  });
}

function countEntries(entries) {
  const counts = entries.reduce((result, entry) => {
    result[entry.language] += entry.loc;
    return result;
  }, { mojo: 0, rust: 0 });
  const total = counts.mojo + counts.rust;
  return {
    mojo_production_loc: counts.mojo,
    mojo_percent: total === 0 ? 0 : (counts.mojo * 100) / total,
    rust_production_loc: counts.rust,
    total_production_loc: total,
  };
}

export function assessMojoProductionNonRegression(baseline, final, approvedReductions = []) {
  const baselineMojoEntries = new Map(
    baseline.entries.filter((entry) => entry.language === "mojo").map((entry) => [entry.path, entry]),
  );
  const finalReachable = new Set(final.reachable_mojo_sources);
  const approvedPaths = new Set();
  let approvedReductionLoc = 0;
  for (const reduction of approvedReductions) {
    assert(reduction && typeof reduction.path === "string" && typeof reduction.reason === "string" &&
      reduction.reason.trim() !== "", "Mojo production non-regression reductions need path and reason");
    assert(!approvedPaths.has(reduction.path),
      `duplicate Mojo production non-regression reduction ${reduction.path}`);
    const baselineEntry = baselineMojoEntries.get(reduction.path);
    assert(baselineEntry && baseline.reachable_mojo_sources.includes(reduction.path),
      `${reduction.path} is not a reachable Mojo source in the non-regression baseline`);
    approvedPaths.add(reduction.path);
    if (!finalReachable.has(reduction.path)) approvedReductionLoc += baselineEntry.loc;
  }
  const missingReachableSources = baseline.reachable_mojo_sources.filter((source) =>
    !finalReachable.has(source) && !approvedPaths.has(source));
  const requiredMojoProductionLoc = Math.max(0, baseline.mojo_production_loc - approvedReductionLoc);
  const mojoProductionLocRegressed = final.mojo_production_loc < requiredMojoProductionLoc;
  return {
    approved_reduction_loc: approvedReductionLoc,
    approved_reductions: approvedReductions,
    baseline_mojo_production_loc: baseline.mojo_production_loc,
    final_mojo_production_loc: final.mojo_production_loc,
    missing_reachable_mojo_sources: missingReachableSources,
    mojo_production_loc_regressed: mojoProductionLocRegressed,
    required_mojo_production_loc: requiredMojoProductionLoc,
    met: missingReachableSources.length === 0 && !mojoProductionLocRegressed,
  };
}

export function productionInventoryAtRevision(revision) {
  const reachable = reachableMojoSources(revision);
  const entries = [
    ...rustEntries(revision),
    ...mojoEntries(revision, reachable),
  ].sort((left, right) => left.path < right.path ? -1 : left.path > right.path ? 1 : 0);
  return {
    entries,
    reachable_mojo_sources: [...reachable].sort(),
    selected_mojo_sources: selectedMojoSources(revision),
    ...countEntries(entries),
    source_inventory_sha256: digestJson(entries),
  };
}

function revisionTree(revision) {
  return git(["rev-parse", `${revision}^{tree}`]).trim();
}

function requiredMojoLoc(rustLoc, minimumPercent) {
  if (minimumPercent <= 0) return 0;
  if (minimumPercent >= 100) throw new Error("minimum_percent must be below 100");
  return Math.ceil((rustLoc * minimumPercent) / (100 - minimumPercent));
}

function readSnapshot(manifest) {
  const snapshotPath = path.join(repoRoot, manifest.baseline_snapshot ?? DEFAULT_SNAPSHOT_PATH);
  return JSON.parse(fs.readFileSync(snapshotPath, "utf8"));
}

function validateFrozenBaseline(manifest, baselineRevision, baseline, snapshot) {
  assert.equal(snapshot.schema_version, 1, "unsupported broad production-share snapshot schema");
  assert.equal(snapshot.baseline_sha, manifest.baseline_sha, "broad snapshot baseline SHA does not match manifest");
  assert.equal(snapshot.baseline_tree, revisionTree(baselineRevision), "broad baseline tree changed");
  assert.equal(snapshot.counting_rules_version, COUNTING_RULES_VERSION, "broad snapshot uses a different counting rule version");
  assert.equal(snapshot.source_inventory_sha256, baseline.source_inventory_sha256, "broad baseline source inventory changed");
  assert.deepEqual(snapshot.source_inventory, baseline.entries, "broad baseline source inventory contents changed");
  assert.equal(snapshot.broad_rust_production_loc, baseline.rust_production_loc, "broad baseline Rust LOC changed");
  assert.equal(snapshot.broad_mojo_production_loc, baseline.mojo_production_loc, "broad baseline Mojo LOC changed");
  assert.equal(snapshot.broad_total_production_loc, baseline.total_production_loc, "broad baseline total LOC changed");
  assert.equal(snapshot.broad_mojo_percent, baseline.mojo_percent, "broad baseline Mojo share changed");
  return snapshot;
}

export function calculateProductionShare(manifest, baselineRevision = manifest.baseline_sha, releaseRevision = "WORKTREE") {
  validateManifestMetadata(manifest);
  assert.equal(manifest.counting_rules_version, COUNTING_RULES_VERSION);
  const baseline = productionInventoryAtRevision(baselineRevision);
  const final = productionInventoryAtRevision(releaseRevision);
  const snapshot = readSnapshot(manifest);
  validateFrozenBaseline(manifest, baselineRevision, baseline, snapshot);
  const nonRegressionBaseline = productionInventoryAtRevision(manifest.mojo_non_regression.baseline_sha);
  assert.equal(
    nonRegressionBaseline.source_inventory_sha256,
    manifest.mojo_non_regression.baseline_source_inventory_sha256,
    "Mojo production non-regression baseline inventory changed",
  );
  assert.equal(
    nonRegressionBaseline.mojo_production_loc,
    manifest.mojo_non_regression.baseline_mojo_production_loc,
    "Mojo production non-regression baseline Mojo LOC changed",
  );
  const nonRegression = assessMojoProductionNonRegression(
    nonRegressionBaseline,
    final,
    manifest.mojo_non_regression.approved_reductions,
  );
  const requiredAtReleaseFloor = requiredMojoLoc(final.rust_production_loc, manifest.release_floor_percent);
  const requiredAtProjectTarget = requiredMojoLoc(final.rust_production_loc, manifest.project_target_percent);
  const currentProdexVersion = parseCargoVersion(fs.readFileSync(cargoTomlPath, "utf8"));
  const waiver = manifest.temporary_release_waiver;
  const releaseFloorMet = productionShareMeetsReleaseFloor({
    final,
    release_floor_percent: manifest.release_floor_percent,
  });
  const projectTargetMet = productionShareMeetsProjectTarget({
    final,
    project_target_percent: manifest.project_target_percent,
  });
  const releaseRequirementMet = releaseFloorMet && nonRegression.met;
  return {
    baseline: {
      broad_mojo_percent: baseline.mojo_percent,
      broad_mojo_production_loc: baseline.mojo_production_loc,
      broad_rust_production_loc: baseline.rust_production_loc,
      broad_total_production_loc: baseline.total_production_loc,
      source_inventory_sha256: baseline.source_inventory_sha256,
    },
    final: {
      broad_mojo_percent: final.mojo_percent,
      broad_mojo_production_loc: final.mojo_production_loc,
      broad_rust_production_loc: final.rust_production_loc,
      broad_total_production_loc: final.total_production_loc,
      source_inventory_sha256: final.source_inventory_sha256,
    },
    current_prodex_version: currentProdexVersion,
    release_floor_percent: manifest.release_floor_percent,
    release_floor_met: releaseFloorMet,
    release_floor_status: releaseFloorMet ? "PASS" : "FAIL",
    project_target_percent: manifest.project_target_percent,
    project_target_met: projectTargetMet,
    project_target_status: projectTargetMet ? "MET" : "NOT_YET_MET",
    mojo_non_regression_baseline_sha: manifest.mojo_non_regression.baseline_sha,
    mojo_non_regression_baseline_source_inventory_sha256: nonRegressionBaseline.source_inventory_sha256,
    mojo_non_regression_baseline_mojo_production_loc: nonRegressionBaseline.mojo_production_loc,
    mojo_non_regression: nonRegression,
    mojo_non_regression_met: nonRegression.met,
    mojo_non_regression_status: nonRegression.met ? "PASS" : "FAIL",
    historical_temporary_release_waiver: waiver
      ? {
          release_target: waiver.release_target,
          baseline_sha: waiver.baseline_sha,
          floor_percent: waiver.temporary_release_floor_percent,
          scope: waiver.scope,
          expiration: waiver.expiration,
          status: waiver.status.toUpperCase(),
        }
      : null,
    temporary_release_floor_percent: null,
    temporary_release_waiver_applicable: false,
    temporary_release_waiver_scope: null,
    temporary_release_waiver_reason: null,
    normal_requirement_met: projectTargetMet,
    release_requirement_met: releaseRequirementMet,
    release_status: releaseRequirementMet
      ? "PASS"
      : "FAIL",
    required_mojo_loc_at_release_floor: requiredAtReleaseFloor,
    additional_mojo_loc_needed_at_release_floor: Math.max(0, requiredAtReleaseFloor - final.mojo_production_loc),
    required_mojo_loc_at_project_target: requiredAtProjectTarget,
    additional_mojo_loc_needed_at_project_target: Math.max(0, requiredAtProjectTarget - final.mojo_production_loc),
    required_mojo_loc_at_final_rust_volume: requiredAtProjectTarget,
    additional_mojo_loc_needed_at_final_rust_volume: Math.max(0, requiredAtProjectTarget - final.mojo_production_loc),
    counting_rules_version: COUNTING_RULES_VERSION,
    baseline_tree: snapshot.baseline_tree,
    baseline_authoritative_operations: snapshot.authoritative_operations ?? [],
    baseline_semantic: snapshot.semantic ?? null,
    selected_mojo_sources: final.selected_mojo_sources,
    reachable_mojo_sources: final.reachable_mojo_sources,
  };
}

export function productionShareMeetsMinimum(result) {
  return productionShareMeetsProjectTarget(result);
}

function productionShareAtLeast(result, threshold) {
  const mojoLoc = result.final.broad_mojo_production_loc ?? result.final.mojo_production_loc;
  const totalLoc = result.final.broad_total_production_loc ?? result.final.total_production_loc;
  return mojoLoc * 100 >= threshold * totalLoc;
}

export function productionShareMeetsReleaseFloor(result) {
  return productionShareAtLeast(result, result.release_floor_percent ?? REQUIRED_RELEASE_FLOOR_PERCENT);
}

export function productionShareMeetsProjectTarget(result) {
  return productionShareAtLeast(
    result,
    result.project_target_percent ?? result.minimum_percent ?? REQUIRED_PROJECT_TARGET_PERCENT,
  );
}

export function productionShareMeetsReleaseRequirement(result) {
  return productionShareMeetsReleaseFloor(result) &&
    (result.mojo_non_regression_met ?? result.mojo_non_regression?.met ?? true);
}

function writeSnapshot(manifest, baselineRevision) {
  const baseline = productionInventoryAtRevision(baselineRevision);
  const semantic = JSON.parse(execFileSync(process.execPath, [
    path.join(repoRoot, "scripts/ci/mojo-ownership.mjs"),
    "--release-sha", baselineRevision,
    "--json",
  ], { cwd: repoRoot, encoding: "utf8" }));
  const snapshot = {
    schema_version: 1,
    release_target: manifest.release_target,
    baseline_sha: manifest.baseline_sha,
    baseline_tree: revisionTree(baselineRevision),
    counting_rules_version: COUNTING_RULES_VERSION,
    counting_rules: {
      rust: "Tracked crates/**/src/**/*.rs and src/**/*.rs in the shipped workspace graph, excluding test/bench/example/fixture/snapshot/generated/vendor/target/fuzz paths, bench_support and the bench-only prodex-bench-support crate, plus test-named source files; count non-blank, non-comment, non-import/directive lines and omit cfg(test)/cfg(not(feature=...)) bodies.",
      mojo: "Reachable sources selected by crates/prodex-mojo-core/build.rs when the production mojo-core feature is enabled, including local imports; count non-blank, non-comment, non-import/directive lines.",
      traversal: "Git source paths and local Mojo imports are sorted by bytewise path order; CRLF and LF are normalized by split(\\r?\\n); current directory and locale do not affect the result.",
      reachability: "Only Mojo roots selected by the production build graph or their local imports count; dead, test-only, generated, and unselected Mojo is zero.",
    },
    source_inventory_sha256: baseline.source_inventory_sha256,
    source_inventory: baseline.entries,
    selected_mojo_sources: baseline.selected_mojo_sources,
    reachable_mojo_sources: baseline.reachable_mojo_sources,
    broad_rust_production_loc: baseline.rust_production_loc,
    broad_mojo_production_loc: baseline.mojo_production_loc,
    broad_total_production_loc: baseline.total_production_loc,
    broad_mojo_percent: baseline.mojo_percent,
    baseline_version: "0.419.1",
    rich_abi_version: 6,
    eligible_rust_semantic_loc: semantic.final.rust_loc,
    eligible_mojo_semantic_loc: semantic.final.mojo_loc,
    semantic_mojo_percent: semantic.final.mojo_percent,
    semantic: {
      eligible_rust_semantic_loc: semantic.final.rust_loc,
      eligible_mojo_semantic_loc: semantic.final.mojo_loc,
      semantic_mojo_percent: semantic.final.mojo_percent,
    },
    authoritative_operations: semantic.authoritative_operations
      .filter((operation) => operation.final_state === "authoritative")
      .map((operation) => operation.name),
  };
  const outputPath = path.join(repoRoot, manifest.baseline_snapshot ?? DEFAULT_SNAPSHOT_PATH);
  fs.writeFileSync(outputPath, `${JSON.stringify(snapshot, null, 2)}\n`);
  return snapshot;
}

function selfTest() {
  const policy = {
    schema_version: 1,
    baseline_sha: REQUIRED_BASELINE_SHA,
    release_target: REQUIRED_HISTORICAL_RELEASE_TARGET,
    counting_rules_version: COUNTING_RULES_VERSION,
    release_floor_percent: REQUIRED_RELEASE_FLOOR_PERCENT,
    project_target_percent: REQUIRED_PROJECT_TARGET_PERCENT,
    mojo_non_regression: {
      baseline_sha: "43768659073cc1ab5c5686d3d58f2af68eebdef2",
      baseline_source_inventory_sha256: "0".repeat(64),
      baseline_mojo_production_loc: 0,
      approved_reductions: [],
    },
    production_build_feature: REQUIRED_PRODUCTION_BUILD_FEATURE,
  };
  assert.doesNotThrow(() => validateManifestMetadata({
    ...policy,
  }));
  assert.throws(
    () => validateManifestMetadata({
      ...policy,
      release_floor_percent: REQUIRED_RELEASE_FLOOR_PERCENT - 0.01,
    }),
    /release floor must remain 7%/u,
  );
  assert.throws(
    () => validateManifestMetadata({
      ...policy,
      project_target_percent: REQUIRED_PROJECT_TARGET_PERCENT - 0.01,
    }),
    /project target must remain 10%/u,
  );
  assert.deepEqual(
    selectedMojoSourcesFromBuild([
      '// sources.push("../../mojo/prodex_core/dead.mojo");',
      'sources.push("../../mojo/prodex_core/quota.mojo");',
      'println!("../../mojo/prodex_core/runtime_math.mojo");',
    ].join("\n")),
    ["mojo/prodex_core/quota.mojo"],
  );
  assert.equal(countRustProductionLines([
    "// comment",
    "use std::fmt;",
    "#[cfg(test)]",
    "mod tests {",
    "    fn ignored() {}",
    "}",
    "#[cfg(feature = \"bench-support\")]",
    "fn bench_only() {}",
    "#[cfg(not(feature = \"mojo\"))]",
    "fn fallback() {}",
    "fn production() {}",
  ].join("\r\n")), 1);
  assert.equal(countMojoProductionLines([
    "# comment",
    "from std.memory import Pointer",
    "@export(\"x\")",
    "def production():",
    "    return 1",
  ].join("\r\n")), 2);
  assert.equal(isProductionRustPath("crates/prodex-domain/src/lib.rs"), true);
  assert.equal(isProductionRustPath("crates/prodex-domain/src/tests.rs"), false);
  assert.equal(isProductionRustPath("crates/prodex-domain/tests/src/lib.rs"), false);
  assert.equal(isProductionRustPath("crates/prodex-app/src/bench_support/stream_cases.rs"), false);
  assert.equal(isProductionRustPath("crates/prodex-bench-support/src/lib.rs"), false);
  assert.equal(isProductionRustPath("crates/prodex-domain/src/fixture.rs"), false);
  assert.equal(isProductionRustPath("crates/prodex-domain/src/generated.rs"), false);
  assert.equal(isProductionRustPath("src/main.rs"), true);
  assert.equal(isProductionRustPath("migration/abi_probe.rs"), false);
  assert.equal(requiredMojoLoc(90, 10), 10);
  assert.equal(requiredMojoLoc(91, 10), 11);
  const share = (mojo) => ({
    final: { broad_mojo_production_loc: mojo, broad_total_production_loc: 10_000 },
    release_floor_percent: 7,
    project_target_percent: 10,
    mojo_non_regression_met: true,
  });
  assert.equal(productionShareMeetsReleaseFloor(share(699)), false);
  assert.equal(productionShareMeetsReleaseFloor(share(700)), true);
  assert.equal(productionShareMeetsProjectTarget(share(999)), false);
  assert.equal(productionShareMeetsProjectTarget(share(1_000)), true);
  assert.equal(productionShareMeetsReleaseRequirement(share(700)), true);
  assert.equal(productionShareMeetsReleaseRequirement({ ...share(700), mojo_non_regression_met: false }), false);
  const baseline = {
    entries: [{ language: "mojo", path: "mojo/prodex_core/example.mojo", loc: 100 }],
    mojo_production_loc: 100,
    reachable_mojo_sources: ["mojo/prodex_core/example.mojo"],
  };
  assert.equal(assessMojoProductionNonRegression(baseline, {
    mojo_production_loc: 100,
    reachable_mojo_sources: ["mojo/prodex_core/replacement.mojo"],
  }).met, false);
}

async function main() {
  const args = parseArgs(process.argv);
  if (args.selfTest) {
    selfTest();
    if (!args.check && !args.json && !args.writeSnapshot && !args.baseline && args.releaseSha === "WORKTREE") return;
  }
  const manifest = readManifest();
  const baseline = args.baseline ?? manifest.baseline_sha;
  if (baseline !== REQUIRED_BASELINE_SHA) {
    throw new Error(`broad production-share baseline must be ${REQUIRED_BASELINE_SHA}`);
  }
  if (args.writeSnapshot) {
    writeSnapshot(manifest, baseline);
  }
  const result = calculateProductionShare(manifest, baseline, args.releaseSha);
  if (args.check && !productionShareMeetsReleaseRequirement(result)) {
    if (!result.release_floor_met) {
      throw new Error(
        `Mojo production implementation share is ${result.final.broad_mojo_percent.toFixed(2)}%; ` +
        `at least ${result.release_floor_percent.toFixed(2)}% release floor is required`,
      );
    }
    throw new Error(
      `Mojo production ownership regressed; missing ${result.mojo_non_regression.missing_reachable_mojo_sources.length} ` +
      `baseline reachable source(s) or below ${result.mojo_non_regression.required_mojo_production_loc} Mojo LOC`,
    );
  }
  if (args.json) {
    process.stdout.write(`${JSON.stringify(result, null, 2)}\n`);
    return;
  }
  process.stdout.write([
    `Broad production source`,
    `Rust: ${result.final.broad_rust_production_loc.toLocaleString("en-US")} LOC`,
    `Mojo: ${result.final.broad_mojo_production_loc.toLocaleString("en-US")} LOC`,
    `Total: ${result.final.broad_total_production_loc.toLocaleString("en-US")} LOC`,
    `Mojo share: ${result.final.broad_mojo_percent.toFixed(2)}%`,
    `Release floor: >=${result.release_floor_percent.toFixed(2)}%`,
    `Release floor status: ${result.release_floor_status}`,
    `Project target: >=${result.project_target_percent.toFixed(2)}%`,
    `Project target status: ${result.project_target_status.replaceAll("_", " ")}`,
    `Mojo ownership non-regression: ${result.mojo_non_regression_status}`,
    `Historical 0.421.0 waiver: ${result.historical_temporary_release_waiver?.status ?? "NONE"}`,
    `Status: ${result.release_status}`,
    `Additional Mojo LOC needed at project target at current Rust volume: ${result.additional_mojo_loc_needed_at_project_target.toLocaleString("en-US")}`,
  ].join("\n") + "\n");
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    process.stderr.write(`mojo-production-share: ${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  });
}
