import { runChecked } from "../lib/checked-subprocess.mjs";

const VERSION_TAG = /^v?\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/u;

// Release history follows ancestry, not version magnitude: 0.165.0 deliberately
// succeeds 0.439.5. Retain numeric ordering only for tags on the same commit.
export function mergedVersionTagsInReleaseOrder(root, revision = "HEAD", historicalAnchors = []) {
  const git = (args) => runChecked("git", args, { cwd: root }).stdout.trim();
  const tags = git(["tag", "--merged", revision, "--sort=version:refname"])
    .split(/\r?\n/u).filter((tag) => VERSION_TAG.test(tag));
  if (tags.length === 0 && historicalAnchors.length === 0) return [];
  const objects = new Map();
  for (const line of (tags.length ? git(["show-ref", "--tags", "--dereference"]) : "").split(/\r?\n/u)) {
    const [sha, ref] = line.split(/\s+/u);
    objects.set(ref, sha);
  }
  const positions = new Map(git(["rev-list", "--topo-order", revision])
    .split(/\r?\n/u).map((sha, index) => [sha, index]));
  const records = tags.map((tag) => {
    const ref = `refs/tags/${tag}`;
    const commit = objects.get(`${ref}^{}`) ?? objects.get(ref);
    const position = positions.get(commit);
    if (position === undefined) throw new Error(`release tag ${tag} has no reachable commit`);
    return { tag, commit, position };
  });
  for (const { tag, commit } of historicalAnchors) {
    const position = positions.get(commit);
    if (position === undefined) continue;
    const existing = records.find((record) => record.tag === tag);
    if (existing && existing.commit !== commit) throw new Error(`release tag ${tag} does not match its historical anchor`);
    if (!existing) records.push({ tag, commit, position });
  }
  return records.sort((left, right) => right.position - left.position)
    .map(({ tag, commit }) => ({ tag, commit }));
}

const STABLE_VERSION = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/u;
const LEGACY_TIP = "ac9aad08f25d6dc8cda96463ce2045b40f35f510";

export function assertReleaseAdvance(version, previous, upstreamRelease) {
  if (!STABLE_VERSION.test(version)) throw new Error("release version must be an exact stable SemVer");
  if (!previous) return;
  const before = previous.tag.replace(/^v/u, "");
  if (!STABLE_VERSION.test(before)) throw new Error("previous release must have a stable version");
  // Only the explicitly authorized numbering transition may decrease SemVer.
  // Binding the old tag to its immutable commit rejects a moved/foreign anchor.
  if (version === "0.165.0" && previous.tag === "0.439.5"
      && previous.commit === LEGACY_TIP && upstreamRelease === "rust-v0.165.0") return;
  const candidate = version.split(".").map(BigInt);
  const current = before.split(".").map(BigInt);
  for (let index = 0; index < candidate.length; index += 1) {
    if (candidate[index] > current[index]) return;
    if (candidate[index] < current[index]) break;
  }
  throw new Error(`release version ${version} must be newer than ${before}`);
}
