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


def valid_ci_run(details, expected_repo, expected_sha):
    if not isinstance(details, dict):
        return False
    return (
        details.get("head_sha") == expected_sha
        and details.get("event") == "push"
        and details.get("head_branch") == "main"
        and details.get("path") == ".github/workflows/ci.yml"
        and details.get("repository", {}).get("full_name") == expected_repo
        and (
            details.get("status") == "in_progress"
            or (
                details.get("status") == "completed"
                and details.get("conclusion") == "success"
            )
        )
    )


def candidates(runs, expected_sha):
    ranked = []
    for run in runs:
        if run.get("headSha") != expected_sha:
            continue
        run_id = run.get("databaseId")
        if not isinstance(run_id, int) or run_id <= 0:
            continue
        if run.get("status") == "completed" and run.get("conclusion") == "success":
            rank = 0
        elif run.get("status") == "in_progress":
            rank = 1
        else:
            continue
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
        "run", "list", "--workflow", "ci.yml", "--event", "push",
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


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--target", required=True)
    parser.add_argument("--sha", required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
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
    with open(output_file, "a", encoding="utf8") as output:
        output.write(f"reused={'true' if reused else 'false'}\n")
    if not reused:
        print(f"No verified reusable CI archive for {args.target}; compiling locally")


if __name__ == "__main__":
    main()
