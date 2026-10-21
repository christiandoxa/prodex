use super::*;

#[test]
fn compiled_snapshot_query_preserves_defaults_duplicates_and_unsigned_numbers() {
    use RuntimeBrokerSnapshotQuery::Query;
    for (url, after, limit) in [
        ("/snapshot", 0, 64),
        ("/snapshot?", 0, 64),
        ("/snapshot?&&unknown=a=b&&", 0, 64),
        ("/snapshot?after=17&limit=12", 17, 12),
        ("/snapshot?after=1&after=2&limit=3&limit=4", 2, 4),
        ("/snapshot?after=+0007&limit=+064", 7, 64),
        ("/snapshot?after=18446744073709551615", u64::MAX, 64),
        ("/snapshot?AFTER=9&Limit=7&名前=無視", 0, 64),
    ] {
        assert_eq!(
            log_snapshot_query(url, 64),
            Ok(Query { after, limit }),
            "{url}"
        );
    }
}

#[test]
fn compiled_snapshot_query_retains_first_error_and_exact_query_text() {
    use RuntimeBrokerSnapshotQuery as Q;
    for (url, expected) in [
        ("/snapshot?unknown", Q::Malformed),
        ("/snapshot?after=1&broken&after=2", Q::Malformed),
        ("/snapshot?after=", Q::InvalidCursor),
        ("/snapshot?after=+", Q::InvalidCursor),
        ("/snapshot?after=-1", Q::InvalidCursor),
        ("/snapshot?after= 1", Q::InvalidCursor),
        ("/snapshot?after=18446744073709551616", Q::InvalidCursor),
        ("/snapshot?after=%31", Q::InvalidCursor),
        ("/snapshot?after=１", Q::InvalidCursor),
        ("/snapshot?limit=1=2", Q::InvalidLimit),
        ("/snapshot?limit=18446744073709551616", Q::InvalidLimit),
        ("/snapshot?limit=0", Q::LimitOutOfBounds),
        ("/snapshot?limit=65", Q::LimitOutOfBounds),
        ("/snapshot?limit=0&after=nope", Q::LimitOutOfBounds),
        ("/snapshot?after=nope&limit=0", Q::InvalidCursor),
    ] {
        assert_eq!(log_snapshot_query(url, 64), Ok(expected), "{url}");
    }
}

#[test]
fn compiled_snapshot_query_abi_rejects_invalid_shapes_without_writing() {
    let invalid = [0xff_u8];
    let mut output = [77_u64; 3];
    let target = output.as_mut_ptr() as usize as u64;
    for (version, source, length, maximum, address, expected) in [
        (2, 0, 0, 64, target, 4),
        (1, 0, 1, 64, target, 1),
        (1, 0, -1, 64, target, 1),
        (1, 0, 0, 0, target, 1),
        (1, 0, 0, 64, 0, 1),
        (1, 0, 0, 64, target + 1, 1),
        (1, invalid.as_ptr() as usize as u64, 1, 64, target, 1),
    ] {
        assert_eq!(
            unsafe {
                prodex_runtime_broker_snapshot_query_v1(version, source, length, maximum, address)
            },
            expected
        );
        assert_eq!(output, [77; 3]);
    }
    assert_eq!(
        unsafe { prodex_runtime_broker_snapshot_query_v1(1, 0, 0, 64, target) },
        0
    );
    assert_eq!(output, [0, 0, 64]);
}
