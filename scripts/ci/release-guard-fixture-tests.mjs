#!/usr/bin/env node
import { spawn } from "node:child_process";
import { repoRoot } from "../npm/common.mjs";

const FIXTURES = Object.freeze([
  {
    name: "mixed release metadata fails metadata-only guard",
    script: "release-metadata-only-guard.mjs",
    args: ["--commit", "72c710c"],
    expectedExit: 1,
  },
  {
    name: "mixed release metadata fails version metadata guard",
    script: "version-metadata-release-guard.mjs",
    args: ["--commit", "72c710c"],
    expectedExit: 1,
  },
  {
    name: "empty release commit fails empty commit guard",
    script: "release-empty-commit-guard.mjs",
    args: ["--commit", "7c80038"],
    expectedExit: 1,
  },
  {
    name: "duplicate release 0.89.0 range fails duplicate guard",
    script: "release-duplicate-version-guard.mjs",
    args: ["--range", "7c80038^..45f3ecd"],
    expectedExit: 1,
  },
  {
    name: "tag 0.91.0 has changelog section",
    script: "release-tag-changelog-guard.mjs",
    args: ["--rev", "0.91.0"],
    expectedExit: 0,
  },
  {
    name: "current head range has matching tag changelog state",
    script: "release-tag-changelog-guard.mjs",
    args: ["--range", "HEAD^..HEAD"],
    expectedExit: 0,
  },
  {
    name: "normal prepare and release range passes duplicate guard",
    script: "release-duplicate-version-guard.mjs",
    args: ["--range", "b9a6122^..41397d2"],
    expectedExit: 0,
  },
]);

function runGuard({ script, args }) {
  return new Promise((resolve, reject) => {
    const commandArgs = [`scripts/ci/${script}`, ...args];
    const child = spawn(process.execPath, commandArgs, {
      cwd: repoRoot,
      stdio: ["ignore", "pipe", "pipe"],
    });

    let stdout = "";
    let stderr = "";
    child.stdout.setEncoding("utf8");
    child.stderr.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    child.on("error", reject);
    child.on("close", (code, signal) => {
      resolve({
        code: signal ? null : code,
        command: [process.execPath, ...commandArgs].join(" "),
        signal,
        stdout: stdout.trim(),
        stderr: stderr.trim(),
      });
    });
  });
}

function printFailure(fixture, result) {
  process.stderr.write(
    [
      `not ok - ${fixture.name}`,
      `  expected exit ${fixture.expectedExit}, got ${result.signal ? `signal ${result.signal}` : result.code}`,
      `  command: ${result.command}`,
      result.stdout ? `  stdout: ${result.stdout}` : null,
      result.stderr ? `  stderr: ${result.stderr}` : null,
    ]
      .filter(Boolean)
      .join("\n") + "\n",
  );
}

let failures = 0;
for (const fixture of FIXTURES) {
  const result = await runGuard(fixture);
  if (!result.signal && result.code === fixture.expectedExit) {
    process.stdout.write(`ok - ${fixture.name}\n`);
    continue;
  }

  failures += 1;
  printFailure(fixture, result);
}

if (failures > 0) {
  process.stderr.write(`release guard fixtures: ${failures} failed\n`);
  process.exitCode = 1;
} else {
  process.stdout.write(`release guard fixtures: ${FIXTURES.length} passed\n`);
}
