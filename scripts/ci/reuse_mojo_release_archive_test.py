"""Historical release artifacts are reusable only across an identical archive boundary."""
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import stat
import subprocess
import tempfile
import unittest
from unittest import mock
import zipfile

SPEC = importlib.util.spec_from_file_location("historical_mojo", Path(__file__).with_name("reuse_mojo_ci_archive.py"))
M = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(M)
SHA, OLD, REPO, RUN, TARGET = "a"*40, "b"*40, "owner/repo", 51, "aarch64-unknown-linux-gnu"
PAYLOAD = b"!<arch>\n" + b"fixture"*256
FILENAME = "libprodex_mojo_core.a"

def zip_bytes(name=FILENAME, payload=PAYLOAD, link=False):
    data = io.BytesIO()
    with zipfile.ZipFile(data, "w") as z:
        info = zipfile.ZipInfo(name)
        if link:
            info.create_system = 3
            info.external_attr = (stat.S_IFLNK | 0o777) << 16
        z.writestr(info, payload)
    return data.getvalue()

def detail(**extra):
    return dict(id=RUN, head_sha=OLD, event="workflow_dispatch", head_branch="main",
                path=".github/workflows/standalone-release.yml", repository={"full_name":REPO},
                head_repository={"full_name":REPO}, status="completed", conclusion="success", **extra)

def metadata(data):
    return dict(id=123, name="mojo-archive-"+TARGET, expired=False, size_in_bytes=len(data),
                digest="sha256:"+hashlib.sha256(data).hexdigest(), workflow_run={"id":RUN,"head_sha":OLD})

def workflows():
    target = "name: target\non:\n  workflow_call:\nenv:\n  MOJO_VERSION: '1.2.0'\njobs:\n  build-mojo-archives:\n    runs-on: ubuntu-24.04\n    steps:\n"
    target += "      - name: Build target Mojo archive\n        run: pinned-compiler\n"
    for name in sorted(M.ARCHIVE_TRANSPORT_STEPS):
        target += "      - name: " + name + "\n        run: transport\n"
    target += "      - name: Verify target archive\n        run: check-abi\n  build:\n    needs: build-mojo-archives\n"
    parent = "jobs:\n  build:\n    matrix: six-exact-targets\n  prepare-release:\n    needs: build\n"
    return target, parent

class HistoricalReuse(unittest.TestCase):
    def test_only_successful_same_repo_main_release_workflow_is_trusted(self):
        self.assertTrue(M.valid_release_run(detail(), REPO, RUN))
        for key,value in [("id",RUN+1),("head_sha","bad"),("event","pull_request"),("head_branch","feature"),
                          ("path",".github/workflows/untrusted.yml"),("repository",{"full_name":"fork/repo"}),
                          ("head_repository",{"full_name":"fork/repo"}),("status","in_progress"),
                          ("conclusion","failure"),("conclusion","cancelled")]:
            d=detail();d[key]=value
            self.assertFalse(M.valid_release_run(d, REPO, RUN), key)

    def test_recipe_ignores_transport_but_preserves_build_env_checks_and_matrix(self):
        target,parent=workflows(); expected=M.archive_recipe(target,parent)
        self.assertEqual(M.archive_recipe(target.replace("run: transport","run: cached-transfer"),parent),expected)
        for modified in [target.replace("1.2.0","1.3.0"),target.replace("pinned-compiler","different-flags"),
                         target.replace("check-abi","missing-check"),target.replace("ubuntu-24.04","other-host"),
                         target.replace("    steps:","    env:\n      UNSAFE_OVERRIDE: 1\n    steps:")]:
            self.assertNotEqual(M.archive_recipe(modified,parent),expected)
        self.assertNotEqual(M.archive_recipe(target,parent.replace("six-exact-targets","changed-target")),expected)
        self.assertNotEqual(M.archive_recipe(target.replace("    steps:","    steps:\n      - name: Unexpected transformation\n        run: mutate"),parent),expected)
        with self.assertRaises(ValueError):
            M.archive_recipe(target.replace("Upload target Mojo archive","Unknown upload"),parent)

    def test_git_object_identity_includes_input_additions_deletions_and_modes_not_unrelated_docs(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d)
            def git(*args):
                return subprocess.run(["git",*args],cwd=root,text=True,capture_output=True,check=True).stdout.strip()
            git("init","-q");git("config","user.name","Fixture");git("config","user.email","fixture@example.invalid")
            files={"mojo/core.mojo":"source", "crates/prodex-mojo-core/build.rs":"build",
                   "Cargo.toml":"version", "Cargo.lock":"lock", "notes.md":"notes"}
            target,parent=workflows()
            files[".github/workflows/release-target.yml"]=target
            files[".github/workflows/standalone-release.yml"]=parent
            for name,text in files.items():
                p=root/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text)
            def commit():
                git("add",".");git("commit","-qm","fixture");return git("rev-parse","HEAD")
            first=commit();identity=M.archive_input_identity(first,root)
            (root/"notes.md").write_text("changed")
            self.assertEqual(M.archive_input_identity(commit(),root),identity)
            (root/"mojo/added.mojo").write_text("new input")
            self.assertNotEqual(M.archive_input_identity(commit(),root),identity)
            git("rm","mojo/added.mojo")
            self.assertEqual(M.archive_input_identity(commit(),root),identity)
            (root/"Cargo.lock").write_text("new dependency")
            self.assertNotEqual(M.archive_input_identity(commit(),root),identity)
            with self.assertRaises(ValueError):M.archive_input_identity("HEAD",root)

    def test_zip_digest_size_name_symlink_and_empty_data_fail_closed(self):
        with tempfile.TemporaryDirectory() as d:
            p=Path(d)/"artifact.zip";data=zip_bytes();p.write_bytes(data)
            self.assertEqual(M.checked_archive_bytes(p,metadata(data),FILENAME),PAYLOAD)
            for damaged in [{**metadata(data),"digest":"sha256:"+"0"*64},{**metadata(data),"size_in_bytes":1}]:
                with self.assertRaises(ValueError):M.checked_archive_bytes(p,damaged,FILENAME)
            for invalid in [zip_bytes("../"+FILENAME),zip_bytes(link=True),zip_bytes(payload=b"tiny")]:
                p.write_bytes(invalid)
                with self.assertRaises(ValueError):M.checked_archive_bytes(p,metadata(invalid),FILENAME)

    def exercise(self, output, *, details=None, identity=None, artifact=None, download_status=0):
        data=zip_bytes();meta=artifact if artifact is not None else metadata(data)
        listings=[dict(databaseId=RUN,headSha=OLD,status="completed",conclusion="success")]
        def gh(*args):
            if args[:2]==("run","list"):return listings
            if args[-1].endswith("/artifacts?per_page=100"):return {"artifacts":[meta]}
            return details if details is not None else detail()
        def download(args, **kwargs):
            kwargs["stdout"].write(data)
            return subprocess.CompletedProcess(args,download_status,b"",b"")
        with mock.patch.object(M,"gh_json",side_effect=gh), mock.patch.object(M,"git_text",return_value=""), \
             mock.patch.object(M,"archive_input_identity",side_effect=identity or ["same","same"]), \
             mock.patch.object(M.subprocess,"run",side_effect=download) as dl:
            result=M.try_reuse_release(TARGET,SHA,REPO,output)
        return result,dl.call_count

    def test_successful_input_equivalent_archive_is_verified_and_atomically_materialized(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d)
            self.assertEqual(self.exercise(root),(True,1))
            self.assertEqual((root/FILENAME).read_bytes(),PAYLOAD)
            self.assertEqual([p.name for p in root.iterdir()],[FILENAME])

    def test_changed_inputs_or_foreign_workflow_never_download(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d)
            self.assertEqual(self.exercise(root,identity=["new","old"]),(False,0))
            damaged=detail();damaged["event"]="pull_request"
            self.assertEqual(self.exercise(root,details=damaged),(False,0))
            self.assertEqual(list(root.iterdir()),[])

    def test_expired_missing_digest_ambiguous_binding_and_wrong_target_never_download(self):
        with tempfile.TemporaryDirectory() as d:
            for key,value in [("expired",True),("digest",None),("size_in_bytes",0),
                              ("workflow_run",{"id":RUN+1,"head_sha":OLD}),("name","other-target")]:
                damaged=metadata(zip_bytes());damaged[key]=value
                # Invalid/missing values must be rejected, not downloaded.
                result,calls=self.exercise(Path(d),artifact=damaged)
                self.assertEqual((result,calls),(False,0),key)

    def test_unknown_target_never_reads_history_or_downloads(self):
        with mock.patch.object(M, "gh_json") as remote:
            self.assertFalse(M.try_reuse_release("unsupported", SHA, REPO, Path("unused")))
            remote.assert_not_called()

    def test_network_failure_leaves_no_archive_and_falls_back(self):
        with tempfile.TemporaryDirectory() as d:
            self.assertEqual(self.exercise(Path(d),download_status=1),(False,1))
            self.assertEqual(list(Path(d).iterdir()),[])

    def test_corrupt_download_cannot_replace_existing_output(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d);(root/FILENAME).write_bytes(b"previous")
            damaged=metadata(zip_bytes());damaged["digest"]="sha256:"+"0"*64
            with self.assertRaises(ValueError):self.exercise(root,artifact=damaged)
            self.assertEqual((root/FILENAME).read_bytes(),b"previous")

if __name__ == "__main__":unittest.main()
