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
