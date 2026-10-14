use super::*;
use crate::ChildProcessPlan;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

const ID: &str = "01900000-0000-7000-8000-000000000712";
const OTHER: &str = "01900000-0000-7000-8000-000000000713";

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "prodex-exit-guidance-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
    fn plan(&self, args: &[&str]) -> RuntimeLaunchPlan {
        RuntimeLaunchPlan::new(
            ChildProcessPlan::new("codex".into(), self.0.clone())
                .with_args(args.iter().map(|arg| (*arg).into()).collect()),
        )
        .with_saved_session_marker(self.0.join("owned.id"))
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn saved_session_guidance_uses_this_launchs_root_marker_over_original_resume_arg() {
    let fixture = Fixture::new();
    let plan = fixture.plan(&["resume", OTHER]);
    fs::write(fixture.0.join("owned.id"), format!("{ID}\n")).unwrap();
    fs::write(fixture.0.join("unrelated-newest.id"), OTHER).unwrap();
    assert_eq!(saved_session_id(&plan), Some(Uuid::parse_str(ID).unwrap()));
}

#[test]
fn saved_session_guidance_explicit_id_and_unknown_session_do_not_use_global_latest() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join("unrelated-newest.id"), OTHER).unwrap();
    assert_eq!(
        saved_session_id(&fixture.plan(&["resume", ID])),
        Some(Uuid::parse_str(ID).unwrap())
    );
    fs::write(fixture.0.join("owned.id"), "not-a-uuid\x1b[2J").unwrap();
    assert_eq!(saved_session_id(&fixture.plan(&[])), None);
    assert_eq!(saved_session_id(&fixture.plan(&["resume"])), None);
    fs::write(fixture.0.join("owned.id"), "x".repeat(300)).unwrap();
    assert_eq!(saved_session_id(&fixture.plan(&[])), None);
}

#[test]
fn saved_session_guidance_appends_without_overwriting_native_footer_or_truncating_id() {
    let native = format!("Token usage: 1,000  output=200\r\nNative resume ID: {ID}");
    let mut output = native.as_bytes().to_vec();
    write_saved_session_guidance(&mut output, Some(Uuid::parse_str(ID).unwrap())).unwrap();
    assert_eq!(&output[..native.len()], native.as_bytes());
    let appended = std::str::from_utf8(&output[native.len()..]).unwrap();
    assert!(appended.starts_with("\r\nProdex saved session\r\n"));
    assert!(appended.contains(&format!("Session ID: {ID}\r\n")));
    assert!(appended.contains(&format!("Resume: prodex s {ID}\r\n")));
    assert!(!appended.contains('\x1b'));
    assert!(!appended.contains("unix://"));
}

#[test]
fn saved_session_guidance_cleanup_removes_only_owned_markers_after_reading_id() {
    let fixture = Fixture::new();
    let marker = fixture.0.join("owned.id");
    let offset = marker.with_extension("offset");
    let lock = crate::runtime_store::json_lock_file_path(&marker);
    let preserved = fixture.0.join("saved-session.jsonl");
    let overlay = fixture.0.join("overlay");
    fs::create_dir(&overlay).unwrap();
    fs::write(&marker, ID).unwrap();
    fs::write(&offset, "25").unwrap();
    fs::write(&lock, "").unwrap();
    fs::write(&preserved, "saved attachment and goal data").unwrap();
    let plan = fixture
        .plan(&[])
        .with_cleanup_path(overlay.clone())
        .with_cleanup_path(marker.clone())
        .with_cleanup_path(offset.clone())
        .with_cleanup_path(lock.clone());
    let id = saved_session_id(&plan);
    prodex_runtime_launch::cleanup_runtime_launch_plan(&plan);
    assert_eq!(id, Some(Uuid::parse_str(ID).unwrap()));
    for path in [marker, offset, lock, overlay] {
        assert!(!path.exists());
    }
    assert_eq!(
        fs::read_to_string(preserved).unwrap(),
        "saved attachment and goal data"
    );
}

#[test]
fn saved_session_guidance_preserves_broken_pipe_errors() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    assert_eq!(
        write_saved_session_guidance(&mut Broken, None)
            .unwrap_err()
            .kind(),
        io::ErrorKind::BrokenPipe
    );
}
