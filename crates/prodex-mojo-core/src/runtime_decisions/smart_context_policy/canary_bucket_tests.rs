use super::*;

#[test]
fn canary_digest_mapping_is_versioned_and_big_endian() {
    assert_eq!(smart_context_rollout_bucket(&[0; 32]), Ok(0));
    assert_eq!(smart_context_rollout_bucket(&[255; 32]), Ok(1615));
    let mut digest = [0; 32];
    digest[0] = 0x01;
    assert_eq!(smart_context_rollout_bucket(&digest), Ok(7936));
    digest[0] = 0;
    digest[7] = 1;
    assert_eq!(smart_context_rollout_bucket(&digest), Ok(1));
}

#[test]
fn canary_digest_abi_rejects_invalid_inputs_without_writing() {
    let digest = [0_u8; 32];
    let mut output = u64::MAX;
    unsafe {
        assert_eq!(
            prodex_smart_context_rollout_bucket_v1(2, digest.as_ptr() as u64, 32, &mut output,),
            4,
        );
        assert_eq!(
            prodex_smart_context_rollout_bucket_v1(1, digest.as_ptr() as u64, 31, &mut output,),
            1,
        );
        assert_eq!(
            prodex_smart_context_rollout_bucket_v1(1, 0, 32, &mut output),
            1,
        );
    }
    assert_eq!(output, u64::MAX);
}
