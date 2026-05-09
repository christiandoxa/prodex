#!/usr/bin/env node
import assert from "node:assert/strict";
import {
  DEFAULT_THRESHOLDS,
  structuralExtractionApplies,
  structuralExtractionGroups,
  structuralGroup,
  summarize,
  thresholdIssues,
} from "./churn-hygiene.mjs";

const thresholds = DEFAULT_THRESHOLDS;

function row(filePath, insertions, deletions) {
  return {
    filePath,
    insertions,
    deletions,
    binary: false,
  };
}

function assertStructuralExtraction(name, rows, expectedGroup) {
  const summary = summarize(rows);
  const groups = structuralExtractionGroups(rows, thresholds);
  const structuralExtraction = structuralExtractionApplies(rows, summary, thresholds);
  const issues = thresholdIssues(summary, thresholds, { structuralExtraction });

  assert.deepEqual(groups, [expectedGroup], `${name}: structural group`);
  assert.equal(structuralExtraction, true, `${name}: structural extraction applies`);
  assert.deepEqual(issues, [], `${name}: threshold issues are suppressed`);
}

assert.equal(structuralGroup("src/foo.rs"), "src/foo");
assert.equal(structuralGroup("src/foo/bar.rs"), "src/foo");
assert.equal(structuralGroup("tests/src/foo.rs"), "tests/src/foo");
assert.equal(structuralGroup("tests/src/foo/bar.rs"), "tests/src/foo");
assert.equal(structuralGroup("tests/support/foo.rs"), "tests/support/foo");
assert.equal(structuralGroup("tests/support/foo/bar.rs"), "tests/support/foo");

assertStructuralExtraction("root src module split", [
  row("src/foo.rs", 2, thresholds.maxFileLines + 20),
  row("src/foo/parser.rs", thresholds.maxFileLines + 20, 1),
], "src/foo");

assertStructuralExtraction("tests/src module split", [
  row("tests/src/foo.rs", 1, thresholds.maxFileLines + 25),
  row("tests/src/foo/cases.rs", thresholds.maxFileLines + 25, 1),
], "tests/src/foo");

assertStructuralExtraction("tests/support module split", [
  row("tests/support/foo.rs", 1, thresholds.maxFileLines + 30),
  row("tests/support/foo/helpers.rs", thresholds.maxFileLines + 30, 1),
], "tests/support/foo");

{
  const rows = [
    row("src/foo.rs", 2, thresholds.maxFileLines + 20),
    row("src/foo/parser.rs", thresholds.maxFileLines + 20, 1),
    row("crates/prodex-context/src/lib.rs", 260, 230),
    row("crates/prodex-context/src/lib/compression.rs", 260, 0),
    row("crates/prodex-runtime-store/src/smart_context_store.rs", 230, 260),
    row("crates/prodex-runtime-store/src/smart_context_store/json.rs", 0, 260),
  ];
  const summary = summarize(rows);
  const structuralExtraction = structuralExtractionApplies(rows, summary, thresholds);
  assert.deepEqual(structuralExtractionGroups(rows, thresholds), ["src/foo"]);
  assert.equal(structuralExtraction, true, "incidental extraction-sized edits remain structural");
  assert.deepEqual(
    thresholdIssues(summary, thresholds, { structuralExtraction }),
    [],
    "incidental extraction-sized edits are suppressed",
  );
}

const unrelatedLargeMove = [
  row("src/foo.rs", 1, thresholds.maxFileLines + 20),
  row("src/bar/extracted.rs", thresholds.maxFileLines + 20, 1),
];
const unrelatedSummary = summarize(unrelatedLargeMove);
const unrelatedStructuralExtraction = structuralExtractionApplies(
  unrelatedLargeMove,
  unrelatedSummary,
  thresholds,
);
assert.deepEqual(structuralExtractionGroups(unrelatedLargeMove, thresholds), []);
assert.equal(unrelatedStructuralExtraction, false);
assert.notDeepEqual(
  thresholdIssues(unrelatedSummary, thresholds, {
    structuralExtraction: unrelatedStructuralExtraction,
  }),
  [],
);

process.stdout.write("churn hygiene fixtures: passed\n");
