#![cfg(feature = "mojo-runtime")]
#![allow(unsafe_code)]

use prodex_mojo_core::launch::{
    LaunchArgument, LaunchArgumentOperation, inspect_launch_arguments, plan_launch_arguments,
};

#[repr(C)]
#[derive(Clone, Copy)]
struct View {
    address: u64,
    length: u64,
    valid_utf8: i64,
}

unsafe extern "C" {
    fn prodex_mojo_launch_args_v1(
        version: i64,
        operation: i64,
        full: i64,
        args: u64,
        count: i64,
        out: u64,
        capacity: i64,
        scratch: u64,
        meta: u64,
    ) -> i64;
}

#[test]
fn launch_boundary_accepts_empty_and_opaque_records() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) };
    let plan = plan_launch_arguments(&[], LaunchArgumentOperation::RetargetExec, false).unwrap();
    assert_eq!(
        plan.arguments,
        vec![
            LaunchArgument::Exec,
            LaunchArgument::Resume,
            LaunchArgument::Session
        ]
    );
    let opaque = [None, Some("--profile-v2=x")];
    let plan =
        plan_launch_arguments(&opaque, LaunchArgumentOperation::NormalizeRun, false).unwrap();
    assert_eq!(
        plan.arguments,
        vec![LaunchArgument::Original(0), LaunchArgument::Original(1)]
    );
    assert_eq!(
        inspect_launch_arguments(&opaque).unwrap().first_positional,
        Some(0)
    );
}

#[test]
fn launch_boundary_rejects_bad_abi_tags_utf8_and_capacities_without_writes() {
    let bytes = [0xff_u8];
    let input = [View {
        address: bytes.as_ptr() as u64,
        length: 1,
        valid_utf8: 1,
    }];
    let mut out = [0x5a_i64; 12];
    let mut scratch = [0x5a_i64; 12];
    let mut meta = [0x5a_i64; 11];
    for (version, op, full, address, count, capacity, expected) in [
        (2, 1, 0, 0, 0, 4, 4),
        (1, 99, 0, 0, 0, 4, 1),
        (1, 4, 0, 0, 0, 4, 1),
        (1, 5, 0, 0, 0, 4, 1),
        (1, 1, 2, 0, 0, 4, 1),
        (1, 1, 0, 0, -1, 4, 1),
        (1, 1, 0, 0, 1, 4, 1),
        (1, 1, 0, 0, 0, 2, 3),
        (1, 1, 0, input.as_ptr() as u64, 1, 4, 2),
    ] {
        let result = unsafe {
            prodex_mojo_launch_args_v1(
                version,
                op,
                full,
                address,
                count,
                out.as_mut_ptr() as u64,
                capacity,
                scratch.as_mut_ptr() as u64,
                meta.as_mut_ptr() as u64,
            )
        };
        assert_eq!(result, expected);
        assert_eq!(out, [0x5a; 12]);
        assert_eq!(scratch, [0x5a; 12]);
        assert_eq!(meta, [0x5a; 11]);
    }
    let mut invalid = View {
        address: 1,
        length: 0,
        valid_utf8: 0,
    };
    for tag in [0, -1, 2] {
        invalid.valid_utf8 = tag;
        let status = unsafe {
            prodex_mojo_launch_args_v1(
                1,
                0,
                0,
                &invalid as *const View as u64,
                1,
                0,
                0,
                0,
                meta.as_mut_ptr() as u64,
            )
        };
        assert_eq!(status, 1);
    }
}

#[test]
fn launch_model_whitespace_matches_rust_not_python_classification() {
    for text in ["\u{001c}", "\u{001d}", "\u{001e}", "\u{001f}"] {
        assert_eq!(
            inspect_launch_arguments(&[Some("-m"), Some(text)])
                .unwrap()
                .model,
            Some(text)
        );
    }
    for text in ["", "\u{0085}", "\u{00a0}", "\u{2007}", "\u{3000}"] {
        assert_eq!(
            inspect_launch_arguments(&[Some("-m"), Some(text)])
                .unwrap()
                .model,
            None
        );
    }
}

#[test]
fn launch_boundary_is_reentrant() {
    std::thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                for _ in 0..200 {
                    let args = [
                        Some("--full-access"),
                        Some("exec"),
                        Some("--"),
                        Some("review"),
                    ];
                    let plan =
                        plan_launch_arguments(&args, LaunchArgumentOperation::Prepare, false)
                            .unwrap();
                    assert_eq!(
                        plan.arguments,
                        vec![
                            LaunchArgument::FullAccess,
                            LaunchArgument::Original(1),
                            LaunchArgument::Original(2),
                            LaunchArgument::Original(3)
                        ]
                    );
                    assert!(!plan.flag);
                }
            });
        }
    });
}
