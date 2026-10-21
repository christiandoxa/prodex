use super::*;

#[test]
fn compiled_main_model_insertion_uses_first_default_or_custom_and_preserves_order() {
    use CatalogInsertionChoice::{Custom, Model, ProviderDefault};
    use CatalogInsertionPlacement::{BundledAfterDefault, CurrentBeforeCustom};
    assert_eq!(
        catalog_insertion_index(&[ProviderDefault, Model, Custom], CurrentBeforeCustom),
        Ok(2),
    );
    assert_eq!(
        catalog_insertion_index(&[ProviderDefault, Model, Custom], BundledAfterDefault),
        Ok(1),
    );
    assert_eq!(
        catalog_insertion_index(
            &[Model, Custom, Model, Custom, ProviderDefault],
            CurrentBeforeCustom
        ),
        Ok(1),
    );
    assert_eq!(
        catalog_insertion_index(
            &[Model, Custom, Model, Custom, ProviderDefault],
            BundledAfterDefault
        ),
        Ok(5),
    );
    assert_eq!(
        catalog_insertion_index(&[Model, Model], CurrentBeforeCustom),
        Ok(2),
    );
    assert_eq!(
        catalog_insertion_index(&[Model, Model], BundledAfterDefault),
        Ok(0),
    );
    assert_eq!(catalog_insertion_index(&[], CurrentBeforeCustom), Ok(0));
    assert_eq!(catalog_insertion_index(&[], BundledAfterDefault), Ok(0));
    let many = vec![Model; 1025];
    assert_eq!(
        catalog_insertion_index(&many, CurrentBeforeCustom),
        Ok(1025)
    );
}

#[test]
fn compiled_main_model_insertion_abi_rejects_late_bad_tag_without_mutation() {
    let mut tags = [0_i64, 1, 4, 1];
    let mut output = 91_i64;
    let source = tags.as_ptr() as usize as u64;
    let result = (&mut output as *mut i64) as usize as u64;
    for (version, mode, ptr, count, output_ptr, expected) in [
        (5, 0, source, 4, result, 4),
        (6, -1, source, 4, result, 1),
        (6, 2, source, 4, result, 1),
        (6, 0, 0, 4, result, 1),
        (6, 0, source, -1, result, 1),
        (6, 0, source, 65_537, result, 1),
        (6, 0, source, 4, 0, 1),
    ] {
        assert_eq!(
            unsafe {
                prodex_mojo_rich_catalog_insertion_index_v1(version, mode, ptr, count, output_ptr)
            },
            expected,
        );
        assert_eq!(output, 91);
    }
    tags[3] = 99;
    assert_eq!(
        unsafe {
            prodex_mojo_rich_catalog_insertion_index_v1(
                RICH_ABI_VERSION,
                1,
                tags.as_ptr() as usize as u64,
                4,
                result,
            )
        },
        1,
    );
    assert_eq!(output, 91);
    assert_eq!(
        unsafe { prodex_mojo_rich_catalog_insertion_index_v1(6, 0, 0, 0, result) },
        0,
    );
    assert_eq!(output, 0);
}
