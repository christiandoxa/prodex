use super::{UsageAuth, UsageFetchFlow, send_usage_request};
use anyhow::{Context, Result};
use prodex_quota::UsageResponse;

impl UsageFetchFlow<'_> {
    // Return the identity actually accepted by the final HTTP request, not a
    // later auth.json read that may belong to another concurrent refresh.
    pub(in crate::quota_support) fn execute_with_authenticated_identity(
        mut self,
    ) -> Result<(serde_json::Value, UsageAuth)> {
        let value = self.fetch_value()?;
        Ok((value, self.auth))
    }

    pub(in crate::quota_support) fn execute_for_display(mut self) -> Result<UsageResponse> {
        let mut usage: UsageResponse =
            serde_json::from_value(self.fetch_value()?).context("invalid quota response")?;
        // Keep the accepted usage identity and configured transport, even if
        // auth.json changes while the optional detail request is in flight.
        if let Some(credits) = usage
            .rate_limit_reset_credits
            .as_mut()
            .filter(|credits| credits.available_count > 0)
            && let Ok((status, body)) = send_usage_request(
                &self.client,
                self.codex_home,
                &self.reset_credits_url,
                &self.auth,
            )
            && status.is_success()
        {
            // Missing details must never discard a successful usage snapshot.
            let _ = credits.update_next_expiry_from_details(&body);
        }
        Ok(usage)
    }

    fn fetch_value(&mut self) -> Result<serde_json::Value> {
        let (status, body) = self.send_with_unauthorized_retry()?;
        self.ensure_success(status, &body)?;
        serde_json::from_slice(&body).with_context(|| {
            format!(
                "invalid JSON returned by quota backend for {}",
                self.codex_home.display()
            )
        })
    }
}
