use super::{
    Path, PathBuf, ProdexProcessInfo, runtime_log_pid_from_path_with_prefix,
    select_active_runtime_log_paths_with_prefix,
};
use std::cell::Cell;

#[test]
fn active_log_pid_preserves_prefix_plus_overflow_and_tail_rules() {
    for (name, prefix, expected) in [
        ("prodex-runtime-200-old.log", "prodex-runtime", Some(200)),
        ("prodex-runtime-+000200-", "prodex-runtime", Some(200)),
        ("prodex-runtime-0-anything", "prodex-runtime", Some(0)),
        (
            "prodex-runtime-4294967295-東京",
            "prodex-runtime",
            Some(u32::MAX),
        ),
        ("prodex-runtime-4294967296-x", "prodex-runtime", None),
        ("prodex-runtime--1-x", "prodex-runtime", None),
        ("prodex-runtime-200", "prodex-runtime", None),
        ("prodex-runtime- 200-x", "prodex-runtime", None),
        ("prodex-runtime-２００-x", "prodex-runtime", None),
        ("prodex-runtime-200x-x", "prodex-runtime", None),
        ("PRODEX-runtime-200-x", "prodex-runtime", None),
        ("-+1-", "", Some(1)),
        ("雪--4-", "雪-", Some(4)),
    ] {
        assert_eq!(
            runtime_log_pid_from_path_with_prefix(Path::new(name), prefix),
            expected
        );
    }
    assert_eq!(
        runtime_log_pid_from_path_with_prefix(Path::new(""), ""),
        None
    );
}

#[test]
fn active_log_selection_keeps_last_encountered_path_in_pid_order() {
    let processes = [
        (200, true),
        (2, true),
        (201, false),
        (200, false),
        (0, true),
        (u32::MAX, true),
    ]
    .map(|(pid, runtime)| ProdexProcessInfo {
        pid,
        command: "run".to_string(),
        runtime,
    });
    let names = [
        "prodex-runtime-200-old.log",
        "prodex-runtime-201-ignored.log",
        "prodex-runtime-2-z.log",
        "prodex-runtime-+000200-new.log",
        "bad-2-ignored.log",
        "prodex-runtime-999-ignored.log",
        "prodex-runtime-2-a.log",
        "prodex-runtime-4294967295-max.log",
        "prodex-runtime-0-zero.log",
    ];
    assert_eq!(
        select_active_runtime_log_paths_with_prefix(
            &processes,
            names.map(PathBuf::from),
            "prodex-runtime"
        ),
        [names[8], names[6], names[3], names[7]]
            .map(PathBuf::from)
            .to_vec(),
    );
}

struct ObservedPaths<'a>(&'a Cell<bool>);
impl IntoIterator for ObservedPaths<'_> {
    type Item = PathBuf;
    type IntoIter = std::iter::Empty<PathBuf>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.set(true);
        std::iter::empty()
    }
}

#[test]
fn active_log_selection_preserves_lazy_empty_runtime_iteration() {
    let idle = [ProdexProcessInfo {
        pid: 4,
        command: "info".to_string(),
        runtime: false,
    }];
    for processes in [&[][..], &idle[..]] {
        let consumed = Cell::new(false);
        assert!(
            select_active_runtime_log_paths_with_prefix(
                processes,
                ObservedPaths(&consumed),
                "prodex-runtime"
            )
            .is_empty()
        );
        assert!(!consumed.get());
    }
    let runtime = [ProdexProcessInfo {
        pid: 4,
        command: "run".to_string(),
        runtime: true,
    }];
    let consumed = Cell::new(false);
    assert!(
        select_active_runtime_log_paths_with_prefix(
            &runtime,
            ObservedPaths(&consumed),
            "prodex-runtime"
        )
        .is_empty()
    );
    assert!(consumed.get());
}

#[cfg(unix)]
#[test]
fn active_log_selection_keeps_native_invalid_utf8_exclusion() {
    use std::os::unix::ffi::OsStringExt;
    let invalid = PathBuf::from(std::ffi::OsString::from_vec(
        b"prodex-runtime-\xff-x".to_vec(),
    ));
    assert_eq!(
        runtime_log_pid_from_path_with_prefix(&invalid, "prodex-runtime"),
        None
    );
    let processes = [ProdexProcessInfo {
        pid: 4,
        command: "run".to_string(),
        runtime: true,
    }];
    let valid = PathBuf::from("prodex-runtime-4-good.log");
    assert_eq!(
        select_active_runtime_log_paths_with_prefix(
            &processes,
            [invalid, valid.clone()],
            "prodex-runtime"
        ),
        vec![valid]
    );
}
