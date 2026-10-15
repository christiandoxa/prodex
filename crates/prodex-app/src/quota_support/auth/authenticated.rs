use super::*;

impl UsageFetchFlow<'_> {
    // Return the identity actually accepted by the final HTTP request, not a
    // later auth.json read that may belong to another concurrent refresh.
    pub(in crate::quota_support) fn execute_with_authenticated_identity(
        mut self,
    ) -> Result<(serde_json::Value, UsageAuth)> {
        let (status, body) = self.send_with_unauthorized_retry()?;
        self.ensure_success(status, &body)?;
        let value = serde_json::from_slice(&body).with_context(|| {
            format!(
                "invalid JSON returned by quota backend for {}",
                self.codex_home.display()
            )
        })?;
        Ok((value, self.auth))
    }
}
