"""Hermetic trust, archive-reuse and strict fallback tests for release CI."""

import importlib.util
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import types
import unittest
from unittest import mock


SOURCE = Path(__file__).with_name("reuse_mojo_ci_archive.py")
SPEC = importlib.util.spec_from_file_location("release_mojo_reuse", SOURCE)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)

SHA = "a" * 40
REPO = "christiandoxa/prodex"
RUN_ID = 12345
TARGET = "x86_64-apple-darwin"
ARCHIVE_NAME = "libprodex_mojo_core.a"


def run_details(**extra):
    return {
        "head_sha": SHA,
        "event": "push",
        "head_branch": "main",
        "path": ".github/workflows/ci.yml",
        "repository": {"full_name": REPO},
        "status": "completed",
        "conclusion": "success",
        **extra,
    }


def listing(**extra):
    return [{
        "databaseId": RUN_ID, "headSha": SHA,
        "status": "completed", "conclusion": "success", **extra,
    }]


def archive_list(**extra):
    return {
        "artifacts": [{
            "name": "mojo-runtime-archive-" + TARGET,
            "size_in_bytes": 1300000, "expired": False, **extra,
        }]
    }


class MojoArchiveReuseTests(unittest.TestCase):
    def test_ci_provenance_is_strict_and_rejects_cross_branch_or_failed_run(self):
        self.assertTrue(MODULE.valid_ci_run(run_details(), REPO, SHA))
        self.assertTrue(MODULE.valid_ci_run(
            run_details(status="in_progress", conclusion=None), REPO, SHA,
        ))
        for damaged in [
            {"head_sha": "b" * 40},
            {"event": "workflow_dispatch"},
            {"head_branch": "release"},
            {"path": ".github/workflows/not-ci.yml"},
            {"repository": {"full_name": "attacker/repo"}},
            {"status": "completed", "conclusion": "failure"},
            {"status": "completed", "conclusion": "cancelled"},
            {"status": "queued", "conclusion": None},
        ]:
            with self.subTest(damaged=damaged):
                self.assertFalse(MODULE.valid_ci_run(
                    run_details(**damaged), REPO, SHA,
                ))

    def test_only_eligible_exact_sha_push_runs_are_considered(self):
        runs = listing() + [
            {"databaseId": 200, "headSha": SHA, "status": "in_progress", "conclusion": None},
            {"databaseId": 300, "headSha": "b" * 40, "status": "completed", "conclusion": "success"},
            {"databaseId": 400, "headSha": SHA, "status": "completed", "conclusion": "failure"},
            {"databaseId": 500, "headSha": SHA, "status": "queued", "conclusion": None},
            {"databaseId": "not-integer", "headSha": SHA, "status": "completed", "conclusion": "success"},
        ]
        self.assertEqual(MODULE.candidates(runs, SHA), [RUN_ID, 200])

    def test_expired_empty_or_wrong_named_artifacts_fail_closed(self):
        self.assertTrue(MODULE.available_artifact(
            archive_list()["artifacts"], "mojo-runtime-archive-" + TARGET,
        ))
        for entry in [
            archive_list(expired=True)["artifacts"],
            archive_list(size_in_bytes=0)["artifacts"],
            archive_list(name="a-different-archive")["artifacts"],
        ]:
            self.assertFalse(MODULE.available_artifact(
                entry, "mojo-runtime-archive-" + TARGET,
            ))

    def test_four_ci_targets_are_explicitly_bounded(self):
        self.assertEqual(len(MODULE.CI_ARCHIVE_TARGETS), 4)
        self.assertNotIn("aarch64-unknown-linux-gnu", MODULE.CI_ARCHIVE_TARGETS)
        self.assertNotIn("aarch64-pc-windows-msvc", MODULE.CI_ARCHIVE_TARGETS)

    def test_exact_source_archive_download_is_atomic_and_reuses_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "release-archive"
            calls = []

            def fake_gh_json(*args):
                calls.append(args)
                if args[:2] == ("run", "list"):
                    return listing()
                if args[-1].endswith(f"/runs/{RUN_ID}"):
                    return run_details()
                if args[-1].endswith(f"/runs/{RUN_ID}/artifacts?per_page=100"):
                    return archive_list()
                raise AssertionError(args)

            def fake_download(args, **_kwargs):
                self.assertEqual(args[:3], ["gh", "run", "download"])
                self.assertIn("--repo", args)
                root = Path(args[args.index("--dir") + 1])
                (root / ARCHIVE_NAME).write_bytes(b"MOJO-VERIFIED-CI\0" * 96)
                return subprocess.CompletedProcess(args, 0, "", "")

            with mock.patch.object(MODULE, "gh_json", side_effect=fake_gh_json):
                with mock.patch.object(MODULE.subprocess, "run", side_effect=fake_download):
                    reused = MODULE.try_reuse(TARGET, SHA, REPO, output)
            self.assertTrue(reused)
            self.assertGreater((output / ARCHIVE_NAME).stat().st_size, 1024)
            self.assertEqual(len(calls), 3)
            self.assertEqual(len(list(output.iterdir())), 1)

    def test_unavailable_or_untrusted_ci_archive_falls_back_without_output(self):
        with tempfile.TemporaryDirectory() as directory:
            for artifact in [
                archive_list(expired=True),
                archive_list(size_in_bytes=0),
                archive_list(name="other-name"),
            ]:
                with mock.patch.object(MODULE, "gh_json", side_effect=[
                    listing(), run_details(), artifact
                ]):
                    result = MODULE.try_reuse(
                        TARGET, SHA, REPO, Path(directory) / "absent"
                    )
                self.assertFalse(result)
            self.assertFalse((Path(directory) / "absent").exists())

    def test_successful_ci_archive_with_failed_download_compiles_fresh(self):
        with tempfile.TemporaryDirectory() as directory:
            def fail_download(args, **_kwargs):
                return subprocess.CompletedProcess(args, 1, "", "network failure")
            with mock.patch.object(MODULE, "gh_json", side_effect=[
                listing(), run_details(), archive_list()
            ]), mock.patch.object(MODULE.subprocess, "run", side_effect=fail_download):
                self.assertFalse(MODULE.try_reuse(
                    TARGET, SHA, REPO, Path(directory)
                ))
            self.assertEqual(list(Path(directory).iterdir()), [])

    def test_wrong_repo_or_ci_workflow_cannot_be_reused(self):
        with tempfile.TemporaryDirectory() as directory:
            for detail in [
                run_details(repository={"full_name": "different/repo"}),
                run_details(path=".github/workflows/release.yml"),
                run_details(head_sha="b" * 40),
                run_details(status="completed", conclusion="failure"),
            ]:
                with mock.patch.object(MODULE, "gh_json", side_effect=[
                    listing(), detail
                ]):
                    self.assertFalse(MODULE.try_reuse(
                        TARGET, SHA, REPO, Path(directory)
                    ))
            self.assertEqual(list(Path(directory).iterdir()), [])


if __name__ == "__main__":
    unittest.main()
