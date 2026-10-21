use super::RateLimitResetCreditsSummary;
use prodex_mojo_core::quota::{QuotaResetCreditInput, quota_reset_credit_expiry};
use serde::Deserialize;

pub use prodex_mojo_core::quota::QuotaResetCreditExpiry as RateLimitResetCreditExpiry;

#[derive(Deserialize)]
struct ResetCreditDetails {
    credits: Vec<ResetCredit>,
}

#[derive(Deserialize)]
struct ResetCredit {
    status: String,
    #[serde(default, deserialize_with = "deserialize_expiry")]
    expires_at: RateLimitResetCreditExpiry,
}

// Chrono owns RFC3339 parsing at the Rust wire boundary; Mojo receives typed
// instants and owns status filtering, incomplete-detail policy and selection.
fn deserialize_expiry<'de, D>(deserializer: D) -> Result<RateLimitResetCreditExpiry, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(match value {
        serde_json::Value::Null => RateLimitResetCreditExpiry::Never,
        serde_json::Value::String(value) => chrono::DateTime::parse_from_rfc3339(&value)
            .map(|value| RateLimitResetCreditExpiry::At(value.timestamp()))
            .unwrap_or_default(),
        _ => RateLimitResetCreditExpiry::Unavailable,
    })
}

impl RateLimitResetCreditsSummary {
    pub fn update_next_expiry_from_details(&mut self, body: &[u8]) -> anyhow::Result<()> {
        self.next_expiry = RateLimitResetCreditExpiry::Unavailable;
        let details: ResetCreditDetails = serde_json::from_slice(body)?;
        let credits = details
            .credits
            .iter()
            .map(|credit| QuotaResetCreditInput {
                status: &credit.status,
                expiry: credit.expires_at,
            })
            .collect::<Vec<_>>();
        self.next_expiry = quota_reset_credit_expiry(self.available_count, &credits)
            .map_err(|error| anyhow::anyhow!("invalid reset-credit expiry output: {error:?}"))?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "../../tests/src/models/reset_credits.rs"]
mod tests;
