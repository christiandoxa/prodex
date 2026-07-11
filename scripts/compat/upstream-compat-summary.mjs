#!/usr/bin/env node
import fs from "node:fs";

function usage() {
  return `Usage:
  node scripts/compat/upstream-compat-summary.mjs <report> <summary> <status_code> <baseline_path> <fail_on_drift>

Writes the GitHub Actions markdown summary for upstream compatibility watchdog.

Arguments:
  report          Path to watchdog JSON report
  summary         Path to markdown summary output
  status_code     watch-upstream.mjs exit status
  baseline_path   Baseline path shown in summary
  fail_on_drift   true or false
`;
}

function parseArgs(argv) {
  const args = argv.slice(2);
  if (args.includes("--help") || args.includes("-h")) {
    return { help: true };
  }
  if (args.length !== 5) {
    throw new Error("expected 5 arguments: <report> <summary> <status_code> <baseline_path> <fail_on_drift>");
  }
  const [reportPath, summaryPath, statusCodeRaw, baselinePath, failOnDrift] = args;
  return { reportPath, summaryPath, statusCodeRaw, baselinePath, failOnDrift };
}

function writeSummary({ reportPath, summaryPath, statusCodeRaw, baselinePath, failOnDrift }) {
  const statusCode = Number.parseInt(statusCodeRaw, 10) || 0;
  const lines = ["## Upstream compatibility", ""];
  lines.push(`- Event: \`${process.env.GITHUB_EVENT_NAME}\``);
  lines.push(`- Ref: \`${process.env.GITHUB_REF}\``);
  lines.push(`- Baseline: \`${baselinePath}\``);
  lines.push("- Offline guard: `passed`");
  lines.push(`- Fail on drift: \`${failOnDrift}\``);

  if (fs.existsSync(reportPath)) {
    const report = JSON.parse(fs.readFileSync(reportPath, "utf8"));
    const codex = report.current?.codex?.latestRelease ?? {};
    const claude = report.current?.claude?.latestRelease ?? {};
    const diffs = Array.isArray(report.diffs) ? report.diffs : [];
    const diffSummary = report.diff_summary || {};
    const statusLabel = String(diffSummary.status || (diffs.length === 0 ? "in_sync" : "drift_detected")).replace(
      /_/g,
      " ",
    );

    lines.push(
      `- Codex latest release: [${codex.tag_name || "unknown"}](${codex.html_url || "https://github.com/openai/codex/releases/latest"})`,
    );
    lines.push(
      `- Claude latest release: [${claude.tag_name || "unknown"}](${claude.html_url || "https://github.com/anthropics/claude-code/releases/latest"})`,
    );
    lines.push(`- Report generated: \`${report.generated_at || "unknown"}\``);
    lines.push("");

    if (diffs.length === 0) {
      lines.push("Status: in sync");
    } else {
      lines.push(`Status: ${statusLabel} (${diffs.length} change${diffs.length === 1 ? "" : "s"})`);
      if (Number.isInteger(diffSummary.blocking)) {
        lines.push(`- Blocking compatibility diffs: \`${diffSummary.blocking}\``);
      }
      if (Number.isInteger(diffSummary.non_blocking)) {
        lines.push(`- Non-blocking drift diffs: \`${diffSummary.non_blocking}\``);
      }
      if (diffSummary.labels && typeof diffSummary.labels === "object") {
        lines.push("");
        lines.push("### Drift labels");
        for (const [label, count] of Object.entries(diffSummary.labels)) {
          lines.push(`- \`${label}\`: ${count}`);
        }
      }
      lines.push("");
      lines.push("### Drift paths");
      for (const diff of diffs.slice(0, 20)) {
        const label = diff.label ? `[${diff.label}] ` : "";
        const blocking = diff.blocking ? " blocking" : "";
        lines.push(`- ${label}\`${diff.path}\`${blocking}`);
      }
      if (diffs.length > 20) {
        lines.push(`- ... and ${diffs.length - 20} more`);
      }
    }
  } else {
    lines.push("");
    lines.push(`Status: watchdog failed before report generation (exit ${statusCode})`);
  }

  if (statusCode !== 0 && failOnDrift === "false") {
    lines.push("");
    lines.push("Manual override: drift does not fail this run. Inspect the uploaded artifact for details.");
  }

  fs.writeFileSync(summaryPath, `${lines.join("\n")}\n`);
}

function main() {
  const args = parseArgs(process.argv);
  if (args.help) {
    process.stdout.write(usage());
    return;
  }
  writeSummary(args);
}

try {
  main();
} catch (error) {
  process.stderr.write(`${error.message}\n`);
  process.exitCode = 1;
}
