import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import test from "node:test";
import { validateWorkflow } from "./supply-chain-guard.mjs";

const workflows = [
  "release-target.yml", "release-macos-cross-calibration.yml",
  "release-windows-arm64-calibration.yml", "release-windows-x64-arm-host-calibration.yml",
  "release-windows-x64-codegen-calibration.yml",
];
function cache(file) {
  const source = readFileSync(`.github/workflows/${file}`, "utf8");
  const block = source.split(/\n      - name:/u).find((value) => value.includes("Swatinem/rust-cache@"));
  assert.ok(block, `missing dependency cache in ${file}`);
  return block;
}
function identity(file, overrides = {}) {
  const block = cache(file);
  // Pinned d1527c8 src/config.ts: shared-key takes precedence; key is ignored.
  const shared = block.match(/^          shared-key: (.+)$/mu)?.[1];
  const additional = block.match(/^          key: (.+)$/mu)?.[1];
  const values = {
    "env.MOJO_VERSION": "1.2.0", "env.PRODEX_MOJO_VERSION": "1.2.0",
    "runner.os": "Windows", "runner.arch": "ARM64",
    "matrix.target": "x86_64-pc-windows-msvc", "matrix.mojo-target-cpu": "x86-64",
    "matrix.units": "16", "steps.rust-deps-cache-key.outputs.external_sha256": "a".repeat(64),
    ...overrides,
  };
  assert.ok(shared || additional);
  return (shared || additional).replace(/\$\{\{ (.*?) \}\}/gu, (_, expression) => {
    if (Object.hasOwn(values, expression)) return values[expression];
    const conditional = expression.match(/^matrix.target == '([^']+)' && '([^']+)' \|\| ''$/u);
    assert.ok(conditional, `unrecognized cache expression: ${expression}`);
    return values["matrix.target"] === conditional[1] ? conditional[2] : "";
  });
}

test("release and calibration use the discriminated shared key, never an ignored additional key", () => {
  for (const file of workflows) {
    const block = cache(file);
    assert.match(block, /shared-key: production-release-deps-v2-rust1\.99\.1-thinlto-cgu/);
    assert.doesNotMatch(block, /^          key:/mu, `${file}: additional key would be ignored`);
    assert.match(block, /cache-workspace-crates: "false"/);
    assert.match(block, /outputs.external_sha256/);
    assert.doesNotMatch(block, /restore-keys:/);
  }
});
test("different release targets, CPUs, dependencies, and codegen profiles cannot collide", () => {
  const base = identity("release-target.yml");
  for (const overrides of [
    { "matrix.target": "aarch64-pc-windows-msvc", "matrix.mojo-target-cpu": "generic" },
    { "matrix.mojo-target-cpu": "generic" }, { "env.MOJO_VERSION": "2.0.0" },
    { "steps.rust-deps-cache-key.outputs.external_sha256": "b".repeat(64) },
    { "runner.arch": "X64" },
  ]) assert.notEqual(identity("release-target.yml", overrides), base);
  assert.notEqual(identity("release-windows-x64-codegen-calibration.yml", { "matrix.units": "1" }),
    identity("release-windows-x64-codegen-calibration.yml", { "matrix.units": "16" }));
});
test("only identical production and calibration target profiles share a namespace", () => {
  assert.equal(identity("release-target.yml"), identity("release-windows-x64-arm-host-calibration.yml"));
  assert.equal(identity("release-target.yml"), identity("release-windows-x64-codegen-calibration.yml"));
  assert.equal(identity("release-target.yml", { "matrix.target": "aarch64-pc-windows-msvc", "matrix.mojo-target-cpu": "generic" }),
    identity("release-windows-arm64-calibration.yml"));
  const apple = { "runner.os": "macOS", "matrix.target": "x86_64-apple-darwin" };
  assert.equal(identity("release-target.yml", apple), identity("release-macos-cross-calibration.yml", apple));
});

test("no Rust cache action silently discards a target/input key in any pipeline", () => {
  for (const file of readdirSync(".github/workflows").filter((name) => name.endsWith(".yml"))) {
    const source = readFileSync(`.github/workflows/${file}`, "utf8");
    for (const block of source.split(/\n      - name:/u)) {
      if (!block.includes("uses: Swatinem/rust-cache@")) continue;
      const shared = /^          shared-key: /mu.test(block);
      const additional = /^          key: /mu.test(block);
      assert.ok(!(shared && additional), `${file}: additional key is ignored when shared-key is present`);
    }
  }
});

test("supply-chain guard rejects ambiguous cache inputs rather than trusting a decorative key", () => {
  const valid = `jobs:
  cache:
    steps:
      - name: Cache compiled dependencies
        uses: Swatinem/rust-cache@d1527c8f67ef9af357290475d4af4b0fab8f12e0 # v2
        with:
          shared-key: source-and-target-bound
`;
  assert.deepEqual(validateWorkflow("test.yml", valid), []);
  const invalid = valid + "          key: ignored-target-discriminator\n";
  assert.ok(validateWorkflow("test.yml", invalid).some((error) => error.includes("key is ignored")));
});
