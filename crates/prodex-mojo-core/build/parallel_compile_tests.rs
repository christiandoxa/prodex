use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn compile_worker_limit_is_bounded_and_rejects_unsafe_overrides() {
    assert_eq!(configured_mojo_jobs(120, Some("12"), None, 16), Ok(4));
    assert_eq!(configured_mojo_jobs(120, Some("2"), None, 16), Ok(2));
    assert_eq!(configured_mojo_jobs(120, Some("bogus"), None, 3), Ok(3));
    assert_eq!(configured_mojo_jobs(120, None, None, 1), Ok(1));
    assert_eq!(configured_mojo_jobs(2, Some("8"), None, 8), Ok(2));
    assert_eq!(configured_mojo_jobs(5, Some("1"), Some("3"), 1), Ok(3));
    assert_eq!(configured_mojo_jobs(120, Some("1"), Some("8"), 1), Ok(8));
    assert_eq!(configured_mojo_jobs(0, Some("8"), None, 8), Ok(1));
    for invalid in ["", "0", "-1", "9", "999", "garbage"] {
        assert!(configured_mojo_jobs(10, Some("4"), Some(invalid), 8).is_err());
    }
}

#[cfg(unix)]
struct CompileFixture(PathBuf);

#[cfg(unix)]
impl CompileFixture {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = env::temp_dir().join(format!(
            "prodex-build-mojo-parallel-fixture-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn stub(&self) -> OsString {
        use std::os::unix::fs::PermissionsExt;
        let script = self.0.join("stub-mojo");
        use std::io::Write;
        let mut handle = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&script)
            .unwrap();
        handle
            .write_all(
                r#"#!/bin/sh
[ "$1" = build ] || exit 10
source="$2"
output=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "-o" ]; then
    shift
    output="$1"
  fi
  shift
done
[ -n "$output" ] || exit 11
sleep 0.1
case "$source" in
  *failure*) echo "deliberate compile failure" >&2; exit 42 ;;
esac
printf '%s\n' "$source" > "$output"
"#
                .as_bytes(),
            )
            .unwrap();
        handle.sync_all().unwrap();
        drop(handle);
        let mut permissions = fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions).unwrap();
        script.into_os_string()
    }

    fn work(&self, names: &[&str]) -> Vec<(PathBuf, PathBuf)> {
        names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let source = self.0.join(name);
                fs::write(&source, format!("fake mojo source {index}")).unwrap();
                (source, self.0.join(format!("compiled-{index}.o")))
            })
            .collect()
    }
}

#[cfg(unix)]
impl Drop for CompileFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(unix)]
#[test]
fn concurrent_compiler_preserves_indexed_objects_for_stable_archiving() {
    let fixture = CompileFixture::new();
    let work = fixture.work(&[
        "quota.mojo",
        "routing.mojo",
        "rich.mojo",
        "context.mojo",
        "gemini.mojo",
        "runtime.mojo",
    ]);
    compile_mojo_objects(
        &fixture.stub(),
        "x86_64-unknown-linux-gnu",
        "x86-64",
        &work,
        3,
    );
    for (source, object) in &work {
        assert_eq!(
            fs::read_to_string(object).unwrap().trim(),
            source.display().to_string()
        );
    }
}

#[cfg(unix)]
#[test]
fn failed_compiler_cannot_produce_a_successful_archive() {
    let fixture = CompileFixture::new();
    let work = fixture.work(&[
        "quota.mojo",
        "deliberate-failure.mojo",
        "context.mojo",
        "routing.mojo",
    ]);
    let result = std::panic::catch_unwind(|| {
        compile_mojo_objects(
            &fixture.stub(),
            "x86_64-unknown-linux-gnu",
            "x86-64",
            &work,
            3,
        );
    });
    assert!(
        result.is_err(),
        "failed Mojo compiler must stop the archive build"
    );
    assert!(
        !fixture.0.join("libprodex_mojo_core.a").exists(),
        "failed compile must never publish a partial archive"
    );
}
