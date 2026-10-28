#!/usr/bin/env python3
"""Reuse a checked-in-SHA CI Mojo archive, or safely fall back to compilation.

This is speculative preparation only. The standalone release binary is still
gated on successful complete CI and the six-target archive verification.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import stat
import zipfile
import tempfile

CI_ARCHIVE_TARGETS = frozenset({
    "x86_64-unknown-linux-gnu",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "x86_64-pc-windows-msvc",
})


def gh_json(*args):
    process = subprocess.run(
        ["gh", *args],
        text=True,
        capture_output=True,
        check=True,
        timeout=45,
    )
    return json.loads(process.stdout)


def reusable_run_state(status, conclusion):
    # A matrix may return to queued while some producer jobs are already done.
    # Only a ready immutable artifact is reused; release still waits for full CI.
    return (
        status in {"queued", "pending", "in_progress"} and conclusion in (None, "")
    ) or (status == "completed" and conclusion == "success")


def valid_ci_run(details, expected_repo, expected_sha):
    if not isinstance(details, dict):
        return False
    return (
        details.get("head_sha") == expected_sha
        and details.get("event") == "push"
        and details.get("head_branch") == "main"
        and details.get("path") == ".github/workflows/ci.yml"
        and details.get("repository", {}).get("full_name") == expected_repo
        and reusable_run_state(details.get("status"), details.get("conclusion"))
    )


def candidates(runs, expected_sha):
    ranked = []
    for run in runs:
        if run.get("headSha") != expected_sha:
            continue
        run_id = run.get("databaseId")
        if not isinstance(run_id, int) or run_id <= 0:
            continue
        if not reusable_run_state(run.get("status"), run.get("conclusion")):
            continue
        rank = {"completed": 0, "in_progress": 1, "queued": 2, "pending": 2}[run["status"]]
        ranked.append((rank, -run_id, run_id))
    return [run_id for _, _, run_id in sorted(ranked)]


def available_artifact(artifacts, name):
    return any(
        artifact.get("name") == name
        and artifact.get("expired") is False
        and isinstance(artifact.get("size_in_bytes"), int)
        and artifact["size_in_bytes"] > 0
        for artifact in artifacts
    )


def try_reuse(target, target_sha, repository, output_dir):
    if target not in CI_ARCHIVE_TARGETS:
        return False
    run_listing = gh_json(
        "run", "list", "--repo", repository, "--workflow", "ci.yml", "--event", "push",
        "--limit", "50", "--json",
        "databaseId,headSha,status,conclusion",
    )
    if not isinstance(run_listing, list):
        return False
    artifact_name = "mojo-runtime-archive-" + target
    archive_filename = (
        "prodex_mojo_core.lib" if target.endswith("-msvc") else "libprodex_mojo_core.a"
    )

    for run_id in candidates(run_listing, target_sha):
        details = gh_json("api", f"repos/{repository}/actions/runs/{run_id}")
        if not valid_ci_run(details, repository, target_sha):
            continue
        listing = gh_json(
            "api", f"repos/{repository}/actions/runs/{run_id}/artifacts?per_page=100"
        )
        if not available_artifact(listing.get("artifacts", []), artifact_name):
            continue

        # Extract privately to a scratch directory, verify the expected name
        # and nonzero payload, then publish atomically into the archive path.
        output_dir.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="verified-ci-mojo-", dir=output_dir) as tmp:
            result = subprocess.run(
                [
                    "gh", "run", "download", str(run_id), "--repo", repository,
                    "--name", artifact_name, "--dir", tmp,
                ],
                text=True,
                capture_output=True,
                timeout=90,
            )
            if result.returncode != 0:
                print(f"CI archive download unavailable; building locally: {result.stderr[-200:]}", file=sys.stderr)
                continue
            archive = Path(tmp) / archive_filename
            if not archive.is_file() or archive.is_symlink() or archive.stat().st_size < 1024:
                print("CI archive lacks expected nonempty archive filename; building locally", file=sys.stderr)
                continue
            digest = hashlib.sha256(archive.read_bytes()).hexdigest()
            archive.replace(output_dir / archive_filename)
        print(
            f"Reused strict CI artifact {artifact_name} from run {run_id} "
            f"with source {target_sha} (sha256={digest})"
        )
        return True
    return False



# Artifact retention is independent of the small, evictable dependency cache.
# Reuse older main-run archives only when both inputs AND the producing recipe
# are identical. Release binaries are still rebuilt from the current source.
RELEASE_ARCHIVE_TARGETS = CI_ARCHIVE_TARGETS | {
    "aarch64-unknown-linux-gnu", "aarch64-pc-windows-msvc",
}
ARCHIVE_INPUT_PATHS = (
    "mojo", "crates/prodex-mojo-core", "Cargo.toml", "Cargo.lock", ".cargo",
    "rust-toolchain", "rust-toolchain.toml", "Cross.toml",
    "scripts/ci/ensure-llvm-archive-tools.sh",
)
ARCHIVE_TRANSPORT_STEPS = frozenset({
    "Restore exact-input archive shared with main CI",
    "Materialize verified-input cached archive for release validation",
    "Reuse exact-source strict CI Mojo archive when available",
    "Save only ABI-validated archive into the shared cache path",
    "Upload target Mojo archive",
})
MAX_ARCHIVE_BYTES = 64 * 1024 * 1024


def workflow_job(text, name):
    match = re.search(r"^  " + re.escape(name) + r":\n.*?(?=^  [\w-]+:|\Z)", text, re.M | re.S)
    if not match:
        raise ValueError("missing archive recipe job: " + name)
    return match.group(0)


def archive_recipe(target_workflow, parent_workflow):
    if "\njobs:\n" not in target_workflow:
        raise ValueError("missing called workflow header")
    job = workflow_job(target_workflow, "build-mojo-archives")
    blocks = re.split(r"(?=^      - name: )", job, flags=re.M)
    kept, removed = [], set()
    for block in blocks:
        match = re.match(r"      - name: (.+)\n", block)
        name = match.group(1) if match else None
        if name in ARCHIVE_TRANSPORT_STEPS:
            if name in removed:
                raise ValueError("ambiguous archive transport step")
            removed.add(name)
        else:
            kept.append(block.rstrip())
    if removed != ARCHIVE_TRANSPORT_STEPS:
        raise ValueError("unknown historical archive layout")
    return [target_workflow.split("\njobs:\n", 1)[0], "\n".join(kept),
            workflow_job(parent_workflow, "build").rstrip()]


def git_text(args, root):
    return subprocess.run(["git", *args], cwd=root, capture_output=True,
                          text=True, check=True, timeout=20).stdout


def archive_input_identity(revision, root=Path(".")):
    if not re.fullmatch(r"[a-f0-9]{40}", revision):
        raise ValueError("invalid archive source SHA")
    tree = git_text(["ls-tree", "-r", revision, "--", *ARCHIVE_INPUT_PATHS], root)
    if "\tmojo/" not in tree or "\tcrates/prodex-mojo-core/build.rs" not in tree:
        raise ValueError("incomplete archive source tree")
    target = git_text(["show", revision + ":.github/workflows/release-target.yml"], root)
    parent = git_text(["show", revision + ":.github/workflows/standalone-release.yml"], root)
    record = {"tree": tree, "recipe": archive_recipe(target, parent)}
    return hashlib.sha256(json.dumps(record, sort_keys=True).encode()).hexdigest()


def valid_release_run(details, repository, run_id):
    return isinstance(details, dict) and (
        details.get("id") == run_id
        and re.fullmatch(r"[a-f0-9]{40}", details.get("head_sha", "")) is not None
        and details.get("event") == "workflow_dispatch"
        and details.get("head_branch") == "main"
        and details.get("path") == ".github/workflows/standalone-release.yml"
        and details.get("repository", {}).get("full_name") == repository
        and details.get("head_repository", {}).get("full_name") == repository
        and details.get("status") == "completed"
        and details.get("conclusion") == "success"
    )


def checked_archive_bytes(zip_path, metadata, filename):
    if zip_path.stat().st_size != metadata.get("size_in_bytes"):
        raise ValueError("archive transport size mismatch")
    if not 0 < zip_path.stat().st_size <= MAX_ARCHIVE_BYTES:
        raise ValueError("archive transport is oversized")
    digest = "sha256:" + hashlib.sha256(zip_path.read_bytes()).hexdigest()
    if metadata.get("digest") != digest:
        raise ValueError("immutable artifact digest mismatch")
    with zipfile.ZipFile(zip_path) as bundle:
        members = bundle.infolist()
        if len(members) != 1 or members[0].filename != filename:
            raise ValueError("unexpected archive member or path")
        member = members[0]
        if stat.S_ISLNK(member.external_attr >> 16) or not 1024 <= member.file_size <= MAX_ARCHIVE_BYTES:
            raise ValueError("unsafe archive member")
        return bundle.read(member)


def try_reuse_release(target, target_sha, repository, output_dir, root=Path(".")):
    if target not in RELEASE_ARCHIVE_TARGETS:
        return False
    expected = archive_input_identity(target_sha, root)
    runs = gh_json("run", "list", "--repo", repository, "--workflow", "standalone-release.yml",
                   "--event", "workflow_dispatch", "--limit", "20", "--json",
                   "databaseId,headSha,status,conclusion")
    if not isinstance(runs, list):
        return False
    eligible = [run for run in runs if run.get("status") == "completed"
                and run.get("conclusion") == "success"
                and type(run.get("databaseId")) is int and run["databaseId"] > 0]
    filename = "prodex_mojo_core.lib" if target.endswith("-msvc") else "libprodex_mojo_core.a"
    for run in eligible[:3]:
        run_id = run["databaseId"]
        details = gh_json("api", f"repos/{repository}/actions/runs/{run_id}")
        if not valid_release_run(details, repository, run_id):
            continue
        source = details["head_sha"]
        try:
            git_text(["cat-file", "-e", source + "^{commit}"], root)
        except subprocess.CalledProcessError:
            # The run was authenticated as same-repository main. Fetch only its
            # immutable object; never check out or execute historical scripts.
            subprocess.run(["git", "fetch", "--no-tags", "--depth=1", "origin", source],
                           cwd=root, capture_output=True, check=True, timeout=45)
        try:
            if archive_input_identity(source, root) != expected:
                continue
        except (ValueError, subprocess.CalledProcessError):
            continue
        listing = gh_json("api", f"repos/{repository}/actions/runs/{run_id}/artifacts?per_page=100")
        matches = [a for a in listing.get("artifacts", [])
                   if a.get("name") == "mojo-archive-" + target and a.get("expired") is False
                   and type(a.get("id")) is int and a["id"] > 0
                   and type(a.get("size_in_bytes")) is int and 0 < a["size_in_bytes"] <= MAX_ARCHIVE_BYTES
                   and re.fullmatch(r"sha256:[a-f0-9]{64}", a.get("digest") or "")]
        if len(matches) != 1:
            continue
        artifact = matches[0]
        binding = artifact.get("workflow_run", {})
        if binding.get("id") != run_id or binding.get("head_sha") != source:
            continue
        output_dir.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="verified-release-mojo-", dir=output_dir) as temp:
            zip_path = Path(temp) / "artifact.zip"
            with zip_path.open("wb") as stream:
                result = subprocess.run(["gh", "api", f"repos/{repository}/actions/artifacts/{artifact['id']}/zip"],
                                        stdout=stream, stderr=subprocess.PIPE, timeout=90)
            if result.returncode != 0:
                continue
            payload = checked_archive_bytes(zip_path, artifact, filename)
            staged = Path(temp) / filename
            staged.write_bytes(payload)
            staged.replace(output_dir / filename)
        print(f"Reused input-equivalent release archive {target} from successful main run {run_id} "
              f"source={source} input_sha256={expected} archive_sha256={hashlib.sha256(payload).hexdigest()}")
        return True
    return False


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--target", required=True)
    parser.add_argument("--sha", required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--release-history", action="store_true",
                        help="allow independently verified, input-equivalent successful main artifacts")
    args = parser.parse_args()

    if not re.fullmatch(r"[a-f0-9]{40}", args.sha):
        parser.error("release source SHA must have 40 lowercase hexadecimal characters")
    if os.environ.get("GITHUB_SHA") != args.sha:
        parser.error("GitHub dispatch SHA must match the exact release source SHA")
    try:
        checkout_sha = subprocess.run(
            ["git", "rev-parse", "HEAD"], text=True, capture_output=True,
            check=True, timeout=10,
        ).stdout.strip()
    except (OSError, subprocess.SubprocessError) as error:
        parser.error(f"cannot validate source checkout: {error}")
    if checkout_sha != args.sha:
        parser.error("release source checkout differs from exact dispatch SHA")
    repository = os.environ.get("GITHUB_REPOSITORY", "")
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository):
        parser.error("GITHUB_REPOSITORY is required")
    output_file = os.environ.get("GITHUB_OUTPUT")
    if not output_file:
        parser.error("GITHUB_OUTPUT is required")

    try:
        reused = try_reuse(args.target, args.sha, repository, args.output_dir)
    except (OSError, subprocess.SubprocessError, ValueError, TypeError, KeyError) as error:
        # A lost/late CI artifact is not permission to skip strict compilation.
        # It causes a fresh, identical-source compile with all existing guards.
        print(f"CI archive not reusable ({error}); compiling release archive", file=sys.stderr)
        reused = False
    if not reused and args.release_history:
        try:
            reused = try_reuse_release(args.target, args.sha, repository, args.output_dir)
        except (OSError, subprocess.SubprocessError, ValueError, TypeError, KeyError, zipfile.BadZipFile) as error:
            print(f"Historical archive not reusable ({type(error).__name__}); compiling release archive", file=sys.stderr)
            reused = False
    with open(output_file, "a", encoding="utf8") as output:
        output.write(f"reused={'true' if reused else 'false'}\n")
    if not reused:
        print(f"No verified reusable CI archive for {args.target}; compiling locally")


if __name__ == "__main__":
    main()
