use super::*;

pub(crate) fn deserialize_null_default<'de, D, T>(
    deserializer: D,
) -> std::result::Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

pub(crate) fn handle_quota(args: QuotaArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;

    if args.all {
        if state.profiles.is_empty() {
            bail!("no profiles configured");
        }
        if quota_watch_enabled(&args) {
            return watch_all_quotas(&paths, args.base_url.as_deref(), args.detail);
        }
        let reports = collect_quota_reports(&state, args.base_url.as_deref());
        print_quota_reports(&reports, args.detail);
        return Ok(());
    }

    let profile_name = resolve_profile_name(&state, args.profile.as_deref())?;
    let profile = state
        .profiles
        .get(&profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?;
    if !profile.provider.supports_codex_runtime() {
        bail!(
            "profile '{}' uses {}. `prodex quota` currently supports OpenAI/Codex profiles only.",
            profile_name,
            profile.provider.display_name()
        );
    }
    let codex_home = profile.codex_home.clone();

    if args.raw {
        let usage = fetch_usage_json(&codex_home, args.base_url.as_deref())?;
        println!(
            "{}",
            serde_json::to_string_pretty(&usage).context("failed to render usage JSON")?
        );
        return Ok(());
    }

    if quota_watch_enabled(&args) {
        return watch_quota(&profile_name, &codex_home, args.base_url.as_deref());
    }

    let usage = fetch_usage(&codex_home, args.base_url.as_deref())?;
    println!("{}", render_profile_quota(&profile_name, &usage));
    Ok(())
}
