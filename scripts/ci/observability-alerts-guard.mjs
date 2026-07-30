#!/usr/bin/env node
import assert from "node:assert/strict";
import fs from "node:fs";

const ALERTS_PATH = "deploy/observability/prodex-alerts.yaml";
const RUNBOOK_PATH = "docs/enterprise-governance/20-operations-slos-and-alerts.md";
const RUNBOOK_PREFIX =
  "https://github.com/christiandoxa/prodex/blob/main/docs/enterprise-governance/20-operations-slos-and-alerts.md#";

const REQUIRED_WINDOWS = Object.freeze({
  ProdexGatewayAvailabilityFastBurn: ["[5m]", "[1h]", "> 0.0144", "for: 2m"],
  ProdexGatewayAvailabilitySlowBurn: ["[30m]", "[6h]", "> 0.006", "for: 15m"],
  ProdexApiP99LatencyFast: ["[5m]", "[1h]", "> 30000", "for: 2m"],
  ProdexApiP99LatencySlow: ["[30m]", "[6h]", "> 30000", "for: 15m"],
});

function markdownAnchor(heading) {
  return heading
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s-]/gu, "")
    .replace(/\s+/gu, "-");
}

function runbookAnchors(markdown) {
  return new Set(
    markdown
      .split(/\r?\n/u)
      .map((line) => /^#{1,6}\s+(.+)$/u.exec(line)?.[1])
      .filter(Boolean)
      .map(markdownAnchor),
  );
}

function alertBlocks(yaml) {
  const lines = yaml.split(/\r?\n/u);
  const starts = [];
  for (let index = 0; index < lines.length; index += 1) {
    const match = /^\s{8}- alert:\s*([A-Za-z][A-Za-z0-9]*)\s*$/u.exec(lines[index]);
    if (match) starts.push({ index, name: match[1] });
  }
  return starts.map((start, index) => ({
    name: start.name,
    text: lines.slice(start.index, starts[index + 1]?.index ?? lines.length).join("\n"),
  }));
}

function nestedValue(block, section, key) {
  const match = new RegExp(
    `^\\s{10}${section}:\\s*$[\\s\\S]*?^\\s{12}${key}:\\s*(.+?)\\s*$`,
    "mu",
  ).exec(block);
  return match?.[1]?.trim();
}

export function validateObservabilityAlerts(yaml, runbook) {
  const errors = [];
  const blocks = alertBlocks(yaml);
  const anchors = runbookAnchors(runbook);
  const names = new Set();

  if (blocks.length === 0) errors.push(`${ALERTS_PATH}: no alert rules found`);
  for (const block of blocks) {
    if (names.has(block.name)) errors.push(`${ALERTS_PATH}: duplicate alert ${block.name}`);
    names.add(block.name);

    if (!/^\s{10}expr:/mu.test(block.text)) errors.push(`${block.name}: missing expr`);
    if (!/^\s{10}for:\s*\S+/mu.test(block.text)) errors.push(`${block.name}: missing for duration`);

    for (const key of ["severity", "owner", "escalation"]) {
      if (!nestedValue(block.text, "labels", key)) errors.push(`${block.name}: missing labels.${key}`);
    }
    for (const key of [
      "summary",
      "description",
      "runbook_url",
      "auto_resolve",
      "closure_evidence",
    ]) {
      if (!nestedValue(block.text, "annotations", key)) {
        errors.push(`${block.name}: missing annotations.${key}`);
      }
    }

    const severity = nestedValue(block.text, "labels", "severity");
    const escalation = nestedValue(block.text, "labels", "escalation");
    if (!new Set(["critical", "warning", "info"]).has(severity)) {
      errors.push(`${block.name}: invalid severity ${severity ?? "<missing>"}`);
    }
    if (!new Set(["page", "ticket", "record"]).has(escalation)) {
      errors.push(`${block.name}: invalid escalation ${escalation ?? "<missing>"}`);
    }
    if (severity === "critical" && escalation !== "page") {
      errors.push(`${block.name}: critical alert must escalate by page`);
    }

    const runbookUrl = nestedValue(block.text, "annotations", "runbook_url");
    if (runbookUrl && !runbookUrl.startsWith(RUNBOOK_PREFIX)) {
      errors.push(`${block.name}: runbook_url must use the canonical operations runbook`);
    } else if (runbookUrl) {
      const anchor = runbookUrl.slice(RUNBOOK_PREFIX.length);
      if (!anchors.has(anchor)) errors.push(`${block.name}: missing runbook anchor #${anchor}`);
    }
  }

  for (const [name, markers] of Object.entries(REQUIRED_WINDOWS)) {
    const block = blocks.find((candidate) => candidate.name === name);
    if (!block) {
      errors.push(`${ALERTS_PATH}: missing ${name}`);
      continue;
    }
    for (const marker of markers) {
      if (!block.text.includes(marker)) errors.push(`${name}: missing ${marker}`);
    }
  }

  return errors;
}

function selfTest(yaml, runbook) {
  assert.deepEqual(validateObservabilityAlerts(yaml, runbook), []);
  assert.ok(
    validateObservabilityAlerts(yaml.replace(/\n\s+owner:\s*prodex-platform/u, ""), runbook).some(
      (error) => error.includes("missing labels.owner"),
    ),
  );
  assert.ok(
    validateObservabilityAlerts(yaml.replace("#local-admission-or-queue-pressure", "#missing-runbook"), runbook).some(
      (error) => error.includes("missing runbook anchor"),
    ),
  );
  assert.ok(
    validateObservabilityAlerts(yaml.replaceAll("[1h]", "[59m]"), runbook).some((error) =>
      error.includes("ProdexGatewayAvailabilityFastBurn: missing [1h]"),
    ),
  );
}

const yaml = fs.readFileSync(ALERTS_PATH, "utf8");
const runbook = fs.readFileSync(RUNBOOK_PATH, "utf8");
if (process.argv.includes("--self-test")) {
  selfTest(yaml, runbook);
  process.stdout.write("observability-alerts-guard: self-test passed\n");
} else {
  const errors = validateObservabilityAlerts(yaml, runbook);
  if (errors.length > 0) {
    process.stderr.write(`${errors.join("\n")}\n`);
    process.exitCode = 1;
  } else {
    process.stdout.write(`observability-alerts-guard: ${alertBlocks(yaml).length} alerts validated\n`);
  }
}
