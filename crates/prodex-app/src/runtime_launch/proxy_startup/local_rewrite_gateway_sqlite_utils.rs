pub(super) fn runtime_gateway_sqlite_optional_i64_to_u64(value: Option<i64>) -> Option<u64> {
    value.map(runtime_gateway_sqlite_i64_to_u64)
}

pub(super) fn runtime_gateway_sqlite_i64_to_u64(value: i64) -> u64 {
    u64::try_from(value).unwrap_or_default()
}

pub(super) fn runtime_gateway_sqlite_optional_u64_to_i64(value: Option<u64>) -> Option<i64> {
    value.map(runtime_gateway_sqlite_u64_to_i64)
}

pub(super) fn runtime_gateway_sqlite_u64_to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_conversions_are_saturating_or_loss_tolerant() {
        assert_eq!(runtime_gateway_sqlite_i64_to_u64(-1), 0);
        assert_eq!(runtime_gateway_sqlite_i64_to_u64(42), 42);
        assert_eq!(runtime_gateway_sqlite_u64_to_i64(u64::MAX), i64::MAX);
        assert_eq!(
            runtime_gateway_sqlite_optional_i64_to_u64(Some(-1)),
            Some(0)
        );
        assert_eq!(runtime_gateway_sqlite_optional_u64_to_i64(None), None);
    }
}
