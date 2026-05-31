use anyhow::{Context, Result, bail};

use crate::{
    AppPaths, AppState, AppStateIoExt, QuotaArgs, QuotaAuthFilter, QuotaProviderFilter,
    collect_quota_reports, collect_quota_reports_with_filters, fetch_profile_quota,
    fetch_profile_quota_json, print_quota_reports, print_stdout_line, quota_watch_enabled,
    render_profile_quota_snapshot, resolve_profile_name, watch_all_quotas, watch_quota,
};

pub(crate) fn handle_quota(args: QuotaArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let auth_filter = args
        .auth
        .as_deref()
        .map(QuotaAuthFilter::parse)
        .transpose()?
        .unwrap_or(QuotaAuthFilter::All);
    let provider_filter = args
        .provider
        .as_deref()
        .map(QuotaProviderFilter::parse)
        .transpose()?
        .unwrap_or(QuotaProviderFilter::All);
    let provider_filter_locked =
        args.provider.is_some() && provider_filter != QuotaProviderFilter::All;

    if args.all {
        if state.profiles.is_empty()
            && !matches!(
                provider_filter,
                QuotaProviderFilter::DeepSeek | QuotaProviderFilter::Local
            )
        {
            bail!("no profiles configured");
        }
        if quota_watch_enabled(&args) {
            return watch_all_quotas(
                &paths,
                args.base_url.as_deref(),
                args.detail,
                auth_filter,
                provider_filter,
                provider_filter_locked,
            );
        }
        let reports = if matches!(auth_filter, QuotaAuthFilter::All)
            && matches!(provider_filter, QuotaProviderFilter::All)
        {
            collect_quota_reports(&state, args.base_url.as_deref())
        } else {
            collect_quota_reports_with_filters(
                &state,
                args.base_url.as_deref(),
                &auth_filter,
                provider_filter,
            )
        };
        print_quota_reports(&reports, args.detail);
        return Ok(());
    }

    let profile_name = resolve_profile_name(&state, args.profile.as_deref())?;
    let profile = state
        .profiles
        .get(&profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?;
    let codex_home = profile.codex_home.clone();

    if args.raw {
        let usage =
            fetch_profile_quota_json(&profile.provider, &codex_home, args.base_url.as_deref())?;
        let json = serde_json::to_string_pretty(&usage).context("failed to render usage JSON")?;
        print_stdout_line(&json);
        return Ok(());
    }

    if quota_watch_enabled(&args) {
        return watch_quota(
            &profile_name,
            &profile.provider,
            &codex_home,
            args.base_url.as_deref(),
        );
    }

    let quota = fetch_profile_quota(&profile.provider, &codex_home, args.base_url.as_deref())?;
    print_stdout_line(&render_profile_quota_snapshot(&profile_name, &quota));
    Ok(())
}
