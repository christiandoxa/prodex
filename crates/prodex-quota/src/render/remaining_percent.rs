#[cfg(feature = "mojo")]
pub fn remaining_percent(used_percent: Option<i64>) -> i64 {
    crate::mojo::remaining_percent(used_percent)
}

#[cfg(not(feature = "mojo"))]
pub fn remaining_percent(used_percent: Option<i64>) -> i64 {
    let Some(used) = used_percent else {
        return 0;
    };
    if used < 0 {
        return 100;
    }
    if used > 100 {
        return 0;
    }
    100 - used
}
