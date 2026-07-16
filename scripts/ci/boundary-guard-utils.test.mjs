import assert from "node:assert/strict";
import test from "node:test";
import { parseDependencySections } from "./boundary-guard-utils.mjs";

test("parses dependency sections and preserves hashes inside strings", () => {
  const sections = parseDependencySections(`
[dependencies]
serde = "1" # comment
url = { git = "https://example.com/repo#main" }

[dev-dependencies]
tempfile = "3"
`);

  assert.deepEqual([...sections.get("dependencies")], ["serde", "url"]);
  assert.deepEqual([...sections.get("dev-dependencies")], ["tempfile"]);
});
