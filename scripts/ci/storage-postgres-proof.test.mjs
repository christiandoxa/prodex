import assert from "node:assert/strict";
import test from "node:test";

import {
  MISSING_DEPENDENCY_MESSAGE,
  selectProofMode,
} from "./storage-postgres-proof.mjs";

test("selectProofMode uses explicit Postgres URL first", () => {
  assert.equal(
    selectProofMode({
      postgresUrl: "postgres://example",
      dockerAvailable: false,
      psqlAvailable: false,
    }),
    "direct",
  );
});

test("selectProofMode uses managed mode when docker and psql are available", () => {
  assert.equal(
    selectProofMode({
      postgresUrl: "",
      dockerAvailable: true,
      psqlAvailable: true,
    }),
    "managed",
  );
});

test("selectProofMode uses stable missing dependency message", () => {
  assert.throws(
    () =>
      selectProofMode({
        postgresUrl: "",
        dockerAvailable: true,
        psqlAvailable: false,
      }),
    (error) => {
      assert.ok(String(error).includes(MISSING_DEPENDENCY_MESSAGE));
      return true;
    },
  );
});
