//! `time` owns RFC 3339 text and nothing else, enforcing INV-STDPLUS-ADDS-ONLY:
//! the instant type is `std::time::SystemTime`, unchanged and unwrapped. std
//! already has a point in time; what std has no way to do is write one down as
//! RFC 3339 or read one back. That gap is the whole of this module.
//!
//! An earlier version of this file defined a `Timestamp` newtype holding
//! seconds and nanoseconds. That was a second point-in-time type standing
//! beside `SystemTime`, which is the one thing std+ must never do — it forces
//! every caller to convert at the boundary and it competes with std instead of
//! completing it. The functions below take and return `SystemTime`, so they
//! compose with every existing API that already speaks it.
//!
//! Retires the `chrono` crate, declared in 2 manifests and reached from exactly
//! 2 call sites — both `Utc::now().to_rfc3339()`. No timezone table, calendar
//! arithmetic, locale formatting, or `strptime` appears anywhere in the estate.
//!
//! It also retires a *second* implementation of the same concept:
//! `forge-sdk/forge-core/src/util.rs:71-118` hand-rolls `now_iso()` and
//! `days_to_date()` to avoid the same dependency. Two implementations of one
//! concept is what Law 3 forbids, and that copy carries a defect this one does
//! not — its `unwrap_or_default()` turns a pre-epoch clock into `1970-01-01`
//! rather than surfacing it. See `docs/LGWKS-STD-MIGRATION.md`.
//!
//! ## Retirement
//!
//! Superseded the day std can format a `SystemTime` as RFC 3339. See
//! [`crate::superseded`].
//!
//! The civil-date conversions are Howard Hinnant's `days_from_civil` and
//! `civil_from_days` (public domain, `howardhinnant.github.io/date_algorithms.html`),
//! valid across the whole proleptic Gregorian range. They are transcribed
//! rather than invented; `epoch_day_zero_is_nineteen_seventy` and
//! `civil_roundtrips_across_four_centuries` are the proof.

use std::error::Error;
use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ── Text ────────────────────────────────────────────────────────────────────

/// Renders an instant as canonical RFC 3339 with a `Z` offset.
///
/// Fractional seconds appear only when the remainder is non-zero, and then at
/// full nanosecond precision. This differs from `chrono`'s `to_rfc3339`, which
/// writes a `+00:00` offset and always emits a fraction; both are valid RFC
/// 3339 and each parses the other, but a stored value compared byte-for-byte
/// against a `chrono` string needs a re-stamp. See
/// `docs/LGWKS-STD-MIGRATION.md`.
pub fn to_rfc3339(at: SystemTime) -> String {
    let (secs, nanos) = unix_parts(at);
    let days = secs.div_euclid(86_400);
    let secs_of_day = secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let (hour, minute, second) = (secs_of_day / 3600, (secs_of_day % 3600) / 60, secs_of_day % 60);
    let date = format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}");
    if nanos == 0 {
        format!("{date}Z")
    } else {
        format!("{date}.{nanos:09}Z")
    }
}

/// The current instant in canonical RFC 3339 UTC form. The one-line replacement
/// for `chrono::Utc::now().to_rfc3339()`.
pub fn now_rfc3339() -> String {
    to_rfc3339(SystemTime::now())
}

/// Parses RFC 3339 into a `SystemTime`.
///
/// Accepts `T`/`t`/space as the date-time separator, `Z`/`z` or a numeric
/// `±HH:MM` offset, and one to nine fractional digits. A numeric offset is
/// applied and discarded, because `SystemTime` is an instant and carries no
/// zone — the returned value is the same moment written a different way.
pub fn parse_rfc3339(text: &str) -> Result<SystemTime, ParseError> {
    let b = text.as_bytes();
    if b.len() < 20 {
        return Err(ParseError::TooShort { len: b.len() });
    }
    let year = i64::from(field(b, 0, 4, Field::Year)?);
    expect(b, 4, b'-')?;
    let month = field(b, 5, 2, Field::Month)?;
    expect(b, 7, b'-')?;
    let day = field(b, 8, 2, Field::Day)?;
    match b[10] {
        b'T' | b't' | b' ' => {}
        byte => return Err(ParseError::Malformed { at: 10, byte }),
    }
    let hour = field(b, 11, 2, Field::Hour)?;
    expect(b, 13, b':')?;
    let minute = field(b, 14, 2, Field::Minute)?;
    expect(b, 16, b':')?;
    let second = field(b, 17, 2, Field::Second)?;

    let mut cursor = 19;
    let mut nanos = 0u32;
    if b[cursor] == b'.' {
        cursor += 1;
        let start = cursor;
        while cursor < b.len() && b[cursor].is_ascii_digit() {
            cursor += 1;
        }
        let digits = cursor - start;
        if digits == 0 || digits > 9 {
            return Err(ParseError::FractionWidth { digits });
        }
        let mut scaled = 0u32;
        for i in 0..9 {
            scaled = scaled * 10 + if i < digits { u32::from(b[start + i] - b'0') } else { 0 };
        }
        nanos = scaled;
    }

    let offset_minutes = match b.get(cursor) {
        None => return Err(ParseError::MissingOffset),
        Some(b'Z' | b'z') if cursor + 1 == b.len() => 0,
        Some(sign @ (b'+' | b'-')) => {
            if cursor + 6 != b.len() {
                return Err(ParseError::MissingOffset);
            }
            let oh = field(b, cursor + 1, 2, Field::OffsetHour)?;
            expect(b, cursor + 3, b':')?;
            let om = field(b, cursor + 4, 2, Field::OffsetMinute)?;
            let magnitude = i64::from(oh) * 60 + i64::from(om);
            if *sign == b'-' {
                -magnitude
            } else {
                magnitude
            }
        }
        Some(&byte) => return Err(ParseError::Malformed { at: cursor, byte }),
    };

    range(Field::Month, month, 1, 12)?;
    range(Field::Day, day, 1, days_in_month(year, month))?;
    range(Field::Hour, hour, 0, 23)?;
    range(Field::Minute, minute, 0, 59)?;
    // 60 is admitted so a leap-second stamp parses; it folds into the next
    // minute rather than being rejected as malformed input.
    range(Field::Second, second, 0, 60)?;

    let secs = days_from_civil(year, month, day) * 86_400
        + i64::from(hour) * 3600
        + i64::from(minute) * 60
        + i64::from(second)
        - offset_minutes * 60;
    Ok(from_unix_parts(secs, nanos))
}

// ── SystemTime ⇄ Unix parts ─────────────────────────────────────────────────

/// Splits an instant into whole seconds since the epoch plus a nanosecond
/// remainder in `0..1_000_000_000`.
///
/// A pre-epoch instant is preserved as a negative second count rather than
/// clamped, so a misconfigured host produces a visibly wrong stamp instead of a
/// plausible one.
fn unix_parts(at: SystemTime) -> (i64, u32) {
    match at.duration_since(UNIX_EPOCH) {
        Ok(d) => (d.as_secs() as i64, d.subsec_nanos()),
        Err(before) => {
            let d = before.duration();
            if d.subsec_nanos() == 0 {
                (-(d.as_secs() as i64), 0)
            } else {
                (-(d.as_secs() as i64) - 1, 1_000_000_000 - d.subsec_nanos())
            }
        }
    }
}

fn from_unix_parts(secs: i64, nanos: u32) -> SystemTime {
    if secs >= 0 {
        UNIX_EPOCH + Duration::new(secs as u64, nanos)
    } else if nanos == 0 {
        UNIX_EPOCH - Duration::new(secs.unsigned_abs(), 0)
    } else {
        UNIX_EPOCH - Duration::new(secs.unsigned_abs() - 1, 1_000_000_000 - nanos)
    }
}

// ── Errors ──────────────────────────────────────────────────────────────────

/// Which calendar or clock field failed. Carried by [`ParseError`] so a caller
/// can report the field without parsing the message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// Calendar year.
    Year,
    /// Calendar month, `1..=12`.
    Month,
    /// Day of month.
    Day,
    /// Hour of day, `0..=23`.
    Hour,
    /// Minute of hour, `0..=59`.
    Minute,
    /// Second of minute, `0..=60`.
    Second,
    /// Hours component of a numeric UTC offset.
    OffsetHour,
    /// Minutes component of a numeric UTC offset.
    OffsetMinute,
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Year => "year",
            Self::Month => "month",
            Self::Day => "day",
            Self::Hour => "hour",
            Self::Minute => "minute",
            Self::Second => "second",
            Self::OffsetHour => "offset hour",
            Self::OffsetMinute => "offset minute",
        })
    }
}

/// Why a string is not an RFC 3339 timestamp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// Shorter than the minimum `YYYY-MM-DDTHH:MM:SSZ`.
    TooShort {
        /// Length of the offending input, in bytes.
        len: usize,
    },
    /// An unexpected byte at a fixed position in the grammar.
    Malformed {
        /// Zero-based offset of the offending byte.
        at: usize,
        /// The offending byte, reported verbatim.
        byte: u8,
    },
    /// A field held non-digits where digits were required.
    NotANumber {
        /// The field that failed to parse.
        field: Field,
    },
    /// A field parsed but fell outside its calendar or clock range.
    OutOfRange {
        /// The field that fell outside its range.
        field: Field,
        /// The value that was read.
        value: u32,
    },
    /// Fractional seconds must carry one to nine digits.
    FractionWidth {
        /// Number of fractional digits actually present.
        digits: usize,
    },
    /// RFC 3339 requires `Z` or a numeric `±HH:MM` offset; none was present.
    MissingOffset,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { len } => write!(f, "rfc3339 needs at least 20 characters, got {len}"),
            Self::Malformed { at, byte } => write!(f, "unexpected byte {byte:#04x} at offset {at}"),
            Self::NotANumber { field } => write!(f, "{field} is not a number"),
            Self::OutOfRange { field, value } => write!(f, "{field} {value} is out of range"),
            Self::FractionWidth { digits } => {
                write!(f, "fractional seconds need 1 to 9 digits, got {digits}")
            }
            Self::MissingOffset => f.write_str("missing 'Z' or ±HH:MM offset"),
        }
    }
}

impl Error for ParseError {}

// ── Field readers ───────────────────────────────────────────────────────────

fn expect(b: &[u8], at: usize, want: u8) -> Result<(), ParseError> {
    match b.get(at) {
        Some(&byte) if byte == want => Ok(()),
        Some(&byte) => Err(ParseError::Malformed { at, byte }),
        None => Err(ParseError::TooShort { len: b.len() }),
    }
}

fn field(b: &[u8], at: usize, width: usize, field: Field) -> Result<u32, ParseError> {
    let slice = b.get(at..at + width).ok_or(ParseError::TooShort { len: b.len() })?;
    let mut value = 0u32;
    for &byte in slice {
        if !byte.is_ascii_digit() {
            return Err(ParseError::NotANumber { field });
        }
        value = value * 10 + u32::from(byte - b'0');
    }
    Ok(value)
}

fn range(field: Field, value: u32, lo: u32, hi: u32) -> Result<(), ParseError> {
    if value < lo || value > hi {
        return Err(ParseError::OutOfRange { field, value });
    }
    Ok(())
}

// ── Proleptic Gregorian conversions (Hinnant, public domain) ─────────────────

fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(year) => 29,
        2 => 28,
        // Out-of-range months are caught by the caller's `range` check; return
        // the widest month so the day check cannot mask a month error.
        _ => 31,
    }
}

/// Days since 1970-01-01 for a proleptic Gregorian date.
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = i64::from(if month > 2 { month - 3 } else { month + 9 });
    let doy = (153 * mp + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The proleptic Gregorian date for a count of days since 1970-01-01.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn at(secs: i64) -> SystemTime {
        from_unix_parts(secs, 0)
    }

    #[test]
    fn epoch_day_zero_is_nineteen_seventy() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(days_from_civil(1970, 1, 1), 0);
    }

    #[test]
    fn civil_roundtrips_across_four_centuries() {
        // One full 400-year Gregorian cycle either side of the epoch, stepped
        // by a prime so leap-year boundaries are not systematically skipped.
        let mut day = -146_097;
        while day <= 146_097 {
            let (y, m, d) = civil_from_days(day);
            assert_eq!(days_from_civil(y, m, d), day, "{y:04}-{m:02}-{d:02}");
            day += 7;
        }
    }

    #[test]
    fn the_instant_type_is_std_system_time() {
        // INV-STDPLUS-ADDS-ONLY. If this module ever grows its own point-in-time
        // type again, this stops compiling — which is the intent.
        let now: SystemTime = SystemTime::now();
        let text: String = to_rfc3339(now);
        let back: SystemTime = parse_rfc3339(&text).unwrap();
        assert_eq!(to_rfc3339(back), text);
    }

    #[test]
    fn epoch_renders_without_a_fraction() {
        assert_eq!(to_rfc3339(UNIX_EPOCH), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn a_known_instant_renders_exactly() {
        // Both cross-checked against the system clock library, not against this
        // module: `date -u -r 1787097600` and `date -u -r 1786752000`.
        assert_eq!(to_rfc3339(at(1_787_097_600)), "2026-08-19T00:00:00Z");
        assert_eq!(to_rfc3339(at(1_786_752_000)), "2026-08-15T00:00:00Z");
    }

    #[test]
    fn nanoseconds_render_at_full_precision() {
        let t = parse_rfc3339("2026-08-19T12:00:00.123456789Z").unwrap();
        assert_eq!(unix_parts(t).1, 123_456_789);
        assert_eq!(to_rfc3339(t), "2026-08-19T12:00:00.123456789Z");
    }

    #[test]
    fn short_fractions_are_left_aligned() {
        let t = parse_rfc3339("2026-08-19T12:00:00.5Z").unwrap();
        assert_eq!(unix_parts(t).1, 500_000_000);
    }

    #[test]
    fn parse_reverses_render_for_the_current_instant() {
        let now = SystemTime::now();
        let text = to_rfc3339(now);
        assert_eq!(to_rfc3339(parse_rfc3339(&text).unwrap()), text);
    }

    #[test]
    fn a_numeric_offset_is_folded_into_utc() {
        let as_utc = parse_rfc3339("2026-08-19T10:00:00Z").unwrap();
        assert_eq!(parse_rfc3339("2026-08-19T12:00:00+02:00").unwrap(), as_utc);
        assert_eq!(parse_rfc3339("2026-08-19T08:00:00-02:00").unwrap(), as_utc);
    }

    #[test]
    fn lowercase_separator_and_zulu_are_accepted() {
        assert_eq!(
            parse_rfc3339("2026-08-19t10:00:00z").unwrap(),
            parse_rfc3339("2026-08-19T10:00:00Z").unwrap()
        );
    }

    #[test]
    fn leap_day_parses_only_in_a_leap_year() {
        assert!(parse_rfc3339("2024-02-29T00:00:00Z").is_ok());
        assert_eq!(
            parse_rfc3339("2026-02-29T00:00:00Z"),
            Err(ParseError::OutOfRange { field: Field::Day, value: 29 })
        );
    }

    #[test]
    fn a_missing_offset_is_refused() {
        assert_eq!(parse_rfc3339("2026-08-19T10:00:00"), Err(ParseError::TooShort { len: 19 }));
        assert_eq!(parse_rfc3339("2026-08-19T10:00:00.5"), Err(ParseError::MissingOffset));
    }

    #[test]
    fn an_out_of_range_month_is_refused() {
        assert_eq!(
            parse_rfc3339("2026-13-01T00:00:00Z"),
            Err(ParseError::OutOfRange { field: Field::Month, value: 13 })
        );
    }

    #[test]
    fn a_non_numeric_field_names_itself() {
        assert_eq!(
            parse_rfc3339("2026-0x-01T00:00:00Z"),
            Err(ParseError::NotANumber { field: Field::Month })
        );
    }

    #[test]
    fn an_over_wide_fraction_is_refused() {
        assert_eq!(
            parse_rfc3339("2026-08-19T10:00:00.1234567890Z"),
            Err(ParseError::FractionWidth { digits: 10 })
        );
    }

    #[test]
    fn pre_epoch_instants_are_preserved_not_clamped() {
        let t = parse_rfc3339("1969-12-31T23:59:59Z").unwrap();
        assert_eq!(unix_parts(t), (-1, 0));
        assert_eq!(to_rfc3339(t), "1969-12-31T23:59:59Z");
        assert!(t < UNIX_EPOCH, "a pre-epoch instant must stay before the epoch");
    }

    #[test]
    fn pre_epoch_fractions_round_trip() {
        let t = parse_rfc3339("1969-12-31T23:59:59.250000000Z").unwrap();
        assert_eq!(to_rfc3339(t), "1969-12-31T23:59:59.250000000Z");
    }

    #[test]
    fn now_is_after_the_start_of_twenty_twenty_six() {
        // A clock that fails this is misconfigured, and the stamp would
        // silently poison every receipt written from it.
        assert!(SystemTime::now() > at(1_767_225_600));
    }
}
