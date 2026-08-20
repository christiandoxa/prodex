unsafe extern "C" {
    fn prodex_quota_remaining_percent(used_percent: i64, has_value: i64) -> i64;
}

pub(super) fn remaining_percent(used_percent: Option<i64>) -> i64 {
    let (used_percent, has_value) = match used_percent {
        Some(value) => (value, 1),
        None => (0, 0),
    };

    // SAFETY: build.rs links the scalar-only Mojo C-ABI object when this feature is enabled.
    unsafe { prodex_quota_remaining_percent(used_percent, has_value) }
}
