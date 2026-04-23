#!/usr/bin/env node
import { readFileSync } from "node:fs";

const jsonPrefix = "runtime_proxy_hot_path_check_json ";
const legacyMarker = "runtime_proxy_hot_path_check ";

function parseArgs(argv) {
  const args = {
    basis: "max",
    format: "table",
    marginPercent: 25,
    percentile: 90,
    paths: [],
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--basis") {
      index += 1;
      if (argv[index] !== "max" && argv[index] !== "percentile") {
        throw new Error("--basis must be max or percentile");
      }
      args.basis = argv[index];
      continue;
    }
    if (value === "--json") {
      args.format = "json";
      continue;
    }
    if (value === "--margin-percent") {
      index += 1;
      args.marginPercent = parsePositiveInteger("--margin-percent", argv[index]);
      continue;
    }
    if (value === "--percentile") {
      index += 1;
      args.percentile = parsePercentile(argv[index]);
      continue;
    }
    if (value === "--help" || value === "-h") {
      args.help = true;
      continue;
    }
    if (value.startsWith("-")) {
      throw new Error(`unknown argument: ${value}`);
    }
    args.paths.push(value);
  }

  return args;
}

function parsePositiveInteger(name, rawValue) {
  if (!rawValue || !/^\d+$/.test(rawValue)) {
    throw new Error(`${name} requires a positive integer`);
  }
  const value = Number(rawValue);
  if (!Number.isSafeInteger(value) || value < 1) {
    throw new Error(`${name} requires a positive integer`);
  }
  return value;
}

function parsePercentile(rawValue) {
  const value = parsePositiveInteger("--percentile", rawValue);
  if (value > 100) {
    throw new Error("--percentile must be between 1 and 100");
  }
  return value;
}

function readInputs(paths) {
  if (paths.length === 0) {
    return [{ source: "stdin", text: readFileSync(0, "utf8") }];
  }
  return paths.map((path) => ({ source: path, text: readFileSync(path, "utf8") }));
}

function parseOptionalInteger(value) {
  if (value === undefined || value === null || value === "") {
    return null;
  }
  const parsed = Number(value);
  return Number.isSafeInteger(parsed) ? parsed : null;
}

function deriveScalePercent(measuredNs, baseThresholdNs) {
  if (
    measuredNs === null ||
    baseThresholdNs === null ||
    measuredNs < 0 ||
    baseThresholdNs < 1
  ) {
    return null;
  }
  return Math.max(1, Math.ceil((measuredNs * 100) / baseThresholdNs));
}

function normalizeRecord(payload, source, lineNumber) {
  const caseName = payload.case ?? payload.name;
  if (!caseName) {
    return null;
  }

  const medianNs = parseOptionalInteger(payload.median_ns);
  const p90Ns = parseOptionalInteger(payload.p90_ns);
  const thresholdNs = parseOptionalInteger(payload.threshold_ns);
  let thresholdScalePercent =
    parseOptionalInteger(payload.threshold_scale_percent) ??
    parseOptionalInteger(payload.applied_scale_percent);
  let baseThresholdNs = parseOptionalInteger(payload.base_threshold_ns);
  if (baseThresholdNs === null && thresholdNs !== null && thresholdScalePercent !== null) {
    baseThresholdNs = Math.round((thresholdNs * 100) / thresholdScalePercent);
  }

  if (medianNs === null || p90Ns === null || thresholdNs === null) {
    return null;
  }

  if (thresholdScalePercent === null && baseThresholdNs !== null) {
    thresholdScalePercent = deriveScalePercent(thresholdNs, baseThresholdNs);
  }

  const requiredScalePercent =
    parseOptionalInteger(payload.required_scale_percent) ??
    deriveScalePercent(medianNs, baseThresholdNs);
  const scaleHeadroomPercent =
    parseOptionalInteger(payload.scale_headroom_percent) ??
    (thresholdScalePercent !== null && requiredScalePercent !== null
      ? thresholdScalePercent - requiredScalePercent
      : null);
  const thresholdHeadroomNs =
    parseOptionalInteger(payload.threshold_headroom_ns) ??
    (thresholdNs !== null && medianNs !== null ? thresholdNs - medianNs : null);

  return {
    baseThresholdNs,
    case: String(caseName),
    lineNumber,
    medianNs,
    p90Ns,
    requiredScalePercent,
    scaleHeadroomPercent,
    source,
    status: payload.status ? String(payload.status) : "unknown",
    thresholdHeadroomNs,
    thresholdNs,
    thresholdScalePercent,
  };
}

function parseKeyValues(text) {
  const values = new Map();
  for (const token of text.trim().split(/\s+/)) {
    const separator = token.indexOf("=");
    if (separator <= 0) {
      continue;
    }
    values.set(token.slice(0, separator), token.slice(separator + 1));
  }
  return values;
}

function parseLog(source, text) {
  const jsonRecords = [];
  const legacyRecords = [];
  const pendingLegacyRecords = [];

  text.split(/\r?\n/).forEach((line, lineIndex) => {
    const lineNumber = lineIndex + 1;
    const jsonIndex = line.indexOf(jsonPrefix);
    if (jsonIndex !== -1) {
      const rawJson = line.slice(jsonIndex + jsonPrefix.length).trim();
      try {
        const payload = JSON.parse(rawJson);
        if (payload.event === "case") {
          const record = normalizeRecord(payload, source, lineNumber);
          if (record) {
            jsonRecords.push(record);
          }
        }
      } catch {
        // Ignore truncated or unrelated JSON log lines.
      }
      return;
    }

    const legacyIndex = line.indexOf(legacyMarker);
    if (legacyIndex === -1) {
      return;
    }
    const fields = parseKeyValues(line.slice(legacyIndex + legacyMarker.length));
    if (fields.has("case")) {
      const record = normalizeRecord(
        {
          base_threshold_ns: fields.get("base_threshold_ns"),
          case: fields.get("case"),
          median_ns: fields.get("median_ns"),
          p90_ns: fields.get("p90_ns"),
          required_scale_percent: fields.get("required_scale_percent"),
          scale_headroom_percent: fields.get("scale_headroom_percent"),
          status: fields.get("status"),
          threshold_headroom_ns: fields.get("threshold_headroom_ns"),
          threshold_ns: fields.get("threshold_ns"),
          threshold_scale_percent: fields.get("threshold_scale_percent"),
        },
        source,
        lineNumber,
      );
      if (record) {
        legacyRecords.push(record);
        pendingLegacyRecords.push(record);
      }
      return;
    }

    const scalePercent = parseOptionalInteger(
      fields.get("threshold_scale_percent") ?? fields.get("default_threshold_scale_percent"),
    );
    if (
      fields.has("summary") ||
      scalePercent !== null ||
      fields.has("default_threshold_scale_percent")
    ) {
      for (const record of pendingLegacyRecords) {
        if (record.thresholdScalePercent === null && scalePercent !== null) {
          record.thresholdScalePercent = scalePercent;
        }
        if (record.baseThresholdNs === null && record.thresholdScalePercent) {
          record.baseThresholdNs = Math.round(
            (record.thresholdNs * 100) / record.thresholdScalePercent,
          );
        }
        if (record.requiredScalePercent === null) {
          record.requiredScalePercent = deriveScalePercent(
            record.medianNs,
            record.baseThresholdNs,
          );
        }
        if (
          record.scaleHeadroomPercent === null &&
          record.thresholdScalePercent !== null &&
          record.requiredScalePercent !== null
        ) {
          record.scaleHeadroomPercent =
            record.thresholdScalePercent - record.requiredScalePercent;
        }
        if (record.thresholdHeadroomNs === null) {
          record.thresholdHeadroomNs = record.thresholdNs - record.medianNs;
        }
      }
      pendingLegacyRecords.length = 0;
    }
  });

  for (const record of pendingLegacyRecords) {
    if (record.thresholdScalePercent === null) {
      record.thresholdScalePercent = 100;
    }
    if (record.baseThresholdNs === null) {
      record.baseThresholdNs = record.thresholdNs;
    }
    if (record.requiredScalePercent === null) {
      record.requiredScalePercent = deriveScalePercent(record.medianNs, record.baseThresholdNs);
    }
    if (
      record.scaleHeadroomPercent === null &&
      record.thresholdScalePercent !== null &&
      record.requiredScalePercent !== null
    ) {
      record.scaleHeadroomPercent = record.thresholdScalePercent - record.requiredScalePercent;
    }
    if (record.thresholdHeadroomNs === null) {
      record.thresholdHeadroomNs = record.thresholdNs - record.medianNs;
    }
  }

  return jsonRecords.length > 0 ? jsonRecords : legacyRecords;
}

function percentile(values, percentileValue) {
  const sorted = [...values].sort((left, right) => left - right);
  const index = Math.max(0, Math.ceil((percentileValue / 100) * sorted.length) - 1);
  return sorted[index];
}

function roundUpReadable(value) {
  const step = value < 10_000 ? 100 : value < 1_000_000 ? 1_000 : 10_000;
  return Math.ceil(value / step) * step;
}

function classifyAction(currentScalePercent, suggestedScalePercent) {
  if (suggestedScalePercent === null) {
    return "unknown";
  }
  if (currentScalePercent === null) {
    return "set";
  }
  if (suggestedScalePercent > currentScalePercent) {
    return "raise";
  }
  if (suggestedScalePercent < currentScalePercent) {
    return "tighten";
  }
  return "keep";
}

function summarize(records, options) {
  const grouped = new Map();
  for (const record of records) {
    if (!grouped.has(record.case)) {
      grouped.set(record.case, []);
    }
    grouped.get(record.case).push(record);
  }

  return [...grouped.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([caseName, caseRecords]) => {
      const medians = caseRecords.map((record) => record.medianNs);
      const runP90s = caseRecords.map((record) => record.p90Ns);
      const p50MedianNs = percentile(medians, 50);
      const selectedPercentileMedianNs = percentile(medians, options.percentile);
      const maxMedianNs = Math.max(...medians);
      const maxRunP90Ns = Math.max(...runP90s);
      const baseThresholdNs =
        [...caseRecords].reverse().find((record) => record.baseThresholdNs !== null)
          ?.baseThresholdNs ?? null;
      const currentScalePercent =
        [...caseRecords].reverse().find((record) => record.thresholdScalePercent !== null)
          ?.thresholdScalePercent ?? null;
      const currentThresholdNs =
        [...caseRecords].reverse().find((record) => record.thresholdNs !== null)?.thresholdNs ??
        null;
      const maxRequiredScalePercent = Math.max(
        ...caseRecords
          .map((record) => record.requiredScalePercent)
          .filter((value) => value !== null),
      );
      const currentScaleHeadroomPercent =
        currentScalePercent !== null && Number.isFinite(maxRequiredScalePercent)
          ? currentScalePercent - maxRequiredScalePercent
          : null;
      const basisNs =
        options.basis === "percentile" ? selectedPercentileMedianNs : maxMedianNs;
      const suggestedThresholdNs = roundUpReadable(
        (basisNs * (100 + options.marginPercent)) / 100,
      );
      const suggestedScalePercent = deriveScalePercent(
        suggestedThresholdNs,
        baseThresholdNs,
      );
      const scaleDeltaPercent =
        currentScalePercent !== null && suggestedScalePercent !== null
          ? suggestedScalePercent - currentScalePercent
          : null;
      const action = classifyAction(currentScalePercent, suggestedScalePercent);

      return {
        action,
        base_threshold_ns: baseThresholdNs,
        case: caseName,
        current_scale_headroom_percent: currentScaleHeadroomPercent,
        current_scale_percent: currentScalePercent,
        current_threshold_ns: currentThresholdNs,
        max_median_ns: maxMedianNs,
        max_required_scale_percent: Number.isFinite(maxRequiredScalePercent)
          ? maxRequiredScalePercent
          : null,
        max_run_p90_ns: maxRunP90Ns,
        p50_median_ns: p50MedianNs,
        [`p${options.percentile}_median_ns`]: selectedPercentileMedianNs,
        records: caseRecords.length,
        scale_delta_percent: scaleDeltaPercent,
        suggested_scale_percent: suggestedScalePercent,
        suggested_threshold_ns: suggestedThresholdNs,
      };
    });
}

function countActions(summaries) {
  const counts = {
    keep: 0,
    raise: 0,
    set: 0,
    tighten: 0,
    unknown: 0,
  };
  for (const summary of summaries) {
    counts[summary.action] = (counts[summary.action] ?? 0) + 1;
  }
  return counts;
}

function buildSuggestedThresholdConfig(summaries) {
  return {
    version: 1,
    default_scale_percent: 100,
    case_scale_percent: Object.fromEntries(
      summaries
        .filter((summary) => summary.suggested_scale_percent !== null)
        .map((summary) => [summary.case, summary.suggested_scale_percent]),
    ),
  };
}

function pad(value, width) {
  return String(value ?? "-").padEnd(width, " ");
}

function renderTable(summaries, options, recordCount) {
  const percentileHeader = `p${options.percentile}`;
  const headers = [
    "case",
    "runs",
    "base_ns",
    "cur_pct",
    "need_pct",
    "slack_pct",
    "p50",
    percentileHeader,
    "max",
    "suggest_pct",
    "delta_pct",
    "action",
  ];
  const rows = summaries.map((summary) => [
    summary.case,
    summary.records,
    summary.base_threshold_ns,
    summary.current_scale_percent,
    summary.max_required_scale_percent,
    summary.current_scale_headroom_percent,
    summary.p50_median_ns,
    summary[`p${options.percentile}_median_ns`],
    summary.max_median_ns,
    summary.suggested_scale_percent,
    summary.scale_delta_percent,
    summary.action,
  ]);
  const widths = headers.map((header, index) =>
    Math.max(header.length, ...rows.map((row) => String(row[index] ?? "-").length)),
  );
  const suggestedGlobalScalePercent = Math.max(
    ...summaries
      .map((summary) => summary.suggested_scale_percent)
      .filter((value) => value !== null),
  );
  const actionCounts = countActions(summaries);
  const suggestedThresholdConfig = buildSuggestedThresholdConfig(summaries);

  process.stdout.write(
    [
      `runtime proxy benchmark calibration records=${recordCount} basis=${options.basis} margin_percent=${options.marginPercent} percentile=${options.percentile}`,
      Number.isFinite(suggestedGlobalScalePercent)
        ? `suggested_global_scale_percent=${suggestedGlobalScalePercent}`
        : "suggested_global_scale_percent=-",
      `actions raise=${actionCounts.raise} tighten=${actionCounts.tighten} keep=${actionCounts.keep} set=${actionCounts.set} unknown=${actionCounts.unknown}`,
      headers.map((header, index) => pad(header, widths[index])).join("  "),
      widths.map((width) => "-".repeat(width)).join("  "),
      ...rows.map((row) => row.map((value, index) => pad(value, widths[index])).join("  ")),
      "",
      "suggested_threshold_config:",
      JSON.stringify(suggestedThresholdConfig, null, 2),
    ].join("\n") + "\n",
  );
}

function renderJson(summaries, options, recordCount) {
  const suggestedGlobalScalePercent = Math.max(
    ...summaries
      .map((summary) => summary.suggested_scale_percent)
      .filter((value) => value !== null),
  );
  process.stdout.write(
    JSON.stringify(
      {
        action_counts: countActions(summaries),
        basis: options.basis,
        cases: summaries,
        margin_percent: options.marginPercent,
        percentile: options.percentile,
        records: recordCount,
        suggested_global_scale_percent: Number.isFinite(suggestedGlobalScalePercent)
          ? suggestedGlobalScalePercent
          : null,
        suggested_threshold_config: buildSuggestedThresholdConfig(summaries),
      },
      null,
      2,
    ) + "\n",
  );
}

function usage() {
  return [
    "Usage: node scripts/ci/benchmark-calibration.mjs [options] [log-file ...]",
    "",
    "Parses runtime_proxy_hot_path_check logs from CI and suggests hot-path thresholds.",
    "With no log files, reads stdin.",
    "",
    "Options:",
    "  --basis max|percentile     Suggest from max observed median or percentile median (default: max)",
    "  --margin-percent <n>       Safety margin over selected basis (default: 25)",
    "  --percentile <n>           Percentile for reporting or --basis percentile (default: 90)",
    "  --json                     Emit JSON summary",
    "",
    "Example:",
    "  gh run view <run-id> --log > /tmp/prodex-bench.log",
    "  node scripts/ci/benchmark-calibration.mjs /tmp/prodex-bench.log",
  ].join("\n");
}

const args = parseArgs(process.argv);
if (args.help) {
  process.stdout.write(`${usage()}\n`);
} else {
  const inputs = readInputs(args.paths);
  const records = inputs.flatMap((input) => parseLog(input.source, input.text));
  if (records.length === 0) {
    process.stderr.write("no runtime_proxy_hot_path_check records found\n");
    process.exitCode = 1;
  } else {
    const summaries = summarize(records, args);
    if (args.format === "json") {
      renderJson(summaries, args, records.length);
    } else {
      renderTable(summaries, args, records.length);
    }
  }
}
