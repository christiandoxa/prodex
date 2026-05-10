use std::str::FromStr;

use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CleanupOlderThan {
    seconds: i64,
}

impl CleanupOlderThan {
    pub fn seconds(self) -> i64 {
        self.seconds
    }
}

impl FromStr for CleanupOlderThan {
    type Err = String;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        let value = value.trim();
        if value.is_empty() {
            return Err("expected an age such as 0d, 1d, 12h, 30m, or 60s".to_string());
        }
        let Some(unit) = value.chars().last() else {
            return Err("expected an age such as 0d, 1d, 12h, 30m, or 60s".to_string());
        };
        let number = &value[..value.len() - unit.len_utf8()];
        if number.is_empty() || !number.chars().all(|ch| ch.is_ascii_digit()) {
            return Err("expected a non-negative age such as 0d, 1d, 12h, 30m, or 60s".to_string());
        }
        let amount = number
            .parse::<i64>()
            .map_err(|_| "age is too large".to_string())?;
        let multiplier = match unit {
            's' => 1,
            'm' => 60,
            'h' => 60 * 60,
            'd' => 24 * 60 * 60,
            _ => return Err("age unit must be one of s, m, h, or d".to_string()),
        };
        let seconds = amount
            .checked_mul(multiplier)
            .ok_or_else(|| "age is too large".to_string())?;
        Ok(Self { seconds })
    }
}

#[derive(Args, Debug, Clone, Copy, Default)]
pub struct CleanupArgs {
    /// Remove orphaned managed profile homes immediately. Equivalent to --older-than 0d.
    #[arg(long, conflicts_with = "older_than")]
    pub aggressive: bool,
    /// Override the orphaned managed profile home age threshold, e.g. 0d, 1d, 7d.
    #[arg(long, value_name = "AGE", conflicts_with = "aggressive")]
    pub older_than: Option<CleanupOlderThan>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ListProfilesCommand;
