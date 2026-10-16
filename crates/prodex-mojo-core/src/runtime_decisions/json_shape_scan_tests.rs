use super::*;

#[test]
fn mojo_json_shape_scan_owns_depth_first_and_node_limits() {
    use SmartContextJsonShapeViolation::{DepthLimit, NodeLimit};
    assert_eq!(smart_context_json_shape_scan(&[1, 64, 1], 0), Ok(None));
    assert_eq!(
        smart_context_json_shape_scan(&[65], 0),
        Ok(Some(DepthLimit))
    );
    assert_eq!(
        smart_context_json_shape_scan(&[64], 50_000),
        Ok(Some(NodeLimit))
    );
    assert_eq!(
        smart_context_json_shape_scan(&[65], 50_000),
        Ok(Some(DepthLimit))
    );
    assert_eq!(
        smart_context_json_shape_scan(&[64, 65], 49_999),
        Ok(Some(DepthLimit))
    );
    assert_eq!(
        smart_context_json_shape_scan(&[64, 64], 49_999),
        Ok(Some(NodeLimit))
    );
    assert_eq!(
        smart_context_json_shape_scan(&[1_024; 1], 0),
        Ok(Some(DepthLimit))
    );
    assert_eq!(
        smart_context_json_shape_scan(&[1; 1_024], 48_977),
        Ok(Some(NodeLimit))
    );
    assert_eq!(smart_context_json_shape_scan(&[1; 1_024], 48_976), Ok(None));
}

#[test]
fn mojo_json_shape_scan_abi_is_fail_closed_on_untrusted_depths() {
    let mut violation = 77_i64;
    let mut depths = [64_u64, 0];
    unsafe {
        assert_eq!(
            prodex_smart_context_json_shape_scan_v1(
                2,
                depths.as_ptr() as u64,
                1,
                0,
                &mut violation,
            ),
            4,
        );
        assert_eq!(
            prodex_smart_context_json_shape_scan_v1(
                1,
                depths.as_ptr() as u64,
                2,
                0,
                &mut violation,
            ),
            1,
        );
        depths[1] = 64;
        assert_eq!(
            prodex_smart_context_json_shape_scan_v1(
                1,
                depths.as_ptr() as u64,
                2,
                50_001,
                &mut violation,
            ),
            1,
        );
        assert_eq!(
            prodex_smart_context_json_shape_scan_v1(1, 0, 2, 0, &mut violation,),
            1,
        );
    }
    assert_eq!(violation, 77);
}
