use anyhow::{Context, Result, bail};

use crate::{
    AppPaths, AppState, AppStateIoExt, ProfileProvider, RateLimitResetCreditConsumeFlow,
    RateLimitResetCreditConsumeOutcome, RedeemArgs, print_stdout_line,
    repair_missing_active_profile_and_save,
};

pub(crate) fn handle_redeem(args: RedeemArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let mut state = AppState::load_and_repair(&paths)?;
    repair_missing_active_profile_and_save(&paths, &mut state)?;
    let profile = state
        .profiles
        .get(&args.profile)
        .with_context(|| format!("profile '{}' is missing", args.profile))?;
    if !matches!(profile.provider, ProfileProvider::Openai) {
        bail!(
            "profile '{}' is not an OpenAI/Codex profile and cannot redeem reset credits",
            args.profile
        );
    }

    let redeem_request_id = manual_redeem_request_id();
    let response = RateLimitResetCreditConsumeFlow::new_with_proxy_policy(
        &profile.codex_home,
        args.base_url.as_deref(),
        args.no_proxy,
    )?
    .execute(&redeem_request_id)?;

    print_stdout_line(&format!(
        "profile={} outcome={} request_id={}",
        args.profile,
        manual_redeem_outcome_label(response.outcome),
        redeem_request_id
    ));
    Ok(())
}

fn manual_redeem_request_id() -> String {
    let now_nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("prodex-manual-redeem-{}-{now_nanos}", std::process::id())
}

fn manual_redeem_outcome_label(outcome: RateLimitResetCreditConsumeOutcome) -> &'static str {
    match outcome {
        RateLimitResetCreditConsumeOutcome::Reset => "reset",
        RateLimitResetCreditConsumeOutcome::NothingToReset => "nothing-to-reset",
        RateLimitResetCreditConsumeOutcome::NoCredit => "no-credit",
        RateLimitResetCreditConsumeOutcome::AlreadyRedeemed => "already-redeemed",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_redeem_request_id_is_prodex_scoped() {
        assert!(manual_redeem_request_id().starts_with("prodex-manual-redeem-"));
    }

    #[test]
    fn manual_redeem_outcome_labels_are_human_readable() {
        assert_eq!(
            manual_redeem_outcome_label(RateLimitResetCreditConsumeOutcome::Reset),
            "reset"
        );
        assert_eq!(
            manual_redeem_outcome_label(RateLimitResetCreditConsumeOutcome::NothingToReset),
            "nothing-to-reset"
        );
        assert_eq!(
            manual_redeem_outcome_label(RateLimitResetCreditConsumeOutcome::NoCredit),
            "no-credit"
        );
        assert_eq!(
            manual_redeem_outcome_label(RateLimitResetCreditConsumeOutcome::AlreadyRedeemed),
            "already-redeemed"
        );
    }
}
