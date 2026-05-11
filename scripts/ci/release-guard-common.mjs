import fs from "node:fs/promises";
import path from "node:path";
import { fileMatchesAnyPattern, git, normalizeGitPath } from "./guard-common.mjs";
import { repoRoot } from "../npm/common.mjs";

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

export const DOC_METADATA_PATHS = Object.freeze(["README.md", "QUICKSTART.md"]);

export const DOC_VERSION_METADATA_LINE_PATTERNS = Object.freeze([
  /\bThe current local version in this repo is\b/,
  /\bnpm install -g @christiandoxa\/prodex@v?\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?\b/,
]);

export const RELEASE_MESSAGE_PATTERNS = Object.freeze([
  /^release(?:\([^)]*\))?!?:/i,
  /^release\s+v?\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/i,
  /^chore(?:\([^)]*\))?!?:\s*release\b/i,
  /^chore\(release\)!?:/i,
  /^bump(?:\([^)]*\))?!?:\s*(?:prodex\s+)?v?\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/i,
]);

export const SEMVER_SOURCE = String.raw`v?([0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?)`;

export const RELEASE_SUBJECT_PATTERNS = Object.freeze([
  {
    action: "release",
    pattern: new RegExp(String.raw`^chore\(release\)!?:\s*release\s+${SEMVER_SOURCE}\s*$`, "i"),
  },
  {
    action: "prepare",
    pattern: new RegExp(String.raw`^chore\(release\)!?:\s*prepare\s+${SEMVER_SOURCE}\s*$`, "i"),
  },
  {
    action: "release",
    pattern: new RegExp(String.raw`^release(?:\([^)]*\))?!?:\s*${SEMVER_SOURCE}\s*$`, "i"),
  },
  {
    action: "release",
    pattern: new RegExp(String.raw`^release\s+${SEMVER_SOURCE}\s*$`, "i"),
  },
  {
    action: "bump",
    pattern: new RegExp(String.raw`^bump(?:\([^)]*\))?!?:\s*${SEMVER_SOURCE}\s*$`, "i"),
  },
]);

export const VERSION_TAG_PATTERN = /^v?(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?)$/;

export function requiredValue(value, name) {
  if (!value) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

export function messageSubject(message) {
  return message.split(/\r?\n/, 1)[0]?.trim() ?? "";
}

export function isReleaseLikeSubject(subject, options = {}) {
  if (options.assumeRelease) {
    return true;
  }
  return RELEASE_MESSAGE_PATTERNS.some((pattern) => pattern.test(subject.trim()));
}

export function isReleaseLikeMessage(message, options = {}) {
  return isReleaseLikeSubject(messageSubject(message), options);
}

export function releaseEntryFromSubject(subject) {
  const trimmed = subject.trim();
  for (const { action, pattern } of RELEASE_SUBJECT_PATTERNS) {
    const match = trimmed.match(pattern);
    if (match) {
      return { action, version: match[1] };
    }
  }
  return null;
}

export function releaseEntryFromMessage(message) {
  return releaseEntryFromSubject(messageSubject(message));
}

export function isVersionMetadataPath(filePath) {
  return fileMatchesAnyPattern(filePath, VERSION_METADATA_PATTERNS);
}

export function isReleaseMetadataPath(filePath) {
  return fileMatchesAnyPattern(filePath, RELEASE_METADATA_PATTERNS);
}

export function isDocMetadataPath(filePath) {
  return DOC_METADATA_PATHS.includes(normalizeGitPath(filePath));
}

export function isDocVersionMetadataLine(line) {
  return DOC_VERSION_METADATA_LINE_PATTERNS.some((pattern) => pattern.test(line));
}

export function isDocVersionMetadataChange(change, filePath) {
  const normalized = normalizeGitPath(filePath);
  const changedLines = change.changedLinesByFile?.get?.(normalized) ?? [];
  return changedLines.some((line) => isDocVersionMetadataLine(line.text));
}

function changedLinesForPath(change, filePath) {
  const normalized = normalizeGitPath(filePath);
  return change.changedLinesByFile?.get?.(normalized) ?? [];
}

function isCargoManifestPath(filePath) {
  const normalized = normalizeGitPath(filePath);
  return normalized === "Cargo.toml" || /^crates\/[^/]+\/Cargo\.toml$/.test(normalized);
}

function isCargoManifestVersionMetadataChange(change, filePath) {
  if (!isCargoManifestPath(filePath)) {
    return false;
  }
  return changedLinesForPath(change, filePath).some((line) =>
    /^[ \t]*version[ \t]*=[ \t]*"v?\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?"[ \t]*$/.test(
      line.text,
    ),
  );
}

function isCargoLockVersionMetadataChange(change, filePath) {
  if (normalizeGitPath(filePath) !== "Cargo.lock") {
    return false;
  }
  return changedLinesForPath(change, filePath).some((line) => {
    const packageName = line.hunkContext?.match(/^name = "(prodex(?:-[^"]+)?)"$/)?.[1];
    return (
      Boolean(packageName) &&
      /^[ \t]*version[ \t]*=[ \t]*"v?\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?"[ \t]*$/.test(
        line.text,
      )
    );
  });
}

function isNpmVersionMetadataChange(change, filePath) {
  const normalized = normalizeGitPath(filePath);
  if (
    ![
      "package-lock.json",
      "npm/package-lock.json",
      "npm/prodex/package.json",
      "npm/prodex/package-lock.json",
    ].includes(normalized) &&
    !/^npm\/platforms\/[^/]+\/package(?:-lock)?\.json$/.test(normalized)
  ) {
    return false;
  }
  return changedLinesForPath(change, filePath).some((line) =>
    /^[ \t]*"version"[ \t]*:[ \t]*"v?\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?"[, \t]*$/.test(
      line.text,
    ),
  );
}

export function isVersionMetadataChangePath(change, filePath) {
  const normalized = normalizeGitPath(filePath);
  return (
    normalized === "CHANGELOG.md" ||
    isCargoManifestVersionMetadataChange(change, filePath) ||
    isCargoLockVersionMetadataChange(change, filePath) ||
    isNpmVersionMetadataChange(change, filePath) ||
    (isDocMetadataPath(filePath) && isDocVersionMetadataChange(change, filePath))
  );
}

export function isReleaseMetadataChangePath(change, filePath) {
  return isReleaseMetadataPath(filePath) || (isDocMetadataPath(filePath) && isDocVersionMetadataChange(change, filePath));
}

export function normalizeVersionTag(tag) {
  const match = VERSION_TAG_PATTERN.exec(tag);
  return match?.[1] ?? null;
}

export function escapeRegExp(value) {
  return value.replace(/[\\^$.*+?()[\]{}|]/g, "\\$&");
}

export function hasChangelogHeading(changelog, version) {
  const pattern = new RegExp(`^##\\s+${escapeRegExp(version)}\\s+-\\s+`, "m");
  return pattern.test(changelog);
}

export async function versionTagsAtRev(rev) {
  const { stdout } = await git(["tag", "--points-at", rev], { cwd: repoRoot });
  return stdout
    .split(/\r?\n/)
    .filter(Boolean)
    .map((tag) => ({ tag, version: normalizeVersionTag(tag) }))
    .filter(({ version }) => version);
}

export function parseReleaseGuardArgs(argv, options = {}) {
  const args = {
    json: false,
    ...options.defaults,
  };
  const allow = {
    range: true,
    baseHead: true,
    commit: true,
    staged: false,
    worktree: false,
    includeUntracked: false,
    message: false,
    messageFile: false,
    json: true,
    help: true,
    ...options.allow,
  };

  for (let index = 2; index < argv.length; index += 1) {
    const value = argv[index];
    if (value === "--range" && allow.range) {
      index += 1;
      args.range = requiredValue(argv[index], value);
      continue;
    }
    if (value === "--base" && allow.baseHead) {
      index += 1;
      args.base = requiredValue(argv[index], value);
      continue;
    }
    if (value === "--head" && allow.baseHead) {
      index += 1;
      args.head = requiredValue(argv[index], value);
      continue;
    }
    if (value === "--commit" && allow.commit) {
      index += 1;
      args.commit = requiredValue(argv[index], value);
      continue;
    }
    if (value === "--staged" && allow.staged) {
      args.staged = true;
      continue;
    }
    if (value === "--worktree" && allow.worktree) {
      args.worktree = true;
      continue;
    }
    if (value === "--include-untracked" && allow.includeUntracked) {
      args.includeUntracked = true;
      continue;
    }
    if (value === "--message" && allow.message) {
      index += 1;
      args.message = requiredValue(argv[index], value);
      continue;
    }
    if (value === "--message-file" && allow.messageFile) {
      index += 1;
      args.messageFile = requiredValue(argv[index], value);
      continue;
    }
    if (value === "--json" && allow.json) {
      args.json = true;
      continue;
    }
    if ((value === "--help" || value === "-h") && allow.help) {
      args.help = true;
      continue;
    }
    throw new Error(`unknown argument: ${value}`);
  }

  return args;
}

export function selectorCount(args, options = {}) {
  const includeSynthetic = options.includeSynthetic ?? true;
  return [
    Boolean(args.range),
    Boolean(args.base || args.head),
    Boolean(args.commit),
    includeSynthetic && Boolean(args.staged),
    includeSynthetic && Boolean(args.worktree),
  ].filter(Boolean).length;
}

export function assertBaseHeadPair(args) {
  if ((args.base || args.head) && !(args.base && args.head)) {
    throw new Error("--base and --head must be used together");
  }
}

export function assertIncludeUntrackedWithWorktree(args) {
  if (args.includeUntracked && !args.worktree) {
    throw new Error("--include-untracked requires --worktree");
  }
}

export function assertSingleCommitSelector(args) {
  const selectors = selectorCount(args, { includeSynthetic: false });
  if (selectors > 1) {
    throw new Error("choose only one selector: --range, --base/--head, or --commit");
  }
  assertBaseHeadPair(args);
}

export function assertSingleChangeSelector(args) {
  const selectors = selectorCount(args);
  if (selectors > 1) {
    throw new Error("choose only one selector: --range, --base/--head, --commit, --staged, or --worktree");
  }
  assertBaseHeadPair(args);
  assertIncludeUntrackedWithWorktree(args);
}

export function selectorRange(args) {
  return args.range ?? `${args.base}..${args.head}`;
}

export async function commitMessage(rev) {
  const { stdout } = await git(["log", "-1", "--format=%B", rev], { cwd: repoRoot });
  return stdout.trimEnd();
}

export async function commitFiles(rev) {
  const { stdout } = await git(["diff-tree", "--root", "--no-commit-id", "--name-only", "-r", rev], {
    cwd: repoRoot,
  });
  return stdout
    .split(/\r?\n/)
    .filter(Boolean)
    .map(normalizeGitPath);
}

export function parseChangedLinesByFile(diff) {
  const changedLinesByFile = new Map();
  let currentFile = null;
  let oldFile = null;
  let currentHunkContext = "";

  for (const line of diff.split(/\r?\n/)) {
    if (line.startsWith("--- ")) {
      const rawPath = line.slice(4).trim();
      oldFile = rawPath === "/dev/null" ? null : normalizeGitPath(rawPath.replace(/^a\//, ""));
      continue;
    }

    if (line.startsWith("+++ ")) {
      const rawPath = line.slice(4).trim();
      currentFile = rawPath === "/dev/null" ? oldFile : normalizeGitPath(rawPath.replace(/^b\//, ""));
      if (currentFile && !changedLinesByFile.has(currentFile)) {
        changedLinesByFile.set(currentFile, []);
      }
      continue;
    }

    if (line.startsWith("@@ ")) {
      currentHunkContext = line.replace(/^@@[^@]*@@[ \t]?/, "").trim();
      continue;
    }

    if (!currentFile || line.startsWith("+++") || line.startsWith("---")) {
      continue;
    }

    const marker = line[0];
    if (marker !== "+" && marker !== "-") {
      continue;
    }

    changedLinesByFile.get(currentFile).push({
      type: marker === "+" ? "add" : "delete",
      text: line.slice(1),
      hunkContext: currentHunkContext,
    });
  }

  return changedLinesByFile;
}

export async function commitChangedLines(rev, options = {}) {
  const { stdout } = await git(
    [
      "show",
      "--format=",
      "--no-ext-diff",
      "--no-color",
      "--unified=0",
      ...diffFilterArgs(options.diffFilter),
      rev,
    ],
    { cwd: repoRoot },
  );
  return parseChangedLinesByFile(stdout);
}

export async function rangeCommits(range) {
  const { stdout } = await git(["rev-list", "--reverse", range], { cwd: repoRoot });
  return stdout.split(/\r?\n/).filter(Boolean);
}

export async function commitSummary(rev) {
  const { stdout } = await git(["log", "-1", "--format=%H%x00%h%x00%s", rev], { cwd: repoRoot });
  const [hash, shortHash, subject] = stdout.trimEnd().split("\0");
  if (!hash || !shortHash) {
    throw new Error(`failed to read commit subject for ${rev}`);
  }
  return {
    rev,
    hash,
    shortHash,
    subject: subject ?? "",
  };
}

export async function messageOverride(args) {
  if (args.messageFile) {
    return fs.readFile(path.resolve(repoRoot, args.messageFile), "utf8");
  }
  return args.message;
}

export async function syntheticMessage(args) {
  return (await messageOverride(args)) ?? "";
}

export async function changedFilesForSynthetic(args, options = {}) {
  const diffArgs = args.staged
    ? ["diff", "--cached", "--name-only", ...diffFilterArgs(options.diffFilter)]
    : ["diff", "--name-only", ...diffFilterArgs(options.diffFilter)];
  const { stdout } = await git(diffArgs, { cwd: repoRoot });
  const files = stdout
    .split(/\r?\n/)
    .filter(Boolean)
    .map(normalizeGitPath);

  if (args.includeUntracked) {
    const untracked = await git(["ls-files", "--others", "--exclude-standard"], { cwd: repoRoot });
    files.push(
      ...untracked.stdout
        .split(/\r?\n/)
        .filter(Boolean)
        .map(normalizeGitPath),
    );
  }

  return [...new Set(files)].sort();
}

export async function changedLinesForSynthetic(args, options = {}) {
  const diffArgs = args.staged
    ? ["diff", "--cached", "--no-ext-diff", "--no-color", "--unified=0", ...diffFilterArgs(options.diffFilter)]
    : ["diff", "--no-ext-diff", "--no-color", "--unified=0", ...diffFilterArgs(options.diffFilter)];
  const { stdout } = await git(diffArgs, { cwd: repoRoot });
  const changedLinesByFile = parseChangedLinesByFile(stdout);

  if (args.includeUntracked) {
    const untracked = await git(["ls-files", "--others", "--exclude-standard"], { cwd: repoRoot });
    for (const filePath of untracked.stdout.split(/\r?\n/).filter(Boolean).map(normalizeGitPath)) {
      try {
        const contents = await fs.readFile(path.resolve(repoRoot, filePath), "utf8");
        changedLinesByFile.set(
          filePath,
          contents.split(/\r?\n/).map((line) => ({ type: "add", text: line })),
        );
      } catch {
        // Ignore unreadable untracked files; path-level classification still applies.
      }
    }
  }

  return changedLinesByFile;
}

export function diffFilterArgs(diffFilter) {
  return diffFilter ? [`--diff-filter=${diffFilter}`] : [];
}

export async function selectedCommitRevs(args) {
  assertSingleCommitSelector(args);
  if (args.range || (args.base && args.head)) {
    const range = selectorRange(args);
    return {
      selector: range,
      revs: await rangeCommits(range),
    };
  }

  const rev = args.commit ?? "HEAD";
  return {
    selector: rev,
    revs: [rev],
  };
}

export async function selectedCommitSummaries(args) {
  const { selector, revs } = await selectedCommitRevs(args);
  const commits = [];
  for (const rev of revs) {
    commits.push(await commitSummary(rev));
  }
  return { selector, commits };
}

export async function selectedChanges(args, options = {}) {
  assertSingleChangeSelector(args);
  const override = await messageOverride(args);

  if (args.range || (args.base && args.head)) {
    const range = selectorRange(args);
    const commits = await rangeCommits(range);
    const changes = [];
    for (const rev of commits) {
      changes.push({
        label: rev,
        message: override ?? (await commitMessage(rev)),
        files: await commitFiles(rev),
        changedLinesByFile: options.includeChangedLines ? await commitChangedLines(rev, options) : undefined,
      });
    }
    return { selector: range, changes };
  }

  if (args.staged || args.worktree) {
    const label = args.staged ? "staged" : "worktree";
    return {
      selector: label,
      changes: [
        {
          label,
          message: override ?? "",
          files: await changedFilesForSynthetic(args, options),
          changedLinesByFile: options.includeChangedLines ? await changedLinesForSynthetic(args, options) : undefined,
        },
      ],
    };
  }

  const rev = args.commit ?? "HEAD";
  return {
    selector: rev,
    changes: [
      {
        label: rev,
        message: override ?? (await commitMessage(rev)),
        files: await commitFiles(rev),
        changedLinesByFile: options.includeChangedLines ? await commitChangedLines(rev, options) : undefined,
      },
    ],
  };
}
