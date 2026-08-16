import assert from "node:assert/strict";
import test from "node:test";
import { cargoLockVersionErrors, npmLockVersionErrors } from "./release-prepare.mjs";

test("release prep checks every workspace Cargo lock entry", () => {
  const lock = `version = 4

[[package]]
name = "prodex"
version = "0.2.0"

[[package]]
name = "prodex-app"
version = "0.2.0"

[[package]]
name = "serde"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
`;

  assert.deepEqual(cargoLockVersionErrors(lock, ["prodex", "prodex-app"], "0.2.0"), []);
  assert.match(
    cargoLockVersionErrors(
      lock.replace('version = "0.2.0"', 'version = "0.1.0"'),
      ["prodex", "prodex-app"],
      "0.2.0",
    ).join("\n"),
    /Cargo\.lock prodex version/,
  );
});

test("release prep checks the gateway SDK package-lock entry", () => {
  const lock = {
    packages: {
      "npm/prodex-gateway-sdk": {
        name: "@christiandoxa/prodex-gateway-sdk",
        version: "0.1.0",
      },
    },
  };

  assert.match(
    npmLockVersionErrors(lock, "package-lock.json", "0.2.0").join("\n"),
    /@christiandoxa\/prodex-gateway-sdk lock version/,
  );
});
