import assert from "node:assert/strict";
import test from "node:test";
import { calculateOwnership, countSemanticLines } from "./mojo-ownership.mjs";

test("Mojo ownership counter excludes comments, imports, and Rust test modules", () => {
  assert.equal(
    countSemanticLines(
      "// comment\nuse std::fmt;\n#[cfg(test)]\nmod tests {\nfn ignored() {}\n}\nfn production() {}\n",
      "rust",
    ),
    1,
  );
  assert.equal(
    countSemanticLines("# comment\nfrom std import Pointer\n@export(\"x\")\ndef production():\n    return 1\n", "mojo"),
    2,
  );
});

test("ownership result is deterministic for an explicit inventory", () => {
  const manifest = {
    rust_deterministic_sources: [],
    mojo_deterministic_sources: [],
    authoritative_operations: [],
  };
  const first = calculateOwnership(manifest, "HEAD", "HEAD");
  const second = calculateOwnership(manifest, "HEAD", "HEAD");
  assert.deepEqual(first, second);
});
