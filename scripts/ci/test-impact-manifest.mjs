export const TEST_IMPACT_MANIFEST_PATH = "scripts/ci/test-impact-manifest.mjs";

export const VERSION_SYNC_PATHS = Object.freeze([
  "Cargo.toml",
  "npm",
  "README.md",
  "QUICKSTART.md",
  "scripts/npm",
]);

export const WATCH_UPSTREAM_FIXTURE_TESTS_PATH = "scripts/compat/watch-upstream-fixture-tests.mjs";

export const UPSTREAM_COMPAT_SCRIPT_PATHS = Object.freeze([
  "scripts/compat/check-upstream-baseline.mjs",
  "scripts/compat/upstream-compat-common.mjs",
  "scripts/compat/upstream-compat-summary.mjs",
  "scripts/compat/watch-upstream-ci.mjs",
  "scripts/compat/watch-upstream.mjs",
  WATCH_UPSTREAM_FIXTURE_TESTS_PATH,
]);

export const PACKAGE_SCRIPT_ALIASES = Object.freeze({
  "ci:changed": "node scripts/ci/changed-tests.mjs",
  "test:changed": "node scripts/ci/changed-tests.mjs",
  "ci:allow-guard": "node scripts/ci/allow-attribute-guard.mjs",
  "ci:size-guard": "node scripts/ci/size-guard.mjs",
  "ci:churn-hygiene-fixtures": "node scripts/ci/churn-hygiene-fixture-tests.mjs",
  "ci:release-hygiene": "node scripts/ci/release-hygiene.mjs",
  "ci:release-cut-fixtures": "node scripts/ci/release-cut-fixture-tests.mjs",
  "release:cut": "node scripts/npm/release-cut.mjs",
  "compat:check": "node scripts/compat/check-upstream-baseline.mjs",
  "compat:watch": "node scripts/compat/watch-upstream.mjs",
  "compat:watch-fixtures": "node scripts/compat/watch-upstream-fixture-tests.mjs",
  "compat:watch-ci": "node scripts/compat/watch-upstream-ci.mjs",
});

export const PATH_GROUPS = Object.freeze({
  smartContext: {
    exact: [
      "crates/prodex-app/src/runtime_proxy/smart_context.rs",
      "crates/prodex-app/tests/src/runtime_proxy/smart_context.rs",
    ],
    prefixes: [
      "crates/prodex-app/src/runtime_proxy/smart_context/",
      "crates/prodex-app/tests/src/runtime_proxy/smart_context/",
    ],
  },
  runtimeHotPath: {
    exact: ["crates/prodex-app/src/runtime_launch/proxy_startup.rs"],
    prefixes: [
      "crates/prodex-app/src/runtime_proxy/",
      "crates/prodex-runtime-proxy/src/",
    ],
  },
  versionManaged: {
    exact: ["Cargo.toml", "README.md", "QUICKSTART.md"],
    prefixes: ["npm/", "scripts/npm/"],
  },
  sizeGuardRelevant: {
    exact: [
      "package.json",
      "scripts/ci/allow-attribute-guard.mjs",
      "scripts/ci/guard-common.mjs",
      "scripts/ci/size-guard.mjs",
    ],
    suffixes: [".rs"],
  },
  releaseGuardRelevant: {
    exact: [
      "package.json",
      ".github/workflows/ci.yml",
      "scripts/ci/changelog-noise-guard.mjs",
      "scripts/ci/preflight.mjs",
      "scripts/ci/prepush.mjs",
      "scripts/ci/release-hygiene.mjs",
      "scripts/ci/release-guard-common.mjs",
      "scripts/ci/release-guard-fixture-tests.mjs",
      "scripts/ci/release-cut-fixture-tests.mjs",
      "scripts/npm/release-cut.mjs",
      TEST_IMPACT_MANIFEST_PATH,
    ],
    prefixes: ["scripts/ci/"],
    suffixes: [".mjs"],
    fileNameIncludesAll: ["release", "guard"],
  },
  upstreamCompatRelevant: {
    exact: ["package.json", ".github/workflows/upstream-compat.yml", TEST_IMPACT_MANIFEST_PATH],
    prefixes: ["scripts/compat/"],
  },
  runtimeManifestRelevant: {
    exact: [
      ".github/workflows/ci.yml",
      "scripts/ci/runtime-test-manifest.mjs",
      "scripts/ci/runtime-test-manifest-guard.mjs",
      TEST_IMPACT_MANIFEST_PATH,
    ],
  },
  runtimePolicyDocsRelevant: {
    exact: ["docs/runtime-policy.md", "scripts/docs/runtime-policy.mjs"],
    prefixes: ["crates/prodex-runtime-policy/"],
  },
  churnHygieneRelevant: {
    exact: [
      "package.json",
      "scripts/ci/changed-tests.mjs",
      "scripts/ci/churn-hygiene.mjs",
      "scripts/ci/churn-hygiene-fixture-tests.mjs",
      TEST_IMPACT_MANIFEST_PATH,
    ],
  },
});

export function pathMatchesSpec(filePath, spec) {
  if (spec.exact?.includes(filePath)) {
    return true;
  }
  if (spec.prefixes || spec.suffixes || spec.fileNameIncludesAll) {
    const prefixMatch = !spec.prefixes || spec.prefixes.some((prefix) => filePath.startsWith(prefix));
    const suffixMatch = !spec.suffixes || spec.suffixes.some((suffix) => filePath.endsWith(suffix));
    const fileName = filePath.split("/").pop() ?? "";
    const fileNameMatch =
      !spec.fileNameIncludesAll ||
      spec.fileNameIncludesAll.every((needle) => fileName.includes(needle));
    return prefixMatch && suffixMatch && fileNameMatch;
  }
  return false;
}
