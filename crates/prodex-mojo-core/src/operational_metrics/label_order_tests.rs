use super::*;

#[test]
fn compiled_metric_pair_order_keeps_secondary_values_unicode_and_stable_duplicates() {
    let labels = [
        ("b", "x"),
        ("a", "z"),
        ("a", "a"),
        ("a", "a"),
        ("é", "x"),
        ("a", ""),
        ("A", "x"),
    ];
    assert_eq!(
        operational_metric_label_order(&labels),
        Ok(vec![6, 5, 2, 3, 1, 0, 4])
    );
    assert_eq!(operational_metric_label_order(&[]), Ok(vec![]));
    assert_eq!(operational_metric_label_order(&[("", "")]), Ok(vec![0]));
}

#[test]
fn compiled_metric_pair_order_is_bounded_and_stable_at_capacity() {
    let labels = vec![("key", "value"); 65_536];
    assert_eq!(
        operational_metric_label_order(&labels),
        Ok((0..65_536).collect())
    );
    assert_eq!(
        operational_metric_label_order(&vec![("key", "value"); 65_537]),
        Err(MojoError::InvalidInput)
    );
    assert_eq!(
        operational_metric_label_order(&[("key", &"x".repeat(129))]),
        Err(MojoError::InvalidInput)
    );
}

#[test]
fn metric_order_abi_validates_all_views_and_capacity_before_writes() {
    let mut fields = [["key".as_ptr() as u64, 3], ["value".as_ptr() as u64, 5]];
    let mut indices = [73_i64; 4];
    let mut call = |version, fields, count, capacity| unsafe {
        prodex_mojo_operational_metric_label_order_v1(
            version,
            fields,
            count,
            indices.as_mut_ptr() as u64,
            capacity,
        )
    };
    let table = fields.as_ptr() as u64;
    assert_eq!(call(2, 0, 0, 4), 4);
    assert_eq!(call(1, 0, 1, 4), 1);
    assert_eq!(call(1, table + 1, 1, 4), 1);
    assert_eq!(call(1, table, -1, 4), 1);
    assert_eq!(call(1, table, 65_537, 4), 1);
    assert_eq!(call(1, table, 1, -1), 1);
    assert_eq!(call(1, table, 1, 0), 2);
    fields[1] = ["é".as_ptr() as u64, 1];
    assert_eq!(call(1, fields.as_ptr() as u64, 1, 4), 1);
    fields[1] = [0, 129];
    assert_eq!(call(1, fields.as_ptr() as u64, 1, 4), 1);
    assert_eq!(indices, [73; 4]);
}
