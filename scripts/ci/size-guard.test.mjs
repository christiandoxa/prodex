import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { promisify } from "node:util";

const SCRIPT_PATH = new URL("./size-guard.mjs", import.meta.url).pathname;
const execFileAsync = promisify(execFile);

function runNode(args, options = {}) {
  return new Promise((resolve, reject) => {
    const child = execFile(process.execPath, args, options, (error, stdout, stderr) => {
      if (error && error.code === undefined) {
        reject(error);
        return;
      }
      resolve({
        code: error?.code ?? 0,
        stdout,
        stderr,
      });
    });
    child.stdin?.end();
  });
}

async function writeLines(root, relativePath, lineCount) {
  const filePath = path.join(root, relativePath);
  await fs.mkdir(path.dirname(filePath), { recursive: true });
  await fs.writeFile(filePath, Array.from({ length: lineCount }, (_, index) => `// ${index}`).join("\n") + "\n");
}

async function writeText(root, relativePath, contents) {
  const filePath = path.join(root, relativePath);
  await fs.mkdir(path.dirname(filePath), { recursive: true });
  await fs.writeFile(filePath, contents);
}

test("default ratchets match checked-in size thresholds", async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-size-guard-"));
  try {
    await execFileAsync("git", ["init", "-q"], { cwd: root });
    const env = { ...process.env, PRODEX_REPO_ROOT: root };
    for (const name of Object.keys(env)) {
      if (name.startsWith("PRODEX_SIZE_GUARD_")) {
        delete env[name];
      }
    }

    const result = await runNode([SCRIPT_PATH, "--json"], {
      env,
    });

    assert.equal(result.code, 0);
    const payload = JSON.parse(result.stdout);
    assert.deepEqual(payload.limits, {
      production: 850,
      test: 860,
      cohesion: 770,
      maxNearLimitSiblings: 3,
      nearLimitFiles: 11,
    });
  } finally {
    await fs.rm(root, { recursive: true, force: true });
  }
});

test("full-file cfg(test) modules under src use the test line limit", async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-size-guard-"));
  try {
    await execFileAsync("git", ["init", "-q"], { cwd: root });
    await writeText(
      root,
      "src/full_file_tests.rs",
      [
        "// test-only source module",
        "#[cfg(test)]",
        "mod tests {",
        "    #[test]",
        "    fn ignores_braces_in_strings() {",
        '        assert_eq!("{", "{");',
        "    }",
        "}",
        "",
      ].join("\n"),
    );

    const result = await runNode(
      [
        SCRIPT_PATH,
        "--production-lines",
        "5",
        "--test-lines",
        "10",
        "--cohesion-lines",
        "4",
        "--near-limit-files",
        "10",
        "--json",
      ],
      { env: { ...process.env, PRODEX_REPO_ROOT: root } },
    );

    assert.equal(result.code, 0);
    const payload = JSON.parse(result.stdout);
    assert.equal(payload.files.length, 1);
    assert.equal(payload.files[0].filePath, "src/full_file_tests.rs");
    assert.equal(payload.files[0].kind, "test");
    assert.equal(payload.files[0].limit, 10);
  } finally {
    await fs.rm(root, { recursive: true, force: true });
  }
});

test("production files with cfg(test) blocks under src keep the production line limit", async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-size-guard-"));
  try {
    await execFileAsync("git", ["init", "-q"], { cwd: root });
    await writeText(
      root,
      "src/mixed.rs",
      [
        "pub fn production() -> bool {",
        "    true",
        "}",
        "",
        "#[cfg(test)]",
        "mod tests {",
        "    #[test]",
        "    fn uses_production_code() {",
        "        assert!(super::production());",
        "    }",
        "}",
        "",
      ].join("\n"),
    );

    const result = await runNode(
      [
        SCRIPT_PATH,
        "--production-lines",
        "8",
        "--test-lines",
        "20",
        "--cohesion-lines",
        "7",
        "--near-limit-files",
        "10",
        "--json",
      ],
      { env: { ...process.env, PRODEX_REPO_ROOT: root } },
    );

    assert.equal(result.code, 1);
    const payload = JSON.parse(result.stdout);
    assert.equal(payload.violations.length, 1);
    assert.equal(payload.violations[0].filePath, "src/mixed.rs");
    assert.equal(payload.violations[0].kind, "production");
    assert.equal(payload.violations[0].limit, 8);
  } finally {
    await fs.rm(root, { recursive: true, force: true });
  }
});

test("cfg(test) modules followed by production items under src stay production files", async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-size-guard-"));
  try {
    await execFileAsync("git", ["init", "-q"], { cwd: root });
    await writeText(
      root,
      "src/test_module_then_production.rs",
      [
        "#[cfg(test)]",
        "mod tests {",
        "    #[test]",
        "    fn works() {",
        "        assert_eq!(1, 1);",
        "    }",
        "}",
        "",
        "pub fn production() -> bool {",
        "    true",
        "}",
        "",
      ].join("\n"),
    );

    const result = await runNode(
      [
        SCRIPT_PATH,
        "--production-lines",
        "8",
        "--test-lines",
        "20",
        "--cohesion-lines",
        "7",
        "--near-limit-files",
        "10",
        "--json",
      ],
      { env: { ...process.env, PRODEX_REPO_ROOT: root } },
    );

    assert.equal(result.code, 1);
    const payload = JSON.parse(result.stdout);
    assert.equal(payload.violations.length, 1);
    assert.equal(payload.violations[0].filePath, "src/test_module_then_production.rs");
    assert.equal(payload.violations[0].kind, "production");
    assert.equal(payload.violations[0].limit, 8);
  } finally {
    await fs.rm(root, { recursive: true, force: true });
  }
});

test("cohesion guard fails on near-limit sibling clusters", async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-size-guard-"));
  try {
    await execFileAsync("git", ["init", "-q"], { cwd: root });
    await writeLines(root, "src/alpha.rs", 8);
    await writeLines(root, "src/beta.rs", 8);
    await writeLines(root, "src/gamma.rs", 8);

    const result = await runNode(
      [
        SCRIPT_PATH,
        "--production-lines",
        "10",
        "--test-lines",
        "12",
        "--cohesion-lines",
        "8",
        "--max-near-limit-siblings",
        "2",
        "--json",
      ],
      { env: { ...process.env, PRODEX_REPO_ROOT: root } },
    );

    assert.equal(result.code, 1);
    const payload = JSON.parse(result.stdout);
    assert.equal(payload.cohesionViolations.length, 1);
    assert.equal(payload.cohesionViolations[0].directory, "src");
    assert.deepEqual(
      payload.cohesionViolations[0].nearLimitFiles.map((file) => file.filePath).sort(),
      ["src/alpha.rs", "src/beta.rs", "src/gamma.rs"],
    );
  } finally {
    await fs.rm(root, { recursive: true, force: true });
  }
});

test("cohesion guard allows clusters at configured sibling cap", async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-size-guard-"));
  try {
    await execFileAsync("git", ["init", "-q"], { cwd: root });
    await writeLines(root, "src/alpha.rs", 8);
    await writeLines(root, "src/beta.rs", 8);

    const result = await runNode(
      [
        SCRIPT_PATH,
        "--production-lines",
        "10",
        "--test-lines",
        "12",
        "--cohesion-lines",
        "8",
        "--max-near-limit-siblings",
        "2",
        "--json",
      ],
      { env: { ...process.env, PRODEX_REPO_ROOT: root } },
    );

    assert.equal(result.code, 0);
    const payload = JSON.parse(result.stdout);
    assert.deepEqual(payload.cohesionViolations, []);
  } finally {
    await fs.rm(root, { recursive: true, force: true });
  }
});

test("near-limit budget fails when global ratchet is exceeded", async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-size-guard-"));
  try {
    await execFileAsync("git", ["init", "-q"], { cwd: root });
    await writeLines(root, "src/alpha.rs", 8);
    await writeLines(root, "src/beta.rs", 8);
    await writeLines(root, "src/gamma.rs", 8);

    const result = await runNode(
      [
        SCRIPT_PATH,
        "--production-lines",
        "10",
        "--test-lines",
        "12",
        "--cohesion-lines",
        "8",
        "--max-near-limit-siblings",
        "3",
        "--near-limit-files",
        "2",
        "--json",
      ],
      { env: { ...process.env, PRODEX_REPO_ROOT: root } },
    );

    assert.equal(result.code, 1);
    const payload = JSON.parse(result.stdout);
    assert.equal(payload.nearLimitBudgetViolations.length, 1);
    assert.equal(payload.nearLimitBudgetViolations[0].maxNearLimitFiles, 2);
    assert.deepEqual(
      payload.nearLimitBudgetViolations[0].nearLimitFiles.map((file) => file.filePath).sort(),
      ["src/alpha.rs", "src/beta.rs", "src/gamma.rs"],
    );
  } finally {
    await fs.rm(root, { recursive: true, force: true });
  }
});

test("allowlisted files use exact caps without consuming global cohesion budgets", async () => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-size-guard-"));
  try {
    await execFileAsync("git", ["init", "-q"], { cwd: root });
    await writeLines(root, "src/legacy.rs", 12);
    await writeLines(root, "src/alpha.rs", 8);
    await writeLines(root, "src/beta.rs", 8);
    await writeText(
      root,
      "scripts/ci/size-guard-allowlist.json",
      `${JSON.stringify(
        [
          {
            file: "src/legacy.rs",
            maxLines: 12,
            reason: "Legacy module has an exact ratcheted cap.",
          },
        ],
        null,
        2,
      )}\n`,
    );

    const result = await runNode(
      [
        SCRIPT_PATH,
        "--production-lines",
        "10",
        "--test-lines",
        "12",
        "--cohesion-lines",
        "8",
        "--max-near-limit-siblings",
        "2",
        "--near-limit-files",
        "2",
        "--json",
      ],
      { env: { ...process.env, PRODEX_REPO_ROOT: root } },
    );

    assert.equal(result.code, 0);
    const payload = JSON.parse(result.stdout);
    assert.equal(payload.allowlistHits.length, 1);
    assert.equal(payload.allowlistHits[0].filePath, "src/legacy.rs");
    assert.equal(payload.nearLimitFileCount, 2);
    assert.deepEqual(payload.cohesionViolations, []);
    assert.deepEqual(payload.nearLimitBudgetViolations, []);
  } finally {
    await fs.rm(root, { recursive: true, force: true });
  }
});
