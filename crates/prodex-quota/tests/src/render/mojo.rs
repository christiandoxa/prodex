use super::*;

#[test]
fn remaining_percent_matches_rust_oracle() {
    for (used_percent, expected) in [
        (None, 0),
        (Some(i64::MIN), 100),
        (Some(-1), 100),
        (Some(0), 100),
        (Some(42), 58),
        (Some(100), 0),
        (Some(101), 0),
        (Some(i64::MAX), 0),
    ] {
        assert_eq!(remaining_percent(used_percent), expected);
        let rust = used_percent.map_or(0, |used| {
            if used < 0 {
                100
            } else if used > 100 {
                0
            } else {
                100 - used
            }
        });
        assert_eq!(remaining_percent(used_percent), rust);
    }
}
