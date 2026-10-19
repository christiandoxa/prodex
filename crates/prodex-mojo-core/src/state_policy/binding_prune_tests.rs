use super::*;

#[test]
fn compiled_binding_pruning_preserves_conflicts_signed_times_and_ties() {
    let ranks = [
        [1, i64::MIN],
        [0, 5],
        [0, -1],
        [0, 5],
        [1, i64::MAX],
        [0, i64::MIN],
    ];
    assert_eq!(
        binding_prune_indices(ranks.into_iter(), 3),
        Ok(vec![5, 2, 1])
    );
    assert_eq!(
        binding_prune_indices(ranks.into_iter(), 0),
        Ok(vec![5, 2, 1, 3, 0, 4])
    );
    fn check_columns<const WIDTH: usize>() {
        for column in 0..WIDTH {
            let mut ranks = [[0_i64; WIDTH]; 4];
            ranks[0][column] = 1;
            ranks[2][column] = -1;
            assert_eq!(
                binding_prune_indices(ranks.into_iter(), 1),
                Ok(vec![2, 1, 3])
            );
        }
    }
    check_columns::<2>();
    check_columns::<9>();
    check_columns::<10>();
}

#[test]
fn compiled_binding_pruning_skips_lazy_facts_when_every_entry_is_kept() {
    for limit in [4, usize::MAX] {
        let facts = (0..4).map(|_| -> [i64; 2] { panic!("no-op pruning acquired ranks") });
        assert_eq!(binding_prune_indices(facts, limit), Ok(vec![]));
    }
    assert_eq!(
        binding_prune_indices(std::iter::empty::<[i64; 9]>(), 0),
        Ok(vec![])
    );
}

#[test]
fn binding_pruning_abi_checks_shapes_before_output_or_fact_reads() {
    let ranks = [[0_i64, 1], [0, 1], [1, i64::MIN]];
    let source = ranks.as_ptr() as u64;
    let mut output = [71_i64; 4];
    let destination = output.as_mut_ptr() as u64;
    let call = |version, rows, count, width, limit, out, capacity| unsafe {
        prodex_state_binding_prune_v1(version, rows, count, width, limit, out, capacity)
    };
    assert_eq!(call(2, source, 3, 2, 1, destination, 4), 4);
    for (count, width) in [(-1, 2), (i64::MAX, 2), (3, 0), (3, 3), (3, -1)] {
        assert_eq!(call(1, source, count, width, 1, destination, 4), 1);
    }
    assert_eq!(call(1, source, 3, 2, 1, 0, 4), 1);
    assert_eq!(call(1, source, 3, 2, 1, 1, 4), 1);
    assert_eq!(call(1, 1, 3, 2, 1, destination, 4), 1);
    assert_eq!(call(1, source, 3, 2, 1, destination, 3), 3);
    assert_eq!(call(1, 0, 3, 2, 1, destination, 1), NEED_RANKS);
    assert_eq!(output, [71; 4]);
    assert_eq!(call(1, 0, 3, 2, u64::MAX, destination, 1), 0);
    assert_eq!(output, [0, 71, 71, 71]);
}

#[test]
fn binding_pruning_rejects_iterators_that_misreport_their_length() {
    struct WrongLength;
    impl Iterator for WrongLength {
        type Item = [i64; 2];
        fn next(&mut self) -> Option<Self::Item> {
            None
        }
        fn size_hint(&self) -> (usize, Option<usize>) {
            (2, Some(2))
        }
    }
    impl ExactSizeIterator for WrongLength {}
    assert_eq!(
        binding_prune_indices(WrongLength, 0),
        Err(MojoError::InvalidInput)
    );
}
