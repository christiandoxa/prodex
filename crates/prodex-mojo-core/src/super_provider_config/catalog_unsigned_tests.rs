use super::*;

fn check(value: &str) {
    let actual = catalog_unsigned_value(value).unwrap();
    if value.is_empty() {
        assert_eq!(actual, CatalogUnsignedPlan::Empty);
    } else if value.chars().any(char::is_whitespace) {
        assert_eq!(actual, CatalogUnsignedPlan::Whitespace);
    } else {
        match value.parse::<u64>() {
            Ok(parsed) if parsed > 1 => assert_eq!(actual, CatalogUnsignedPlan::Value(parsed)),
            Ok(_) => assert_eq!(actual, CatalogUnsignedPlan::TooSmall),
            Err(error) => {
                let CatalogUnsignedPlan::InvalidUnsignedInteger { reason } = actual else {
                    panic!("expected an integer source error, got {actual:?}");
                };
                assert_eq!(reason, error.to_string());
            }
        }
    }
}

#[test]
fn compiled_catalog_unsigned_plan_preserves_rust_integer_errors_and_whitespace_order() {
    for prefix in [
        "",
        "+",
        "-",
        "000",
        "+000",
        "1844674407370955161",
        "18446744073709551616",
    ] {
        for suffix in [
            "", "0", "1", "2", "5", "6", "x", "x6", " ", "\u{a0}", "é", "\u{200b}",
        ] {
            check(&format!("{prefix}{suffix}"));
        }
    }
    check("18446744073709551615");
    check("18446744073709551616");
    let long = format!("{}2", "0".repeat(1_048_577));
    assert_eq!(
        catalog_unsigned_value(&long),
        Ok(CatalogUnsignedPlan::Value(2))
    );
}

#[test]
fn compiled_catalog_unsigned_abi_rejects_bad_shapes_without_writes() {
    let mut output = [77_u64; 2];
    let address = output.as_mut_ptr() as u64;
    assert_eq!(
        unsafe { prodex_provider_catalog_uint_v1(2, 0, 0, address) },
        4
    );
    assert_eq!(
        unsafe { prodex_provider_catalog_uint_v1(1, 0, 1, address) },
        1
    );
    assert_eq!(
        unsafe { prodex_provider_catalog_uint_v1(1, 0, -1, address) },
        1
    );
    let malformed = [255_u8];
    assert_eq!(
        unsafe { prodex_provider_catalog_uint_v1(1, malformed.as_ptr() as u64, 1, address) },
        1
    );
    assert_eq!(output, [77; 2]);
}
