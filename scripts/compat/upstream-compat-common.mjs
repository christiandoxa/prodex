import path from "node:path";
import { repoRoot } from "../npm/common.mjs";

export const DEFAULT_BASELINE_PATH = path.join(repoRoot, "scripts/compat/upstream-baseline.json");

export const SEMANTIC_FILE_CONTAINS_FIELD = "file_contains_all";

export const SEMANTIC_DIFF_FIELDS = Object.freeze([
  {
    field: SEMANTIC_FILE_CONTAINS_FIELD,
    found: "file_contains_found",
    missing: "file_contains_missing",
    source: "file",
  },
]);

export const DIFF_LABELS = Object.freeze({
  DOCUMENTATION_METADATA_DRIFT: "documentation_metadata_drift",
  RELEASE_DRIFT: "release_drift",
  REQUIRED_CONTENT_MISSING: "required_content_missing",
  SEMANTIC_REQUIRED_CONTENT_MISSING: "semantic_required_content_missing",
  WATCHDOG_SCOPE_MISMATCH: "watchdog_scope_mismatch",
});

export const DIFF_CATEGORIES = Object.freeze({
  DOCUMENTATION_METADATA: "documentation_metadata",
  RELEASE_METADATA: "release_metadata",
  SEMANTIC_COMPATIBILITY: "semantic_compatibility",
  WATCHDOG_SCOPE: "watchdog_scope",
});

export const DIFF_SEVERITIES = Object.freeze({
  ERROR: "error",
  WARNING: "warning",
});

export const BLOCKING_SEMANTIC_DIFF = Object.freeze({
  category: DIFF_CATEGORIES.SEMANTIC_COMPATIBILITY,
  severity: DIFF_SEVERITIES.ERROR,
  blocking: true,
});

export const RELEASE_METADATA_DIFF = Object.freeze({
  label: DIFF_LABELS.RELEASE_DRIFT,
  category: DIFF_CATEGORIES.RELEASE_METADATA,
  severity: DIFF_SEVERITIES.WARNING,
  blocking: false,
});

export const DOCUMENTATION_METADATA_DIFF = Object.freeze({
  label: DIFF_LABELS.DOCUMENTATION_METADATA_DRIFT,
  category: DIFF_CATEGORIES.DOCUMENTATION_METADATA,
  severity: DIFF_SEVERITIES.WARNING,
  blocking: false,
});

export const WATCHDOG_SCOPE_DIFF = Object.freeze({
  label: DIFF_LABELS.WATCHDOG_SCOPE_MISMATCH,
  category: DIFF_CATEGORIES.WATCHDOG_SCOPE,
  severity: DIFF_SEVERITIES.ERROR,
  blocking: true,
});

export function requiredValue(value, name) {
  if (!value) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

export function parseCompatScriptArgs(argv, options) {
  const args = { ...(options.defaults ?? {}) };
  const valueOptions = options.valueOptions ?? {};
  const booleanOptions = options.booleanOptions ?? {};

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value in valueOptions) {
      index += 1;
      args[valueOptions[value]] = requiredValue(argv[index], value);
      continue;
    }
    if (value in booleanOptions) {
      args[booleanOptions[value]] = true;
      continue;
    }
    throw new Error(`unknown argument: ${value}`);
  }

  return args;
}

export function stringArray(value) {
  if (!Array.isArray(value)) {
    return [];
  }
  return value.filter((item) => typeof item === "string");
}

export function containsAllResult(tokens, contents) {
  const required = stringArray(tokens);
  return {
    required,
    found: required.filter((token) => contents.includes(token)),
    missing: required.filter((token) => !contents.includes(token)),
  };
}

export function makeDiff({
  path: pathLabel,
  baseline,
  current,
  label,
  category,
  severity,
  blocking,
  metadata = {},
}) {
  return {
    path: pathLabel,
    label,
    category,
    severity,
    blocking,
    metadata,
    baseline,
    current,
  };
}

export function diffField(pathLabel, baselineValue, currentValue, classification) {
  if (JSON.stringify(baselineValue) === JSON.stringify(currentValue)) {
    return null;
  }
  return makeDiff({
    path: pathLabel,
    ...classification,
    baseline: baselineValue,
    current: currentValue,
  });
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

function countBy(values, key) {
  return values.reduce((counts, value) => {
    const countKey = value[key] ?? "unknown";
    counts[countKey] = (counts[countKey] ?? 0) + 1;
    return counts;
  }, {});
}

export function buildDiffSummary(diffs) {
  const blocking = diffs.filter((diff) => diff.blocking).length;
  const nonBlocking = diffs.length - blocking;
  const releaseDriftOnly =
    diffs.length > 0 && diffs.every((diff) => diff.category === DIFF_CATEGORIES.RELEASE_METADATA);
  const status =
    blocking > 0
      ? "compatibility_break"
      : releaseDriftOnly
        ? "release_drift"
        : diffs.length > 0
          ? "non_blocking_drift"
          : "in_sync";

  return {
    status,
    exit_code: blocking > 0 ? 1 : 0,
    total: diffs.length,
    blocking,
    non_blocking: nonBlocking,
    labels: countBy(diffs, "label"),
    categories: countBy(diffs, "category"),
    severities: countBy(diffs, "severity"),
  };
}

function formatStatus(status) {
  return status.replace(/_/g, " ");
}

export function renderWatchReport(snapshot, diffs, diffSummary) {
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

  lines.push(`Status: ${formatStatus(diffSummary.status)}`);
  lines.push(`Blocking compatibility diffs: ${diffSummary.blocking}`);
  lines.push(`Non-blocking drift diffs: ${diffSummary.non_blocking}`);
  lines.push("Diff labels:");
  for (const [label, count] of Object.entries(diffSummary.labels)) {
    lines.push(`- ${label}: ${count}`);
  }
  lines.push("");
  for (const diff of diffs) {
    lines.push(`- [${diff.label}] ${diff.path}`);
    lines.push(`  category: ${diff.category}`);
    lines.push(`  severity: ${diff.severity}`);
    lines.push(`  blocking: ${diff.blocking ? "yes" : "no"}`);
    lines.push(`  baseline: ${formatList(diff.baseline)}`);
    lines.push(`  current: ${formatList(diff.current)}`);
  }
  return lines.join("\n") + "\n";
}
