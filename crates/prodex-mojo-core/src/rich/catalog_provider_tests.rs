use super::*;

#[test]
fn provider_catalog_indices_keep_order_and_handle_empty_results() {
    let providers = [2, 0, 2, 6, 1, 6];

    assert_eq!(
        CatalogModel::provider_indices(&providers, 6).unwrap(),
        [3, 5]
    );
    assert_eq!(CatalogModel::provider_indices(&providers, 99).unwrap(), []);
    assert_eq!(CatalogModel::provider_indices(&[], 0).unwrap(), []);
}

#[test]
fn provider_catalog_indices_reject_oversized_input() {
    let providers = vec![0; CATALOG_MAX_MODELS + 1];
    assert_eq!(
        CatalogModel::provider_indices(&providers, 0),
        Err(MojoError::InvalidInput)
    );
}

#[test]
fn provider_catalog_indices_abi_enforces_version_and_output_capacity() {
    let mut count = -1_i64;
    assert_eq!(
        unsafe {
            prodex_mojo_rich_catalog_provider_indices_v1(
                0,
                0,
                0,
                0,
                0,
                0,
                count_address(&mut count),
            )
        },
        RICH_STATUS_ABI
    );

    let providers = [4_i64, 4];
    let mut indices = [-1_i64];
    count = -1;
    assert_eq!(
        unsafe {
            prodex_mojo_rich_catalog_provider_indices_v1(
                RICH_ABI_VERSION,
                address(&providers),
                providers.len() as i64,
                4,
                mojo_mut_pointer_address(indices.as_mut_ptr()),
                indices.len() as i64,
                count_address(&mut count),
            )
        },
        RICH_STATUS_CAPACITY
    );
    assert_eq!(count, 1);
    assert_eq!(indices, [0]);
}
