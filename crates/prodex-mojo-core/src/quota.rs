#[cfg(not(prodex_mojo_fallback))]
unsafe extern "C" {
    fn prodex_quota_remaining_percent(used_percent: i64, has_value: i64) -> i64;
    fn prodex_quota_window_status(remaining_percent: i64, has_window: i64) -> i64;
    fn prodex_quota_pressure_band(five_hour_status: i64, weekly_status: i64) -> i64;
    fn prodex_quota_window_pair_has_ready_limit(
        first_used_percent: i64,
        first_has_value: i64,
        second_used_percent: i64,
        second_has_value: i64,
    ) -> i64;
}

pub fn remaining_percent(used_percent: Option<i64>) -> i64 {
    #[cfg(not(prodex_mojo_fallback))]
    {
        unsafe {
            prodex_quota_remaining_percent(
                used_percent.unwrap_or(0),
                i64::from(used_percent.is_some()),
            )
        }
    }
    #[cfg(prodex_mojo_fallback)]
    {
        match used_percent {
            None => 0,
            Some(value) if value < 0 => 100,
            Some(value) if value > 100 => 0,
            Some(value) => 100 - value,
        }
    }
}

pub fn window_status(remaining_percent: i64, has_window: bool) -> i64 {
    #[cfg(not(prodex_mojo_fallback))]
    {
        unsafe { prodex_quota_window_status(remaining_percent, i64::from(has_window)) }
    }
    #[cfg(prodex_mojo_fallback)]
    {
        if !has_window {
            4
        } else if remaining_percent == 0 {
            3
        } else if remaining_percent <= 5 {
            2
        } else if remaining_percent <= 15 {
            1
        } else {
            0
        }
    }
}

pub fn pressure_band(five_hour_status: i64, weekly_status: i64) -> i64 {
    #[cfg(not(prodex_mojo_fallback))]
    {
        unsafe { prodex_quota_pressure_band(five_hour_status, weekly_status) }
    }
    #[cfg(prodex_mojo_fallback)]
    {
        fn band(status: i64) -> i64 {
            match status {
                0 => 0,
                1 => 1,
                2 => 2,
                3 => 3,
                _ => 4,
            }
        }
        band(five_hour_status).max(band(weekly_status))
    }
}

pub fn window_pair_has_ready_limit(first: Option<i64>, second: Option<i64>) -> bool {
    #[cfg(not(prodex_mojo_fallback))]
    {
        unsafe {
            prodex_quota_window_pair_has_ready_limit(
                first.unwrap_or(0),
                i64::from(first.is_some()),
                second.unwrap_or(0),
                i64::from(second.is_some()),
            ) != 0
        }
    }
    #[cfg(prodex_mojo_fallback)]
    {
        let values = [first, second].into_iter().flatten().collect::<Vec<_>>();
        !values.is_empty() && values.into_iter().all(|value| value < 100)
    }
}
