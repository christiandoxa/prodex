import assert from "node:assert/strict";
import test from "node:test";
import { runCheckedJson } from "./checked-subprocess.mjs";

test("reports a missing executable before JSON parsing", () => {
  assert.throws(
    () => runCheckedJson("prodex-command-that-does-not-exist"),
    (error) => error.kind === "could not start" && /executable not found/u.test(error.message),
  );
});

test("reports a non-zero exit with bounded redacted stderr", () => {
  assert.throws(
    () =>
      runCheckedJson(process.execPath, [
        "-e",
        "console.error('Authorization: Bearer fixture-secret'); process.exit(7)",
      ]),
    (error) =>
      error.kind === "failed" &&
      error.status === 7 &&
      error.message.includes("Bearer <redacted>") &&
      !error.message.includes("fixture-secret"),
  );
});

test("parses JSON only after successful execution", () => {
  assert.throws(
    () => runCheckedJson(process.execPath, ["-e", "console.log('not-json')"]),
    (error) => error.kind === "returned invalid JSON",
  );
});

test("terminates a timed-out subprocess", () => {
  assert.throws(
    () =>
      runCheckedJson(process.execPath, ["-e", "setTimeout(() => {}, 1000)"], {
        timeoutMs: 20,
      }),
    (error) => error.kind === "timed out",
  );
});
