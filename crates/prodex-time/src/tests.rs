use super::{
    DateTime, Datelike, Local, NaiveDate, NaiveDateTime, NaiveTime, SecondsFormat, TimeZone, Utc,
    resolve_local,
};
use jiff::{civil, tz};

#[test]
fn timestamp_roundtrips_keep_offset_fraction_and_negative_epoch() {
    for (input, seconds, nanos, rendered) in [
        (
            "2026-11-18T06:00:00+07:00",
            1794956400,
            0,
            "2026-11-18T06:00:00+07:00",
        ),
        (
            "1970-01-01T00:00:00.123456Z",
            0,
            123_456_000,
            "1970-01-01T00:00:00.123456+00:00",
        ),
        (
            "1969-12-31T23:59:59.999Z",
            -1,
            999_000_000,
            "1969-12-31T23:59:59.999+00:00",
        ),
    ] {
        let value = DateTime::parse_from_rfc3339(input).unwrap();
        assert_eq!(value.timestamp(), seconds, "{input}");
        assert_eq!(value.timestamp_subsec_nanos(), nanos);
        assert_eq!(value.to_rfc3339(), rendered);
        assert_eq!(
            value.timestamp_millis(),
            seconds * 1000 + i64::from(nanos / 1_000_000)
        );
    }
}
#[test]
fn persistent_formats_and_calendar_boundaries_are_stable() {
    let value = Utc.timestamp_opt(0, 123_456_789).single().unwrap();
    assert_eq!(
        value.format("%Y-%m-%d %H:%M:%S%.3f %:z"),
        "1970-01-01 00:00:00.123 +00:00"
    );
    assert_eq!(
        value.format("%Y-%m-%d %H:%M:%S %Z"),
        "1970-01-01 00:00:00 UTC"
    );
    assert_eq!(
        value.to_rfc3339_opts(SecondsFormat::Millis, true),
        "1970-01-01T00:00:00.123Z"
    );
    let today = NaiveDate::parse_from_str("2024-03-01", "%Y-%m-%d").unwrap();
    assert_eq!(today.pred_opt().unwrap().format("%Y-%m-%d"), "2024-02-29");
    assert_eq!((today.year(), today.month(), today.day()), (2024, 3, 1));
    assert_eq!(today.to_string(), "2024-03-01");
    let clock = NaiveTime::parse_from_str("11:45 PM", "%I:%M %p").unwrap();
    let next = today
        .and_time(clock)
        .checked_add_signed(super::Duration::days(1))
        .unwrap();
    assert_eq!(
        Utc.resolve_local_datetime(&next)
            .single()
            .unwrap()
            .to_rfc3339(),
        "2024-03-02T23:45:00+00:00"
    );
    assert_eq!(
        Utc.with_ymd_and_hms(2024, 3, 1, 0, 0, 0)
            .single()
            .unwrap()
            .date_naive()
            .format("%Y-%m-%d"),
        "2024-03-01"
    );
    assert!(
        Utc.with_ymd_and_hms(2023, 2, 29, 0, 0, 0)
            .single()
            .is_none()
    );
}
#[test]
fn invalid_unbounded_and_ambiguous_times_fail_closed() {
    for invalid in [
        "",
        "garbage",
        "2026-02-30T00:00:00Z",
        "2026-01-01T25:00:00Z",
        "2026-01-01T00:00:00",
    ] {
        assert!(DateTime::parse_from_rfc3339(invalid).is_err());
    }
    assert!(Utc.timestamp_opt(i64::MAX, 0).single().is_none());
    assert!(Utc.timestamp_opt(0, 1_000_000_000).single().is_none());
    let zone = tz::TimeZone::get("America/New_York").unwrap();
    assert!(
        resolve_local::<Local>(zone.clone(), civil::datetime(2026, 3, 8, 2, 30, 0, 0))
            .earliest()
            .is_none()
    );
    let fold = civil::datetime(2026, 11, 1, 1, 30, 0, 0);
    assert!(
        resolve_local::<Local>(zone.clone(), fold)
            .single()
            .is_none()
    );
    assert_eq!(
        resolve_local::<Local>(zone, fold)
            .earliest()
            .unwrap()
            .to_rfc3339(),
        "2026-11-01T01:30:00-04:00"
    );
}
#[test]
fn rfc2822_logs_and_retry_dates_keep_their_parsing_contract() {
    assert_eq!(
        DateTime::parse_from_rfc2822("Thu, 01 Jan 1970 00:00:00 GMT")
            .unwrap()
            .timestamp(),
        0
    );
    assert!(DateTime::parse_from_rfc2822("not a cookie expiry").is_err());
    let value =
        DateTime::parse_from_str("2026-11-18 06:00:00.125 +07:00", "%Y-%m-%d %H:%M:%S%.f %:z")
            .unwrap();
    assert_eq!(
        value.format("%Y-%m-%d %H:%M:%S%.3f %:z"),
        "2026-11-18 06:00:00.125 +07:00"
    );
    let retry =
        NaiveDateTime::parse_from_str("Nov 18, 2026 11:00 PM", "%b %d, %Y %I:%M %p").unwrap();
    assert_eq!(
        Utc.resolve_local_datetime(&retry)
            .single()
            .unwrap()
            .to_rfc3339(),
        "2026-11-18T23:00:00+00:00"
    );
    assert!(
        DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z").unwrap()
            < DateTime::parse_from_rfc3339("2026-01-01T00:00:01Z").unwrap()
    );
    let before = Utc::now().timestamp();
    let local = Local::now().with_timezone(&Utc).timestamp();
    assert!((before..=Utc::now().timestamp()).contains(&local));
}

#[test]
fn unix_constructor_uses_floor_seconds_with_positive_fraction() {
    for (seconds, nanos, expected) in [
        (-1, 999_000_000, "1969-12-31T23:59:59.999+00:00"),
        (-1, 1, "1969-12-31T23:59:59.000000001+00:00"),
        (0, 1, "1970-01-01T00:00:00.000000001+00:00"),
        (1, 999_000_000, "1970-01-01T00:00:01.999+00:00"),
    ] {
        let value = Utc.timestamp_opt(seconds, nanos).single().unwrap();
        assert_eq!(value.to_rfc3339(), expected);
        assert_eq!(value.timestamp(), seconds);
        assert_eq!(value.timestamp_subsec_nanos(), nanos);
    }
}
