import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import fs from "node:fs/promises";
import { readFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

const script = "scripts/ci/release-artifact-smoke.mjs";

test("artifact smoke requires an explicit artifact and release version", () => {
  const help = spawnSync(process.execPath, [script, "--help"], {
    cwd: process.cwd(),
    encoding: "utf8",
  });
  assert.equal(help.status, 0, help.stderr);
  assert.match(help.stdout, /--binary PATH/);
  assert.match(help.stdout, /--version VERSION/);

  const missing = spawnSync(process.execPath, [script], {
    cwd: process.cwd(),
    encoding: "utf8",
  });
  assert.notEqual(missing.status, 0);
  assert.match(missing.stderr, /--binary and --version are required/);
});

test("standalone release runs the downloaded artifact smoke before SBOM preparation", () => {
  const source = readFileSync(script, "utf8");
  const workflow = readFileSync(".github/workflows/standalone-release.yml", "utf8");
  const smoke = workflow.match(/\n  artifact-smoke:\n([\s\S]*?)\n  attest-binaries:/u)?.[1];
  const prepare = workflow.match(/\n  prepare-release:\n([\s\S]*?)\n  sync-release-docs:/u)?.[1];

  assert.ok(smoke, "artifact smoke job missing");
  assert.ok(prepare, "release preparation job missing");
  assert.match(smoke, /- verify-ci/);
  assert.match(smoke, /- build/);
  assert.match(smoke, /name: x86_64-unknown-linux-gnu/);
  assert.match(smoke, /binary="artifact\/prodex"/u);
  assert.match(smoke, /node scripts\/ci\/release-artifact-smoke\.mjs \\\n\s+--binary/u);
  assert.match(prepare, /- artifact-smoke/);
  assert.doesNotMatch(smoke, /cargo\s+(run|build)/u);
  assert.doesNotMatch(smoke, /codex/iu);
  assert.doesNotMatch(workflow, /Build patched Codex runtime|codex_binary|codex-[^*]/u);
  assert.match(workflow, /-iname '\*codex\*'/u);
  assert.doesNotMatch(source, /target\/(?:debug|release)/u);
});

test("release manifest, checksums, and SBOM accept Prodex assets only", () => {
  const renderer = readFileSync("scripts/release/render-release-manifest.mjs", "utf8");
  const workflow = readFileSync(".github/workflows/standalone-release.yml", "utf8");
  const prepare = workflow.match(/\n  prepare-release:\n([\s\S]*?)\n  sync-release-docs:/u)?.[1];
  const release = workflow.match(/\n  publish-github-release:\n([\s\S]*)/u)?.[1];

  assert.ok(prepare, "release preparation job missing");
  assert.ok(release, "release publication job missing");
  assert.match(renderer, /\^prodex-\[A-Za-z0-9\._-\]\+/u);
  assert.match(renderer, /codex/iu);
  assert.match(prepare, /find artifacts .* -name 'prodex' .* -name 'prodex\.exe'/u);
  assert.match(prepare, /sbom-input:\/source:ro/u);
  assert.match(release, /find artifacts .* -name 'prodex' .* -name 'prodex\.exe'/u);
  assert.match(release, /sha256sum install\.sh install\.ps1 release-manifest\.tsv release-manifest\.json prodex-\* release-sbom\.spdx\.json/u);
  assert.match(release, /release assets must not contain Codex executables or bundles/u);
});

test("release stages immutable artifacts and waits before public version mutations", () => {
  const workflow = readFileSync(".github/workflows/standalone-release.yml", "utf8");
  const container = workflow.match(/\n  publish-container:\n([\s\S]*?)\n  prepare-release:/u)?.[1];
  const publish = workflow.match(/\n  publish-github-release:\n([\s\S]*?)\n  publish-duration-telemetry:/u)?.[1];

  assert.ok(container, "container staging job missing");
  assert.ok(publish, "release publication job missing");
  assert.match(workflow, /publish_at:/u);
  assert.match(container, /candidate_tag="sha-\$\{TARGET_SHA\}"/u);
  assert.doesNotMatch(container, /docker push "\$\{image\}:\$\{VERSION\}"/u);
  assert.match(publish, /--checkpoint P --release-sha/u);
  assert.match(publish, /docker buildx imagetools create/u);
  assert.match(publish, /--prefer-index=false/u);
  assert.match(publish, /refusing to move it/u);
  assert.doesNotMatch(publish, /--clobber|gh release edit/u);
  assert.ok(
    publish.indexOf("Wait for the absolute publication target") <
      publish.indexOf("Create release tag"),
  );
});

test("release manifest renderer rejects Codex executable names", async (t) => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), "prodex-release-manifest-test-"));
  t.after(() => fs.rm(root, { recursive: true, force: true }));
  const input = path.join(root, "matrix.tsv");
  await fs.writeFile(
    input,
    "x86_64-unknown-linux-gnu\tcodex-x86_64-unknown-linux-gnu\trust\t\t\tfalse\tGLIBC_2.23\n",
  );
  const result = spawnSync(
    process.execPath,
    [
      "scripts/release/render-release-manifest.mjs",
      "--version",
      "0.426.1",
      "--commit",
      "0".repeat(40),
      "--input",
      input,
      "--output-tsv",
      path.join(root, "release-manifest.tsv"),
      "--output-json",
      path.join(root, "release-manifest.json"),
    ],
    { cwd: process.cwd(), encoding: "utf8" },
  );
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /invalid target or asset/iu);
});
