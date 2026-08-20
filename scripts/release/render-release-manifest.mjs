#!/usr/bin/env node

import fs from "node:fs";

const args = process.argv.slice(2);
const valueFor = (name) => {
  const index = args.indexOf(name);
  if (index < 0 || !args[index + 1]) throw new Error(`missing ${name}`);
  return args[index + 1];
};

const version = valueFor("--version");
const commit = valueFor("--commit");
const input = valueFor("--input");
const outputTsv = valueFor("--output-tsv");
const outputJson = valueFor("--output-json");
const rows = [];
for (const rawLine of fs.readFileSync(input, "utf8").split(/\r?\n/)) {
  if (!rawLine || rawLine.startsWith("#")) continue;
  const fields = rawLine.split("\t");
  if (fields.length !== 7) throw new Error(`invalid release target row: ${rawLine}`);
  const [target, asset, implementation, mojoVersion, mojoFeatures, runtimeBundle, minimumGlibcValue] = fields;
  const minimumGlibc = minimumGlibcValue === "-" ? "" : minimumGlibcValue;
  if (!/^[A-Za-z0-9._-]+$/.test(target) || !asset.startsWith("prodex-")) {
    throw new Error(`invalid target or asset: ${rawLine}`);
  }
  if (!["rust", "mojo-compiled-in", "mojo-bundled-runtime"].includes(implementation)) {
    throw new Error(`invalid implementation: ${implementation}`);
  }
  if (!["true", "false"].includes(runtimeBundle)) throw new Error(`invalid runtime flag: ${rawLine}`);
  if (implementation === "mojo-compiled-in" && (!mojoVersion || !mojoFeatures)) {
    throw new Error(`Mojo row needs version and feature set: ${rawLine}`);
  }
  rows.push({ target, asset, implementation, mojoVersion: mojoVersion || null, mojoFeatures: mojoFeatures ? mojoFeatures.split(",") : [], runtimeBundle: runtimeBundle === "true", minimumGlibc: minimumGlibc || null });
}
if (rows.length === 0) throw new Error("release target matrix is empty");

const tsv = [
  "# Prodex release capability manifest",
  `schema_version\t1`,
  `version\t${version}`,
  `commit\t${commit}`,
  "target\tasset\timplementation\tmojo_version\tmojo_features\truntime_bundle\tminimum_glibc",
  ...rows.map((row) => [row.target, row.asset, row.implementation, row.mojoVersion ?? "", row.mojoFeatures.join(","), row.runtimeBundle ? "true" : "false", row.minimumGlibc ?? "-"].join("\t")),
  "",
].join("\n");
fs.writeFileSync(outputTsv, tsv);

const manifest = {
  schema_version: 1,
  version,
  commit,
  artifacts: Object.fromEntries(rows.map((row) => [row.target, {
    asset: row.asset,
    implementation: row.implementation,
    mojo_version: row.mojoVersion,
    mojo_features: row.mojoFeatures,
    runtime_bundle: row.runtimeBundle,
    minimum_glibc: row.minimumGlibc,
  }])),
};
fs.writeFileSync(outputJson, `${JSON.stringify(manifest, null, 2)}\n`);
