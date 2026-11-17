import assert from "node:assert/strict";
import { readFileSync, mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import test from "node:test";

const root = fileURLToPath(new URL("../../", import.meta.url));
const baseline = JSON.parse(readFileSync(join(root, "scripts/compat/upstream-baseline.json"), "utf8"));

for (const [id, needle] of [
  ["models.gpt62-native-contract", '"slug": "gpt-6.2-sol"'],
  ["responses.provider-output-normalization", "provider.normalize_completed_output_item(&mut item);"],
  ["bedrock.mantle-commentary-replay", "*annotations = None;"],
]) {
  test(`upstream guard rejects a missing ${id} contract`, () => {
    const directory = mkdtempSync(join(tmpdir(), "prodex-gpt62-contract-"));
    try {
      const mutated = structuredClone(baseline);
      const check = mutated.codex.compatibility.semantic_checks.find((entry) => entry.id === id);
      assert.ok(check?.file_contains_all.includes(needle));
      check.file_contains_all = check.file_contains_all.filter((value) => value !== needle);
      const path = join(directory, "baseline.json");
      writeFileSync(path, JSON.stringify(mutated));
      const result = spawnSync(process.execPath, ["scripts/compat/check-upstream-baseline.mjs", "--baseline", path], { cwd: root, encoding: "utf8" });
      assert.equal(result.status, 1, result.stdout + result.stderr);
      assert.match(result.stdout + result.stderr, new RegExp(id.replaceAll(".", "\\.")));
    } finally {
      rmSync(directory, { recursive: true, force: true });
    }
  });
}
