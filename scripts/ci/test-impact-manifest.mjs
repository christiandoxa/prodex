import fs from "node:fs";

export const TEST_IMPACT_MANIFEST_PATH = "scripts/ci/test-impact-manifest.mjs";
export const TEST_IMPACT_MANIFEST_DATA_PATH = "scripts/ci/test-impact-manifest.json";
export const RELEASE_RUN_TEST_PATH = "scripts/npm/release-run.test.mjs";

export const SEMVER_SOURCE = String.raw`v?([0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?)`;

const manifest = JSON.parse(
  fs.readFileSync(new URL("./test-impact-manifest.json", import.meta.url), "utf8"),
);

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

function deepFreeze(value) {
  if (Array.isArray(value)) {
    for (const item of value) {
      deepFreeze(item);
    }
  } else if (value && typeof value === "object") {
    for (const item of Object.values(value)) {
      deepFreeze(item);
    }
  }
  return Object.freeze(value);
}

function manifestValue(name) {
  return deepFreeze(clone(manifest[name]));
}

function expandSemverPattern(source) {
  return source.replaceAll("${SEMVER_SOURCE}", SEMVER_SOURCE);
}

export const VERSION_SYNC_PATHS = manifestValue("versionSyncPaths");
export const GENERATED_METADATA_CHECK_PATHS = manifestValue("generatedMetadataCheckPaths");
export const DOC_METADATA_PATHS = manifestValue("docMetadataPaths");
export const DOC_VERSION_METADATA_LINE_SOURCES = manifestValue("docVersionMetadataLineSources");
export const VERSION_METADATA_PATTERNS = manifestValue("versionMetadataPatterns");
export const RELEASE_METADATA_PATTERNS = manifestValue("releaseMetadataPatterns");
export const RELEASE_METADATA_CHURN_PATTERNS = deepFreeze([
  ...RELEASE_METADATA_PATTERNS,
  ...DOC_METADATA_PATHS,
]);
export const RELEASE_COMMIT_PATHS = manifestValue("releaseCommitPaths");
export const KNOWN_NPM_LOCKFILE_PATHS = manifestValue("knownNpmLockfilePaths");
export const RELEASE_MESSAGE_PATTERN_SOURCES = manifestValue("releaseMessagePatternSources");
export const RELEASE_SUBJECT_PATTERN_SPECS = deepFreeze(
  manifest.releaseSubjectPatternSpecs.map((spec) => ({
    action: spec.action,
    source: expandSemverPattern(spec.source),
  })),
);
export const CI_IMPACT_PATHS = manifestValue("ciImpactPaths");
export const WATCH_UPSTREAM_FIXTURE_TESTS_PATH =
  "scripts/compat/watch-upstream-fixture-tests.mjs";
export const UPSTREAM_COMPAT_SCRIPT_PATHS = manifestValue("upstreamCompatScriptPaths");
export const PACKAGE_SCRIPT_ALIASES = manifestValue("packageScriptAliases");
export const PATH_GROUPS = manifestValue("pathGroups");

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
