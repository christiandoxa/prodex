use super::*;

fn fact<'a>(
    response: Option<&'a str>,
    profile: &'a str,
    timestamp: i64,
) -> RuntimeLineageLatestBindingFact<'a> {
    RuntimeLineageLatestBindingFact {
        response_id: response,
        profile_name: profile,
        bound_at: timestamp,
    }
}

#[test]
fn compiled_lineage_latest_binding_selects_last_newest_matching_identity_and_profile() {
    let candidates = [
        fact(Some("resp:雪"), "main", i64::MIN),
        fact(Some("resp:雪"), "other", i64::MAX),
        fact(None, "main", i64::MAX),
        fact(Some("resp"), "main", i64::MAX),
        fact(Some("resp:雪"), "main", -1),
        fact(Some("resp:雪"), "main", 8),
        fact(Some("resp:雪"), "main", 8),
        fact(Some("resp:雪"), "other", i64::MIN),
    ];
    assert_eq!(
        runtime_lineage_latest_binding_index(&candidates, "resp:雪", Some("main")),
        Ok(Some(6)),
    );
    assert_eq!(
        runtime_lineage_latest_binding_index(&candidates, "resp:雪", None),
        Ok(Some(1)),
    );
    assert_eq!(
        runtime_lineage_latest_binding_index(&candidates, "resp:雪", Some("missing")),
        Ok(None),
    );
    assert_eq!(
        runtime_lineage_latest_binding_index(&candidates, "resp", Some("main")),
        Ok(Some(3)),
    );
    assert_eq!(
        runtime_lineage_latest_binding_index(&[], "resp", None),
        Ok(None)
    );
    assert_eq!(
        runtime_lineage_latest_binding_index(&[fact(Some(""), "", -1)], "", Some(""),),
        Ok(Some(0)),
    );
    let many = vec![fact(Some("resp"), "main", 1); 1025];
    assert_eq!(
        runtime_lineage_latest_binding_index(&many, "resp", Some("main")),
        Ok(Some(1024)),
        "equal timestamps choose the final source row",
    );
}

#[test]
fn compiled_lineage_latest_binding_abi_rejects_late_invalid_row_without_write() {
    let mut facts = [0_u64; 10];
    let mut timestamps = [10_i64; 2];
    let mut selected = 93_i64;
    let source = facts.as_ptr() as usize as u64;
    let times = timestamps.as_ptr() as usize as u64;
    let result = (&mut selected as *mut i64) as usize as u64;
    let query = b"resp";
    let profile = b"main";
    let query_ptr = query.as_ptr() as usize as u64;
    let profile_ptr = profile.as_ptr() as usize as u64;
    for (
        version,
        rows,
        timestamp_ptr,
        count,
        resp_ptr,
        resp_len,
        profile_present,
        destination,
        expected,
    ) in [
        (2, source, times, 2, query_ptr, 4, 1, result, 4),
        (1, 0, times, 2, query_ptr, 4, 1, result, 1),
        (1, source, 0, 2, query_ptr, 4, 1, result, 1),
        (1, source, times, -1, query_ptr, 4, 1, result, 1),
        (1, source, times, 65_537, query_ptr, 4, 1, result, 1),
        (1, source, times, 2, 0, 4, 1, result, 1),
        (1, source, times, 2, query_ptr, -1, 1, result, 1),
        (1, source, times, 2, query_ptr, 4, 2, result, 1),
        (1, source, times, 2, query_ptr, 4, 1, 0, 1),
    ] {
        assert_eq!(
            unsafe {
                prodex_runtime_lineage_latest_turn_binding_v1(
                    version,
                    rows,
                    timestamp_ptr,
                    count,
                    resp_ptr,
                    resp_len,
                    profile_ptr,
                    4,
                    profile_present,
                    destination,
                )
            },
            expected,
        );
        assert_eq!(selected, 93);
    }
    facts[9] = 2;
    assert_eq!(
        unsafe {
            prodex_runtime_lineage_latest_turn_binding_v1(
                1,
                facts.as_ptr() as usize as u64,
                times,
                2,
                query_ptr,
                4,
                profile_ptr,
                4,
                1,
                result,
            )
        },
        1,
    );
    assert_eq!(selected, 93);
    facts[9] = 1;
    facts[5] = query_ptr;
    facts[6] = 4;
    timestamps[1] = i64::MAX;
    assert_eq!(
        unsafe {
            prodex_runtime_lineage_latest_turn_binding_v1(
                1,
                facts.as_ptr() as usize as u64,
                timestamps.as_ptr() as usize as u64,
                2,
                query_ptr,
                4,
                profile_ptr,
                4,
                1,
                result,
            )
        },
        0,
    );
    assert_eq!(
        selected, -1,
        "no profile rows qualify without their profile views"
    );
}
