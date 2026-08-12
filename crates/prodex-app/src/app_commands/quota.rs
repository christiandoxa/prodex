use anyhow::{Context, Result, bail};

use crate::{
    AppPaths, AppState, AppStateIoExt, QuotaArgs, QuotaAuthFilter, QuotaProviderFilter,
    collect_quota_reports, collect_quota_reports_with_filters, fetch_profile_quota,
    fetch_profile_quota_json, print_stdout_line, print_stdout_text, quota_watch_enabled,
    render_all_quota_reports_once_tui, render_profile_quota_once_tui, render_quota_reports,
    resolve_profile_name, save_openai_quota_runtime_usage_snapshot,
    save_openai_quota_runtime_usage_snapshots, watch_all_quotas, watch_quota,
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
        return handle_all_quota(
            &paths,
            &state,
            &args,
            auth_filter,
            provider_filter,
            provider_filter_locked,
        );
    }

    handle_profile_quota(&paths, &state, &args)
}

fn handle_all_quota(
    paths: &AppPaths,
    state: &AppState,
    args: &QuotaArgs,
    auth_filter: QuotaAuthFilter,
    provider_filter: QuotaProviderFilter,
    provider_filter_locked: bool,
) -> Result<()> {
    if state.profiles.is_empty()
        && !matches!(
            provider_filter,
            QuotaProviderFilter::DeepSeek | QuotaProviderFilter::Local | QuotaProviderFilter::Agy
        )
    {
        bail!("no profiles configured");
    }
    if quota_watch_enabled(args) {
        return watch_all_quotas(
            paths,
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
        collect_quota_reports(state, args.base_url.as_deref())
    } else {
        collect_quota_reports_with_filters(
            state,
            args.base_url.as_deref(),
            &auth_filter,
            provider_filter,
        )
    };
    if let Some(mut terminal) = crate::try_inline_stdout_terminal(
        reports
            .len()
            .saturating_mul(if args.detail { 4 } else { 3 })
            .saturating_add(8)
            .clamp(8, 32) as u16,
    ) {
        terminal.draw(|frame| render_all_quota_reports_once_tui(frame, &reports, args.detail))?;
        let _ = terminal.show_cursor();
    } else {
        print_stdout_text(&render_quota_reports(&reports, args.detail))?;
    }
    save_openai_quota_runtime_usage_snapshots(paths, &state.profiles, &reports);
    Ok(())
}

fn handle_profile_quota(paths: &AppPaths, state: &AppState, args: &QuotaArgs) -> Result<()> {
    let profile_name = resolve_profile_name(state, args.profile.as_deref())?;
    let profile = state
        .profiles
        .get(&profile_name)
        .with_context(|| format!("profile '{}' is missing", profile_name))?;
    let codex_home = profile.codex_home.clone();

    if args.raw {
        let usage =
            fetch_profile_quota_json(&profile.provider, &codex_home, args.base_url.as_deref())?;
        let json = serde_json::to_string_pretty(&usage).context("failed to render usage JSON")?;
        print_stdout_line(&json)?;
        return Ok(());
    }

    if quota_watch_enabled(args) {
        return watch_quota(
            &profile_name,
            &profile.provider,
            &codex_home,
            args.detail,
            args.base_url.as_deref(),
        );
    }

    let quota = fetch_profile_quota(&profile.provider, &codex_home, args.base_url.as_deref())?;
    if let Some(mut terminal) = crate::try_inline_stdout_terminal(12) {
        terminal.draw(|frame| {
            render_profile_quota_once_tui(frame, &profile_name, quota.clone(), args.detail)
        })?;
        let _ = terminal.show_cursor();
    } else {
        print_stdout_text(
            &crate::quota_support::render_profile_quota_snapshot_with_detail(
                &profile_name,
                &quota,
                args.detail,
            ),
        )?;
    }
    save_openai_quota_runtime_usage_snapshot(paths, &state.profiles, &profile_name, &quota);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AppState, ProfileEntry, ProfileProvider, TestEnvVarGuard};
    use std::collections::BTreeMap;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn quota_does_not_repair_or_save_state() {
        let root = std::env::temp_dir().join(format!(
            "prodex-quota-read-only-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        let shared = root.join("shared-codex");
        let _root_guard = TestEnvVarGuard::set("PRODEX_HOME", &root.display().to_string());
        let _shared_guard =
            TestEnvVarGuard::set("PRODEX_SHARED_CODEX_HOME", &shared.display().to_string());
        let paths = AppPaths::discover().expect("paths should resolve");
        fs::create_dir_all(&paths.root).expect("test state parent should be created");
        let state = AppState {
            active_profile: Some("deleted".to_string()),
            profiles: BTreeMap::from([(
                "main".to_string(),
                ProfileEntry {
                    codex_home: root.join("profiles/main"),
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            )]),
            ..AppState::default()
        };
        let original = serde_json::to_string_pretty(&state).expect("state should serialize");
        fs::write(&paths.state_file, &original).expect("state should be written");

        handle_quota(QuotaArgs {
            profile: None,
            all: true,
            auth: None,
            provider: Some("gemini".to_string()),
            detail: false,
            raw: false,
            watch: false,
            once: true,
            base_url: None,
        })
        .expect("filtered quota view should succeed");

        assert_eq!(
            fs::read_to_string(&paths.state_file).expect("state should remain readable"),
            original,
            "quota viewing must not repair or rewrite state"
        );
        assert!(
            !paths.state_file.with_extension("json.lock").exists(),
            "quota viewing must not create the state lock"
        );
        let _ = fs::remove_dir_all(root);
    }
}
