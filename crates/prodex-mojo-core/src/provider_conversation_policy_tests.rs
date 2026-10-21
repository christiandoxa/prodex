use super::*;

const BYTE_LIMIT: usize = 64 * 1024 * 1024;

fn entry(key: &str, sequence: u64, estimated_bytes: usize) -> ConversationEntry<'_> {
    ConversationEntry {
        key,
        sequence,
        estimated_bytes,
    }
}

#[test]
fn compiled_history_isolates_scopes_and_preserves_predicate_order_and_latest_ties() {
    let rows = [
        entry("\0prodex:1:a:a", 9, 1),
        entry("\0prodex:1:a:z", 9, 1),
        entry("\0prodex:3:a:b:a", 99, 1),
        entry("\0prodex:2:é:a", 99, 1),
        entry("root-a", 1, 1),
        entry("root-z", 2, 1),
    ];
    // BTreeMap byte order, including UTF-8, is the production input contract.
    let mut rows = rows;
    rows.sort_by_key(|row| row.key);
    let mut visited = Vec::new();
    let selected = latest_matching(&rows, Some("\0prodex:1:a:"), |index| {
        visited.push(rows[index].key);
        true
    })
    .unwrap();
    assert_eq!(visited, ["\0prodex:1:a:a", "\0prodex:1:a:z"]);
    assert_eq!(
        selected.map(|index| rows[index].key),
        Some("\0prodex:1:a:z")
    );
    visited.clear();
    assert_eq!(
        latest_matching(&rows, None, |index| {
            visited.push(rows[index].key);
            rows[index].key == "root-a"
        })
        .unwrap()
        .map(|index| rows[index].key),
        Some("root-a")
    );
    assert_eq!(visited, ["root-a", "root-z"]);
    assert_eq!(
        latest_matching(&rows, Some("absent"), |_| panic!("out-of-scope predicate")).unwrap(),
        None
    );
    assert_eq!(
        latest_matching(&[], None, |_| panic!("empty predicate")).unwrap(),
        None
    );
}

#[test]
fn compiled_replacement_and_oversize_rejection_account_exactly() {
    let rows = [entry("a", 1, 10), entry("b", 2, 20)];
    assert_eq!(
        retention_plan(&rows, None, "a", 30, 2).unwrap(),
        ConversationRetention {
            keep: vec![false, true],
            keep_incoming: true,
            next_sequence: 3,
            estimated_bytes: 50,
        }
    );
    assert_eq!(
        retention_plan(&rows, None, "a", BYTE_LIMIT + 1, 2).unwrap(),
        ConversationRetention {
            keep: vec![false, true],
            keep_incoming: false,
            next_sequence: 2,
            estimated_bytes: 20,
        }
    );
    assert_eq!(
        retention_plan(&[], None, "a", BYTE_LIMIT, 0)
            .unwrap()
            .estimated_bytes,
        BYTE_LIMIT
    );
}

#[test]
fn compiled_scope_limit_and_saturated_sequence_use_lexical_oldest_ties() {
    let keys = (0..256)
        .map(|index| format!("b-{index:04}"))
        .collect::<Vec<_>>();
    let rows = keys
        .iter()
        .map(|key| entry(key, u64::MAX, 1))
        .collect::<Vec<_>>();
    assert_eq!(
        retention_plan(&rows, None, "a", 1, u64::MAX).unwrap(),
        ConversationRetention {
            keep: vec![true; 256],
            keep_incoming: false,
            next_sequence: u64::MAX,
            estimated_bytes: 256,
        }
    );
    let plan = retention_plan(&rows, None, "z", 1, u64::MAX).unwrap();
    assert!(!plan.keep[0]);
    assert!(plan.keep[1..].iter().all(|keep| *keep));
    assert!(plan.keep_incoming);
}

#[test]
fn compiled_global_entry_and_memory_limits_evict_oldest_across_scopes() {
    let keys = (0..2048)
        .map(|index| format!("\0prodex:4:{index:04}:a"))
        .collect::<Vec<_>>();
    let rows = keys
        .iter()
        .enumerate()
        .map(|(index, key)| entry(key, index as u64, 1))
        .collect::<Vec<_>>();
    let plan = retention_plan(&rows, Some("\0prodex:3:new:"), "\0prodex:3:new:a", 1, 2048).unwrap();
    assert!(!plan.keep[0]);
    assert!(plan.keep[1..].iter().all(|keep| *keep));
    assert!(plan.keep_incoming);
    assert_eq!(plan.estimated_bytes, 2048);
    let rows = [entry("a", 1, BYTE_LIMIT / 2), entry("b", 2, BYTE_LIMIT / 2)];
    assert_eq!(
        retention_plan(&rows, None, "c", 1, 2).unwrap(),
        ConversationRetention {
            keep: vec![false, true],
            keep_incoming: true,
            next_sequence: 3,
            estimated_bytes: BYTE_LIMIT / 2 + 1,
        }
    );
}

#[test]
fn compiled_boundary_rejects_invalid_rows_before_mutating_outputs() {
    let rows = [entry("b", 1, 1), entry("a", 1, 1)];
    assert_eq!(
        retention_plan(&rows, None, "c", 1, 1),
        Err(MojoError::InvalidInput)
    );
    assert_eq!(
        latest_matching(&rows, None, |_| panic!("invalid rows")),
        Err(MojoError::InvalidInput)
    );
    assert_eq!(
        retention_plan(&[entry("a", 2, 1)], None, "b", 1, 1),
        Err(MojoError::InvalidInput)
    );
    assert_eq!(
        retention_plan(&[entry("a", 1, BYTE_LIMIT + 1)], None, "b", 1, 1),
        Err(MojoError::InvalidInput)
    );
    let rows = [Row::from(&entry("a", 0, 1))];
    let mut keep = [7_u8];
    let mut scratch = [99_i64];
    let mut output = [99_u64; 2];
    let mut call = |version, capacity| unsafe {
        prodex_provider_conversation_retention_v1(
            version,
            0,
            0,
            0,
            rows.as_ptr() as u64,
            0,
            keep.as_mut_ptr() as u64,
            capacity,
            scratch.as_mut_ptr() as u64,
            1,
            output.as_mut_ptr() as u64,
        )
    };
    assert_eq!(call(2, 1), 4);
    assert_eq!(call(1, 0), 3);
    assert_eq!(keep, [7]);
    assert_eq!(scratch, [99]);
    assert_eq!(output, [99; 2]);
}
