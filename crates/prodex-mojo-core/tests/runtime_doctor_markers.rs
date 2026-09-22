#![cfg(feature = "mojo-rich")]
#![allow(unsafe_code)]
use prodex_mojo_core::rich::{runtime_doctor_marker_known, runtime_doctor_marker_semantics};

#[test]
fn long_unknown_marker_is_not_an_invalid_event() {
    for length in [0, 1, 255, 256, 257, 4096, 1_048_640] {
        for symbol in ["x", "🦀"] {
            let text = symbol.repeat(length);
            assert!(!runtime_doctor_marker_known(&text).unwrap());
            let semantics = runtime_doctor_marker_semantics(&text).unwrap();
            assert_eq!(
                (
                    semantics.timeline_phase,
                    semantics.selection_bucket,
                    semantics.route_action
                ),
                (0, 0, 0)
            );
        }
    }
    assert!(runtime_doctor_marker_known("runtime_proxy_queue_overloaded").unwrap());
    assert!(runtime_doctor_marker_known("first_local_chunk").unwrap());
}

#[repr(C)]
struct View {
    ptr: u64,
    len: u64,
}
unsafe extern "C" {
    fn prodex_mojo_runtime_doctor_marker_known_v1(abi: i64, marker: u64, output: u64) -> i64;
    fn prodex_mojo_runtime_doctor_marker_semantics_v1(abi: i64, marker: u64, output: u64) -> i64;
}

#[test]
fn long_marker_support_does_not_accept_invalid_utf8_or_abi() {
    for length in [1, 256, 257, 4096] {
        let mut bytes = vec![b'x'; length];
        bytes[length - 1] = 0xff;
        let view = View {
            ptr: bytes.as_ptr() as u64,
            len: bytes.len() as u64,
        };
        for function in [
            prodex_mojo_runtime_doctor_marker_known_v1,
            prodex_mojo_runtime_doctor_marker_semantics_v1,
        ] {
            let mut output = [77_i64; 3];
            let code =
                unsafe { function(1, &view as *const View as u64, output.as_mut_ptr() as u64) };
            assert_eq!(code, 2);
            assert_eq!(output, [77; 3]);
            let code =
                unsafe { function(9, &view as *const View as u64, output.as_mut_ptr() as u64) };
            assert_eq!(code, 1);
            assert_eq!(output, [77; 3]);
        }
    }
}
