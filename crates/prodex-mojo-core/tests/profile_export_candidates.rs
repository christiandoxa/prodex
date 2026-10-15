#![cfg(feature = "mojo-runtime")]

#[repr(C)]
#[derive(Clone, Copy)]
struct View {
    ptr: u64,
    len: u64,
}

unsafe extern "C" {
    fn prodex_profile_export_copilot_candidate_users_v1(
        abi_version: i64,
        last_present: i64,
        last_host_address: u64,
        last_login_address: u64,
        logged_hosts_address: u64,
        logged_logins_address: u64,
        logged_count: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

fn view(value: &[u8]) -> View {
    if value.is_empty() {
        View { ptr: 0, len: 0 }
    } else {
        View {
            ptr: value.as_ptr() as usize as u64,
            len: value.len() as u64,
        }
    }
}

fn raw_call(
    abi: (i64, i64),
    last_host: &View,
    last_login: &View,
    logged_hosts: &[View],
    logged_logins: &[View],
    output: &mut [i64],
    written: &mut i64,
) -> i64 {
    // SAFETY: all views, slices, and output remain alive for the ABI call.
    unsafe {
        prodex_profile_export_copilot_candidate_users_v1(
            abi.0,
            abi.1,
            last_host as *const View as usize as u64,
            last_login as *const View as usize as u64,
            logged_hosts.as_ptr() as usize as u64,
            logged_logins.as_ptr() as usize as u64,
            logged_hosts.len() as i64,
            output.as_mut_ptr() as usize as u64,
            output.len() as i64,
            written as *mut i64 as usize as u64,
        )
    }
}

#[test]
fn compiled_copilot_candidate_abi_deduplicates_without_reordering() {
    const { assert!(prodex_mojo_core::MOJO_REQUIRED) }

    let last_host = view(b"https://ghe.example");
    let last_login = view(b"main");
    let logged_hosts = [
        view(b"https://ghe.example"),
        view(b"https://github.com"),
        view(b"https://github.com"),
    ];
    let logged_logins = [view(b"main"), view(b"fallback"), view(b"fallback")];
    let mut output = [91_i64; 4];
    let mut written = -1_i64;

    assert_eq!(
        raw_call(
            (1, 1),
            &last_host,
            &last_login,
            &logged_hosts,
            &logged_logins,
            &mut output,
            &mut written,
        ),
        0
    );
    assert_eq!(written, 2);
    assert_eq!(&output[..2], &[-1, 1]);
}

#[test]
fn compiled_copilot_candidate_abi_rejects_bad_version_and_capacity_before_writes() {
    let empty = view(&[]);
    let logged_hosts = [view(b"https://github.com"), view(b"https://ghe.example")];
    let logged_logins = [view(b"one"), view(b"two")];
    let mut output = [91_i64; 2];
    let mut written = 92_i64;

    assert_eq!(
        raw_call(
            (2, 0),
            &empty,
            &empty,
            &logged_hosts,
            &logged_logins,
            &mut output,
            &mut written,
        ),
        100
    );
    assert_eq!(output, [91, 91]);
    assert_eq!(written, 92);

    let mut output = [91_i64; 1];
    assert_eq!(
        raw_call(
            (1, 0),
            &empty,
            &empty,
            &logged_hosts,
            &logged_logins,
            &mut output,
            &mut written,
        ),
        2
    );
    assert_eq!(output, [91]);
    assert_eq!(written, 92);
}
