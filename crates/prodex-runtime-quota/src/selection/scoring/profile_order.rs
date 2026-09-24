use super::*;

pub fn provider_aware_profile_order_with_view<S: ProfileSelectionRead, I>(
    selection: S,
    names: I,
) -> Vec<String>
where
    I: IntoIterator<Item = String>,
{
    let names = names.into_iter().collect::<Vec<_>>();
    let priorities = names
        .iter()
        .map(|name| {
            selection
                .profile_entry(name)
                .map(ProfileSelectionProvider::runtime_pool_priority)
                .unwrap_or(usize::MAX)
        })
        .collect::<Vec<_>>();
    let order = (0..names.len()).collect::<Vec<_>>();
    let order = prodex_mojo_core::runtime::provider_aware_profile_order_batch(&priorities, &order)
        .expect("Mojo provider profile order returned invalid output");
    order
        .into_iter()
        .map(|index| names[index].clone())
        .collect()
}

#[cfg(test)]
fn provider_aware_profile_order_rust<S: ProfileSelectionRead, I>(
    selection: S,
    names: I,
) -> Vec<String>
where
    I: IntoIterator<Item = String>,
{
    let mut ordered = names
        .into_iter()
        .enumerate()
        .map(|(index, name)| {
            let provider_priority = selection
                .profile_entry(&name)
                .map(ProfileSelectionProvider::runtime_pool_priority)
                .unwrap_or(usize::MAX);
            (provider_priority, index, name)
        })
        .collect::<Vec<_>>();
    ordered.sort_by_key(|(provider_priority, index, _)| (*provider_priority, *index));
    ordered.into_iter().map(|(_, _, name)| name).collect()
}

#[cfg(test)]
mod mojo_profile_order_parity_tests {
    use super::*;

    struct Entry {
        name: String,
        priority: usize,
    }

    impl ProfileSelectionProvider for Entry {
        fn runtime_pool_priority(&self) -> usize {
            self.priority
        }
    }

    #[derive(Clone, Copy)]
    struct View<'a> {
        entries: &'a [Entry],
    }

    impl ProfileSelectionRead for View<'_> {
        type Profile = Entry;

        fn profile_names(&self) -> Vec<String> {
            self.entries
                .iter()
                .map(|entry| entry.name.clone())
                .collect()
        }

        fn profile_entry(&self, name: &str) -> Option<&Self::Profile> {
            self.entries.iter().find(|entry| entry.name == name)
        }

        fn last_run_selected_at(&self, _: &str) -> Option<i64> {
            None
        }
    }

    fn next(seed: &mut u64) -> u64 {
        *seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        *seed
    }

    #[test]
    fn profile_order_wrappers_match_pre_migration_rust_oracle() {
        const { assert!(prodex_mojo_core::MOJO_ACTIVE) };
        let mut seed = 0x1f83_d9ab_fb41_bd6b_u64;
        for case_index in 0..1_024 {
            let count = 2 + (next(&mut seed) % 15) as usize;
            let entries = (0..count)
                .map(|index| Entry {
                    name: format!("profile-{case_index}-{index}"),
                    priority: (next(&mut seed) % 4) as usize,
                })
                .collect::<Vec<_>>();
            let selection = View { entries: &entries };
            let current = if next(&mut seed).is_multiple_of(5) {
                "missing-profile".to_string()
            } else {
                entries[(next(&mut seed) as usize) % count].name.clone()
            };
            let names = selection.profile_names();
            assert_eq!(
                provider_aware_profile_order_with_view(selection, names.clone()),
                provider_aware_profile_order_rust(selection, names),
                "provider order mismatch: seed=0x1f83d9abfb41bd6b case={case_index}"
            );
            let rust_rotation =
                if let Some(index) = entries.iter().position(|entry| entry.name == current) {
                    provider_aware_profile_order_rust(
                        selection,
                        entries
                            .iter()
                            .skip(index + 1)
                            .chain(entries.iter().take(index))
                            .map(|entry| entry.name.clone()),
                    )
                } else {
                    provider_aware_profile_order_rust(
                        selection,
                        entries
                            .iter()
                            .filter(|entry| entry.name != current)
                            .map(|entry| entry.name.clone()),
                    )
                };
            assert_eq!(
                profile_rotation_order_with_view(selection, &current),
                rust_rotation,
                "rotation order mismatch: seed=0x1f83d9abfb41bd6b case={case_index}"
            );
            let rust_active = provider_aware_profile_order_rust(
                selection,
                std::iter::once(current.clone()).chain(rust_rotation),
            );
            assert_eq!(
                active_profile_selection_order_with_view(selection, &current),
                rust_active,
                "active order mismatch: seed=0x1f83d9abfb41bd6b case={case_index}"
            );
        }
    }
}
