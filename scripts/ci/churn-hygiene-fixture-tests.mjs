#!/usr/bin/env node
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  DEFAULT_THRESHOLDS,
  mechanicalOnlyDeclared,
  structuralExtractionApplies,
  structuralExtractionAcceptedByDeclaration,
  structuralExtractionDeclarationIssues,
  structuralExtractionGroups,
  structuralGroup,
  summarize,
  thresholdIssues,
} from "./churn-hygiene.mjs";

const thresholds = DEFAULT_THRESHOLDS;
const churnScript = fileURLToPath(new URL("./churn-hygiene.mjs", import.meta.url));

function row(filePath, insertions, deletions) {
  return {
    filePath,
    insertions,
    deletions,
    binary: false,
  };
}

function largeRustModule(name, lines = thresholds.maxFileLines + 140) {
  return Array.from({ length: lines }, (_, index) => `pub fn ${name}_${index}() {}`).join("\n") + "\n";
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: options.cwd,
    encoding: "utf8",
    env: options.env ?? process.env,
  });
  if (options.expectFailure) {
    assert.notEqual(result.status, 0, `${command} ${args.join(" ")} should fail`);
    return result;
  }
  assert.equal(
    result.status,
    0,
    `${command} ${args.join(" ")} failed\nstdout:\n${result.stdout}\nstderr:\n${result.stderr}`,
  );
  return result;
}

function git(repo, args) {
  return run("git", args, { cwd: repo }).stdout.trim();
}

function writeFile(repo, relativePath, contents) {
  const fullPath = path.join(repo, relativePath);
  fs.mkdirSync(path.dirname(fullPath), { recursive: true });
  fs.writeFileSync(fullPath, contents);
}

function commit(repo, subject) {
  git(repo, ["add", "-A"]);
  git(repo, ["commit", "-m", subject]);
  return git(repo, ["rev-parse", "HEAD"]);
}

function runChurn(repo, args, options = {}) {
  return run(process.execPath, [churnScript, ...args], {
    env: {
      ...process.env,
      PRODEX_CHURN_HYGIENE_IGNORE_BEFORE: options.ignoreBeforeEnv ?? "",
      PRODEX_CHURN_HYGIENE_REPORT_ONLY: "",
      PRODEX_REPO_ROOT: repo,
    },
    expectFailure: options.expectFailure,
  });
}

function assertStructuralExtraction(name, rows, expectedGroup) {
  const summary = summarize(rows);
  const groups = structuralExtractionGroups(rows, thresholds);
  const structuralExtraction = structuralExtractionApplies(rows, summary, thresholds);
  const declaredCommits = [
    {
      message: "refactor: split module\n\nMechanical-only: yes\n",
    },
  ];
  const structuralExtractionAccepted = structuralExtractionAcceptedByDeclaration(
    structuralExtraction,
    summary,
    thresholds,
    declaredCommits,
  );
  const issues = thresholdIssues(summary, thresholds, { structuralExtractionAccepted });

  assert.deepEqual(groups, [expectedGroup], `${name}: structural group`);
  assert.equal(structuralExtraction, true, `${name}: structural extraction applies`);
  assert.equal(structuralExtractionAccepted, true, `${name}: declaration accepts structural extraction`);
  assert.deepEqual(issues, [], `${name}: threshold issues are suppressed`);
}

assert.equal(mechanicalOnlyDeclared("refactor: split module\n\nMechanical-only: yes\n"), true);
assert.equal(mechanicalOnlyDeclared("refactor: split module\n\nMechanical-only: true\n"), true);
assert.equal(mechanicalOnlyDeclared("refactor: split module [mechanical-only]\n"), true);
assert.equal(mechanicalOnlyDeclared("refactor: split module\n"), false);

assert.equal(structuralGroup("src/foo.rs"), "src/foo");
assert.equal(structuralGroup("src/foo/bar.rs"), "src/foo");
assert.equal(structuralGroup("tests/src/foo.rs"), "tests/src/foo");
assert.equal(structuralGroup("tests/src/foo/bar.rs"), "tests/src/foo");
assert.equal(structuralGroup("tests/support/foo.rs"), "tests/support/foo");
assert.equal(structuralGroup("tests/support/foo/bar.rs"), "tests/support/foo");
assert.equal(
  summarize([row("crates/prodex-app/src/runtime_proxy.rs", 1, 0)]).behaviorFiles,
  1,
  "crate Rust files count as behavior files",
);

{
  const summary = summarize([row("README.md", thresholds.maxFileLines + 68, 0)]);
  assert.equal(summary.releaseMetadataOnly, true, "README-only churn is metadata/docs-only");
  assert.deepEqual(
    thresholdIssues(summary, thresholds),
    [],
    "metadata/docs-only churn is exempt from generic size thresholds",
  );
}

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
  const rows = Array.from({ length: thresholds.maxBehaviorFiles + 1 }, (_, index) => [
    row(`crates/prodex-fixture-${index}/src/lib.rs`, 1, thresholds.maxFileLines + 20),
    row(`crates/prodex-fixture-${index}/src/extracted.rs`, thresholds.maxFileLines + 20, 1),
  ]).flat();
  const summary = summarize(rows);
  const structuralExtraction = structuralExtractionApplies(rows, summary, thresholds);
  const structuralExtractionAccepted = structuralExtractionAcceptedByDeclaration(
    structuralExtraction,
    summary,
    thresholds,
    [
      {
        subject: "refactor: split runtime support modules",
        message: "refactor: split runtime support modules\n",
      },
    ],
  );
  assert.equal(
    summary.behaviorFiles > thresholds.maxBehaviorFiles,
    true,
    "multi-crate extraction fixture exceeds behavior-file threshold",
  );
  assert.equal(structuralExtraction, true, "multi-crate extraction remains structural");
  assert.equal(
    structuralExtractionAccepted,
    true,
    "mechanical split subject accepts structural extraction without a footer",
  );
  assert.deepEqual(
    thresholdIssues(summary, thresholds, { structuralExtractionAccepted }),
    [],
    "accepted extraction suppresses behavior-file churn caused by extraction rows",
  );
}

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
  const structuralExtractionAccepted = structuralExtractionAcceptedByDeclaration(
    structuralExtraction,
    summary,
    thresholds,
    [
      {
        message: "refactor: split module\n\nMechanical-only: yes\n",
      },
    ],
  );
  assert.deepEqual(structuralExtractionGroups(rows, thresholds), ["src/foo"]);
  assert.equal(structuralExtraction, true, "incidental extraction-sized edits remain structural");
  assert.equal(structuralExtractionAccepted, true, "declared incidental extraction is accepted");
  assert.deepEqual(
    thresholdIssues(summary, thresholds, { structuralExtractionAccepted }),
    [],
    "incidental extraction-sized edits are suppressed",
  );
}

{
  const rows = [
    row("src/foo.rs", 2, thresholds.maxFileLines + 20),
    row("src/foo/parser.rs", thresholds.maxFileLines + 20, 1),
  ];
  const summary = summarize(rows);
  const structuralExtraction = structuralExtractionApplies(rows, summary, thresholds);
  const structuralExtractionAccepted = structuralExtractionAcceptedByDeclaration(
    structuralExtraction,
    summary,
    thresholds,
    [],
  );
  assert.equal(structuralExtraction, true, "undeclared structural extraction is recognized");
  assert.equal(structuralExtractionAccepted, false, "undeclared structural extraction is not accepted");
  assert.notDeepEqual(
    thresholdIssues(summary, thresholds, { structuralExtractionAccepted }),
    [],
    "undeclared structural extraction still reports threshold issues",
  );
  assert.notDeepEqual(
    structuralExtractionDeclarationIssues(
      summary,
      thresholds,
      structuralExtraction,
      structuralExtractionAccepted,
      [],
    ),
    [],
    "undeclared structural extraction reports declaration guidance",
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

{
  const repo = fs.mkdtempSync(path.join(os.tmpdir(), "prodex-churn-hygiene-"));
  try {
    git(repo, ["init", "-q"]);
    git(repo, ["config", "user.email", "fixtures@example.test"]);
    git(repo, ["config", "user.name", "Prodex Fixtures"]);

    writeFile(repo, "src/foo.rs", largeRustModule("foo"));
    writeFile(repo, "src/bar.rs", largeRustModule("bar"));
    const base = commit(repo, "test: seed large modules");

    writeFile(repo, "src/foo.rs", "pub mod parser;\n");
    writeFile(repo, "src/foo/parser.rs", largeRustModule("foo_parser"));
    const baseline = commit(repo, "refactor: split foo module");

    writeFile(repo, "src/bar.rs", "pub mod parser;\n");
    writeFile(repo, "src/bar/parser.rs", largeRustModule("bar_parser"));
    const head = commit(repo, "refactor: split bar module");

    const baselineRun = runChurn(repo, [
      "--range",
      `${base}..${head}`,
      "--ignore-before",
      baseline,
      "--json",
      "--check",
    ]);
    const baselineReport = JSON.parse(baselineRun.stdout);
    assert.equal(baselineReport.originalSelector, `${base}..${head}`);
    assert.equal(baselineReport.selector, `${baseline}..${head}`);
    assert.equal(baselineReport.structuralExtraction, true);
    assert.equal(baselineReport.structuralExtractionAccepted, true);
    assert.deepEqual(baselineReport.issues, []);
    assert.deepEqual(baselineReport.declarationIssues, []);

    const strictBaseHeadRun = runChurn(
      repo,
      ["--base", base, "--head", head, "--dry-run"],
      {
        expectFailure: true,
        ignoreBeforeEnv: baseline,
      },
    );
    assert.match(
      strictBaseHeadRun.stderr,
      /--ignore-before is only supported with --range/,
      "env baseline must not weaken --base/--head guard ranges",
    );
  } finally {
    fs.rmSync(repo, { recursive: true, force: true });
  }
}

process.stdout.write("churn hygiene fixtures: passed\n");
