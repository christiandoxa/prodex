export const TEST_IMPACT_MANIFEST_PATH = "scripts/ci/test-impact-manifest.mjs";
export const RELEASE_RUN_TEST_PATH = "scripts/npm/release-run.test.mjs";

export const SEMVER_SOURCE = String.raw`v?([0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?)`;

export const VERSION_SYNC_PATHS = Object.freeze([
  "Cargo.toml",
  "npm",
  "README.md",
  "QUICKSTART.md",
  "scripts/npm",
]);

export const GENERATED_METADATA_CHECK_PATHS = Object.freeze([
  "Cargo.toml",
  "npm",
  "README.md",
  "QUICKSTART.md",
]);

export const DOC_METADATA_PATHS = Object.freeze(["README.md", "QUICKSTART.md"]);

export const DOC_VERSION_METADATA_LINE_SOURCES = Object.freeze([
  String.raw`\bThe current local version in this repo is\b`,
  String.raw`\bnpm install -g @christiandoxa/prodex@v?\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?\b`,
]);

export const VERSION_METADATA_PATTERNS = Object.freeze([
  "Cargo.toml",
  "Cargo.lock",
  "crates/*/Cargo.toml",
  "npm/prodex/package.json",
  "npm/platforms/*/package.json",
  "package-lock.json",
  "npm/package-lock.json",
  "npm/prodex/package-lock.json",
  "npm/platforms/*/package-lock.json",
  "CHANGELOG.md",
]);

export const RELEASE_METADATA_PATTERNS = Object.freeze([
  "Cargo.toml",
  "Cargo.lock",
  "CHANGELOG.md",
  "crates/*/Cargo.toml",
  "package-lock.json",
  "npm/package-lock.json",
  "npm/prodex/package.json",
  "npm/prodex/package-lock.json",
  "npm/platforms/*/package.json",
  "npm/platforms/*/package-lock.json",
]);

export const RELEASE_METADATA_CHURN_PATTERNS = Object.freeze([
  ...RELEASE_METADATA_PATTERNS,
  ...DOC_METADATA_PATHS,
]);

export const RELEASE_COMMIT_PATHS = Object.freeze([
  "Cargo.toml",
  "Cargo.lock",
  "CHANGELOG.md",
  "README.md",
  "QUICKSTART.md",
  "package.json",
  "package-lock.json",
  "npm/prodex/package.json",
  "npm/platforms/linux-x64/package.json",
  "npm/platforms/linux-arm64/package.json",
  "npm/platforms/darwin-x64/package.json",
  "npm/platforms/darwin-arm64/package.json",
  "npm/platforms/win32-x64/package.json",
  "npm/platforms/win32-arm64/package.json",
]);

export const KNOWN_NPM_LOCKFILE_PATHS = Object.freeze([
  "package-lock.json",
  "npm/package-lock.json",
  "npm/prodex/package-lock.json",
  "npm/platforms/linux-x64/package-lock.json",
  "npm/platforms/linux-arm64/package-lock.json",
  "npm/platforms/darwin-x64/package-lock.json",
  "npm/platforms/darwin-arm64/package-lock.json",
  "npm/platforms/win32-x64/package-lock.json",
  "npm/platforms/win32-arm64/package-lock.json",
]);

export const RELEASE_MESSAGE_PATTERN_SOURCES = Object.freeze([
  String.raw`^release(?:\([^)]*\))?!?:`,
  String.raw`^release\s+v?\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$`,
  String.raw`^chore(?:\([^)]*\))?!?:\s*release\b`,
  String.raw`^chore\(release\)!?:`,
  String.raw`^bump(?:\([^)]*\))?!?:\s*(?:prodex\s+)?v?\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$`,
]);

export const RELEASE_SUBJECT_PATTERN_SPECS = Object.freeze([
  {
    action: "release",
    source: String.raw`^chore\(release\)!?:\s*release\s+${SEMVER_SOURCE}\s*$`,
  },
  {
    action: "prepare",
    source: String.raw`^chore\(release\)!?:\s*prepare\s+${SEMVER_SOURCE}\s*$`,
  },
  {
    action: "release",
    source: String.raw`^release(?:\([^)]*\))?!?:\s*${SEMVER_SOURCE}\s*$`,
  },
  {
    action: "release",
    source: String.raw`^release\s+${SEMVER_SOURCE}\s*$`,
  },
  {
    action: "bump",
    source: String.raw`^bump(?:\([^)]*\))?!?:\s*${SEMVER_SOURCE}\s*$`,
  },
]);

export const CI_IMPACT_PATHS = Object.freeze({
  light: {
    exact: [
      "README.md",
      "QUICKSTART.md",
      "package.json",
      "package-lock.json",
      "scripts/ci/ci-impact.mjs",
      "scripts/ci/ci-impact.test.mjs",
      "scripts/ci/release-duplicate-version-guard.mjs",
      "scripts/ci/release-empty-commit-guard.mjs",
      "scripts/ci/generated-metadata-clean.mjs",
      "scripts/ci/release-metadata-only-guard.mjs",
      "scripts/ci/release-tag-changelog-guard.mjs",
      "scripts/ci/size-guard.test.mjs",
      TEST_IMPACT_MANIFEST_PATH,
      "scripts/ci/version-metadata-release-guard.mjs",
    ],
    prefixes: ["docs/", "npm/", "scripts/npm/"],
  },
  heavy: {
    exact: [
      "Cargo.lock",
      "Cargo.toml",
      "scripts/ci/runtime-env-parallel.mjs",
      "scripts/ci/runtime-hotpath-guard.mjs",
      "scripts/ci/runtime-proxy-bench-thresholds.json",
      "scripts/ci/runtime-proxy-ci-matrix.mjs",
      "scripts/ci/runtime-proxy-shard.mjs",
      "scripts/ci/runtime-stress.mjs",
      "scripts/ci/runtime-test-manifest-guard.mjs",
      "scripts/ci/runtime-test-manifest.mjs",
    ],
    prefixes: [
      ".cargo/",
      ".github/workflows/",
      "benches/",
      "crates/",
      "src/",
      "tests/",
      "rust-toolchain",
    ],
  },
});

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
  "ci:impact": "node --test scripts/ci/ci-impact.test.mjs",
  "ci:allow-guard": "node scripts/ci/allow-attribute-guard.mjs",
  "ci:env-mutation-guard": "node scripts/ci/env-mutation-guard.mjs",
  "ci:size-guard": "node scripts/ci/size-guard.mjs",
  "ci:size-guard-fixtures": "node --test scripts/ci/size-guard.test.mjs",
  "ci:churn-hygiene-fixtures": "node scripts/ci/churn-hygiene-fixture-tests.mjs",
  "ci:release-hygiene": "node scripts/ci/release-hygiene.mjs",
  "ci:generated-metadata-clean": "node scripts/ci/generated-metadata-clean.mjs",
  "ci:release-cut-fixtures": "node scripts/ci/release-cut-fixture-tests.mjs",
  "release:cut": "node scripts/npm/release-cut.mjs",
  "release:run": "node scripts/npm/release-run.mjs",
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
      "crates/prodex-app/src/runtime_launch/proxy_startup/",
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
      "scripts/ci/env-mutation-guard.mjs",
      "scripts/ci/guard-common.mjs",
      "scripts/ci/size-guard.mjs",
      "scripts/ci/size-guard.test.mjs",
    ],
    suffixes: [".rs"],
  },
  releaseGuardRelevant: {
    exact: [
      "package.json",
      ".github/workflows/ci.yml",
      "scripts/ci/changelog-noise-guard.mjs",
      "scripts/ci/generated-metadata-clean.mjs",
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
  releaseRunRelevant: {
    exact: ["scripts/npm/release-run.mjs", "scripts/npm/release-run-lib.mjs", RELEASE_RUN_TEST_PATH],
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
