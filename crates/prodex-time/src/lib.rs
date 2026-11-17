//! Narrow host date/time compatibility boundary backed entirely by Jiff.
//! The legacy dependency alias preserves callers while removing registry Chrono.
//! Calendar parsing, arithmetic and timezone resolution are owned by Jiff;
//! runtime policies and business decisions remain in their existing owners.
use jiff::{Timestamp, ZonedDateTime, civil, fmt::strtime::BrokenDownTime, tz};
use std::{cmp::Ordering, marker::PhantomData};

#[derive(Debug, Clone, Copy)]
pub struct Local;
#[derive(Debug, Clone, Copy)]
pub struct Utc;
#[derive(Debug, Clone, Copy)]
pub struct FixedOffset(tz::Offset);
#[derive(Debug, Clone)]
pub struct DateTime<Z> {
    value: ZonedDateTime,
    zone: PhantomData<Z>,
}

impl<Z> DateTime<Z> {
    fn new(value: ZonedDateTime) -> Self {
        Self {
            value,
            zone: PhantomData,
        }
    }
    pub fn timestamp(&self) -> i64 {
        self.value
            .timestamp()
            .as_nanosecond()
            .div_euclid(1_000_000_000) as i64
    }
    pub fn timestamp_millis(&self) -> i64 {
        self.value.timestamp().as_nanosecond().div_euclid(1_000_000) as i64
    }
    pub fn timestamp_subsec_nanos(&self) -> u32 {
        self.value
            .timestamp()
            .as_nanosecond()
            .rem_euclid(1_000_000_000) as u32
    }
    pub fn with_timezone<T: TimeZone>(&self, zone: &T) -> DateTime<T> {
        DateTime::new(self.value.timestamp().to_zoned_datetime(zone.zone()))
    }
    pub fn date_naive(&self) -> NaiveDate {
        NaiveDate(self.value.date())
    }
    pub fn to_rfc3339(&self) -> String {
        let nanos = self.timestamp_subsec_nanos();
        let fraction = if nanos == 0 {
            ""
        } else if nanos.is_multiple_of(1_000_000) {
            "%.3f"
        } else if nanos.is_multiple_of(1_000) {
            "%.6f"
        } else {
            "%.9f"
        };
        render(
            &self.value,
            &format!("%Y-%m-%dT%H:%M:%S{fraction}%:z"),
            false,
        )
    }
    pub fn to_rfc3339_opts(&self, _: SecondsFormat, use_z: bool) -> String {
        let zone = if use_z && self.value.offset() == tz::Offset::UTC {
            "Z"
        } else {
            "%:z"
        };
        render(&self.value, &format!("%Y-%m-%dT%H:%M:%S%.3f{zone}"), false)
    }
}
impl<Z: TimeZone> DateTime<Z> {
    pub fn format(&self, pattern: &str) -> String {
        render(&self.value, pattern, Z::IS_UTC)
    }
}
// Match persisted numeric-offset formatting, including historical sub-minute zones.
fn render(value: &ZonedDateTime, pattern: &str, utc: bool) -> String {
    let offset = value.offset().seconds();
    let sign = if offset < 0 { '-' } else { '+' };
    let magnitude = offset.unsigned_abs();
    let minutes = (magnitude + 30) / 60;
    let rounded = format!("{sign}{:02}:{:02}", minutes / 60, minutes % 60);
    let full = if utc {
        "UTC".to_string()
    } else if magnitude.is_multiple_of(60) {
        format!("{sign}{:02}:{:02}", magnitude / 3600, magnitude % 3600 / 60)
    } else {
        format!(
            "{sign}{:02}:{:02}:{:02}",
            magnitude / 3600,
            magnitude % 3600 / 60,
            magnitude % 60
        )
    };
    value
        .strftime(&pattern.replace("%:z", &rounded).replace("%Z", &full))
        .to_string()
}
impl<Z> PartialEq for DateTime<Z> {
    fn eq(&self, other: &Self) -> bool {
        self.value.timestamp() == other.value.timestamp()
    }
}
impl<Z> Eq for DateTime<Z> {}
impl<Z> PartialOrd for DateTime<Z> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl<Z> Ord for DateTime<Z> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value.timestamp().cmp(&other.value.timestamp())
    }
}
impl DateTime<FixedOffset> {
    pub fn parse_from_rfc3339(value: &str) -> Result<Self, jiff::Error> {
        let normalized = value.strip_suffix(['Z', 'z']).map(|v| format!("{v}+00:00"));
        Self::parse_from_str(
            normalized.as_deref().unwrap_or(value),
            "%Y-%m-%dT%H:%M:%S%.f%:z",
        )
    }
    pub fn parse_from_rfc2822(value: &str) -> Result<Self, jiff::Error> {
        jiff::fmt::rfc2822::parse(value).map(Self::new)
    }
    pub fn parse_from_str(value: &str, pattern: &str) -> Result<Self, jiff::Error> {
        BrokenDownTime::parse(pattern, value)?
            .to_zoned_datetime()
            .map(Self::new)
    }
}
impl Local {
    pub fn now() -> DateTime<Self> {
        DateTime::new(ZonedDateTime::now())
    }
}
impl Utc {
    pub fn now() -> DateTime<Self> {
        DateTime::new(Timestamp::now().to_zoned_datetime(tz::TimeZone::UTC))
    }
}

pub struct LocalResult<T> {
    single: Option<T>,
    earliest: Option<T>,
}
impl<T> LocalResult<T> {
    pub fn single(self) -> Option<T> {
        self.single
    }
    pub fn earliest(self) -> Option<T> {
        self.earliest
    }
}
pub trait TimeZone: Sized + Clone {
    const IS_UTC: bool = false;
    fn zone(&self) -> tz::TimeZone;
    fn timestamp_opt(&self, seconds: i64, nanos: u32) -> LocalResult<DateTime<Self>> {
        let value = (nanos < 1_000_000_000)
            .then(|| Timestamp::new(seconds, nanos as i32).ok())
            .flatten()
            .map(|value| DateTime::new(value.to_zoned_datetime(self.zone())));
        LocalResult {
            single: value.clone(),
            earliest: value,
        }
    }
    fn with_ymd_and_hms(
        &self,
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
    ) -> LocalResult<DateTime<Self>> {
        let date = i16::try_from(year)
            .ok()
            .zip(i8::try_from(month).ok())
            .zip(i8::try_from(day).ok())
            .and_then(|((year, month), day)| civil::Date::new(year, month, day).ok());
        let value = date.and_then(|date| NaiveDate(date).and_hms_opt(hour, minute, second));
        value
            .map(|value| self.resolve_local_datetime(&value))
            .unwrap_or(LocalResult {
                single: None,
                earliest: None,
            })
    }
    fn resolve_local_datetime(&self, value: &NaiveDateTime) -> LocalResult<DateTime<Self>> {
        resolve_local(self.zone(), value.0)
    }
}
fn resolve_local<Z>(zone: tz::TimeZone, value: civil::DateTime) -> LocalResult<DateTime<Z>> {
    let ambiguous = zone.to_ambiguous_zoned_datetime(value);
    if matches!(ambiguous.offset(), tz::AmbiguousOffset::Gap { .. }) {
        return LocalResult {
            single: None,
            earliest: None,
        };
    }
    LocalResult {
        single: ambiguous.clone().unambiguous().ok().map(DateTime::new),
        earliest: ambiguous.earlier().ok().map(DateTime::new),
    }
}
impl TimeZone for Local {
    fn zone(&self) -> tz::TimeZone {
        tz::TimeZone::system()
    }
}
impl TimeZone for Utc {
    const IS_UTC: bool = true;
    fn zone(&self) -> tz::TimeZone {
        tz::TimeZone::UTC
    }
}
impl TimeZone for FixedOffset {
    fn zone(&self) -> tz::TimeZone {
        tz::TimeZone::fixed(self.0)
    }
}

pub trait Datelike {
    fn year(&self) -> i32;
    fn month(&self) -> u32;
    fn day(&self) -> u32;
}
impl<Z> Datelike for DateTime<Z> {
    fn year(&self) -> i32 {
        i32::from(self.value.year())
    }
    fn month(&self) -> u32 {
        self.value.month() as u32
    }
    fn day(&self) -> u32 {
        self.value.day() as u32
    }
}
#[derive(Debug, Clone, Copy)]
pub struct NaiveDate(civil::Date);
#[derive(Debug, Clone, Copy)]
pub struct NaiveTime(civil::Time);
#[derive(Debug, Clone, Copy)]
pub struct NaiveDateTime(civil::DateTime);
impl Datelike for NaiveDate {
    fn year(&self) -> i32 {
        i32::from(self.0.year())
    }
    fn month(&self) -> u32 {
        self.0.month() as u32
    }
    fn day(&self) -> u32 {
        self.0.day() as u32
    }
}
impl NaiveDate {
    pub fn parse_from_str(value: &str, pattern: &str) -> Result<Self, jiff::Error> {
        BrokenDownTime::parse(pattern, value)?.to_date().map(Self)
    }
    pub fn and_hms_opt(self, hour: u32, minute: u32, second: u32) -> Option<NaiveDateTime> {
        let time = civil::Time::new(
            i8::try_from(hour).ok()?,
            i8::try_from(minute).ok()?,
            i8::try_from(second).ok()?,
            0,
        )
        .ok()?;
        Some(NaiveDateTime(self.0.to_datetime(time)))
    }
    pub fn and_time(self, time: NaiveTime) -> NaiveDateTime {
        NaiveDateTime(self.0.to_datetime(time.0))
    }
    pub fn pred_opt(self) -> Option<Self> {
        self.0.yesterday().ok().map(Self)
    }
    pub fn format(&self, pattern: &str) -> String {
        self.0.strftime(pattern).to_string()
    }
}
impl NaiveTime {
    pub fn parse_from_str(value: &str, pattern: &str) -> Result<Self, jiff::Error> {
        BrokenDownTime::parse(pattern, value)?.to_time().map(Self)
    }
}
impl NaiveDateTime {
    pub fn parse_from_str(value: &str, pattern: &str) -> Result<Self, jiff::Error> {
        BrokenDownTime::parse(pattern, value)?
            .to_datetime()
            .map(Self)
    }
    pub fn checked_add_signed(self, duration: Duration) -> Option<Self> {
        self.0
            .checked_add(jiff::SignedDuration::from_secs(duration.0))
            .ok()
            .map(Self)
    }
}
pub struct Duration(i64);
impl Duration {
    pub fn days(days: i64) -> Self {
        Self(
            days.checked_mul(86_400)
                .expect("duration day count overflow"),
        )
    }
}
pub enum SecondsFormat {
    Millis,
}
#[cfg(test)]
mod tests;
