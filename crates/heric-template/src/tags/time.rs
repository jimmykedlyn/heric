//! `{% now 'format' %}` and `{% timestamp %}`.
//!
//! `now` takes `iso-8601`, `millis`/`ms`, `unix`/`seconds`/`s` or `custom`
//! (`common/templating/local-template-tags.ts:94-146`). The first three are
//! computed here from the system clock; the calendar conversion is the
//! days-from-civil algorithm rather than a date crate, because no date crate is
//! on the plan's allowed-new list.
//!
//! `custom` is not implemented. Its placeholder format is date-fns' `format`
//! (`MMMM Do YYYY, h:mm:ss a`), a token language with dozens of specifiers and
//! locale-dependent output; a partial port would silently print the wrong month
//! in some templates, so the tag names the gap instead.

use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

use crate::tag::TagError;
use crate::truthy::to_output;

/// The tag's default format, which is what an empty argument means.
const DEFAULT_FORMAT: &str = "iso-8601";

/// The current time, in milliseconds since the Unix epoch.
///
/// A clock before 1970 is reported as zero rather than as a negative time: no
/// template has a meaningful use for one, and `SystemTime`'s signed
/// representation is not portable across targets.
fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}

/// Runs `{% now %}`.
///
/// # Errors
///
/// Returns [`TagError::BadArguments`] for an unknown format, and
/// [`TagError::Unsupported`] for `custom`.
pub(super) fn now(args: &[Value]) -> Result<Value, TagError> {
    let format = args
        .first()
        .map(to_output)
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| DEFAULT_FORMAT.to_owned());
    let millis = now_millis();
    match format.as_str() {
        "iso-8601" => Ok(Value::String(iso_8601(millis))),
        "millis" | "ms" => Ok(Value::String(millis.to_string())),
        "unix" | "seconds" | "s" => Ok(Value::String((millis / 1_000).to_string())),
        "custom" => Err(TagError::Unsupported {
            tag: "now",
            detail:
                "the `custom` format needs a date-format crate, which is a dependency approval item"
                    .to_owned(),
        }),
        other => Err(TagError::BadArguments {
            tag: "now",
            detail: format!("`{other}` is not a date type; use iso-8601, millis, unix or custom"),
        }),
    }
}

/// Runs `{% timestamp %}`.
///
/// The tag is a Nunjucks built-in rather than one of the original's local tags;
/// it takes an optional unit and defaults to seconds.
///
/// # Errors
///
/// Returns [`TagError::BadArguments`] for an unknown unit.
pub(super) fn timestamp(args: &[Value]) -> Result<Value, TagError> {
    let unit = args
        .first()
        .map(to_output)
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| "seconds".to_owned());
    let millis = now_millis();
    match unit.as_str() {
        "seconds" | "s" | "unix" => Ok(Value::from(millis / 1_000)),
        "millis" | "ms" => Ok(Value::from(millis)),
        other => Err(TagError::BadArguments {
            tag: "timestamp",
            detail: format!("`{other}` is not a unit; use seconds or millis"),
        }),
    }
}

/// Formats a moment as ISO-8601 in UTC.
///
/// The shape is `YYYY-MM-DDTHH:MM:SS.mmmZ`, which is what JavaScript's
/// `new Date().toISOString()` produces — and that is what the original returns,
/// because its `iso-8601` branch calls `toISOString()`.
fn iso_8601(millis: i64) -> String {
    let (days, rest) = (millis.div_euclid(86_400_000), millis.rem_euclid(86_400_000));
    let (year, month, day) = civil_from_days(days);
    let hour = rest / 3_600_000;
    let minute = rest % 3_600_000 / 60_000;
    let second = rest % 60_000 / 1_000;
    let milli = rest % 1_000;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{milli:03}Z")
}

/// Converts a day count since 1970-01-01 into a calendar date.
///
/// Howard Hinnant's `civil_from_days`: shift the epoch to 0000-03-01 so leap days
/// fall at the end of the year, which makes the month lengths regular enough for
/// integer arithmetic.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_offset = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_offset + 2) / 5 + 1;
    let month = if month_offset < 10 {
        month_offset + 3
    } else {
        month_offset - 9
    };
    let year = if month <= 2 { year + 1 } else { year };
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn now_args(format: &str) -> Result<String, TagError> {
        now(&[json!(format)]).map(|value| to_output(&value))
    }

    #[test]
    fn the_epoch_formats_correctly() {
        assert_eq!(iso_8601(0), "1970-01-01T00:00:00.000Z");
    }

    #[test]
    fn a_known_moment_formats_correctly() {
        // 2024-02-29T12:34:56.789Z.
        assert_eq!(iso_8601(1_709_210_096_789), "2024-02-29T12:34:56.789Z");
        // 1999-12-31T23:59:59.999Z, which is the end of a non-leap year.
        assert_eq!(iso_8601(946_684_799_999), "1999-12-31T23:59:59.999Z");
    }

    #[test]
    fn a_pre_epoch_moment_still_has_a_date() {
        assert_eq!(iso_8601(-86_400_000), "1969-12-31T00:00:00.000Z");
    }

    #[test]
    fn the_default_format_is_iso_8601() -> Result<(), TagError> {
        let text = now(&[]).map(|value| to_output(&value))?;
        assert!(text.ends_with('Z'), "{text}");
        assert_eq!(text.len(), 24, "{text}");
        Ok(())
    }

    #[test]
    fn the_millis_and_unix_aliases_agree() -> Result<(), Box<dyn std::error::Error>> {
        let millis: i64 = now_args("millis")?.parse()?;
        let seconds: i64 = now_args("unix")?.parse()?;
        assert_eq!(millis / 1_000, seconds);
        assert_eq!(now_args("ms")?.len(), now_args("millis")?.len());
        Ok(())
    }

    #[test]
    fn an_unknown_format_is_an_error() {
        assert!(now_args("nope").is_err());
    }

    #[test]
    fn the_custom_format_names_what_it_needs() {
        assert!(matches!(
            now_args("custom"),
            Err(TagError::Unsupported { tag: "now", .. })
        ));
    }

    #[test]
    fn a_timestamp_defaults_to_seconds() -> Result<(), Box<dyn std::error::Error>> {
        let seconds = timestamp(&[])?;
        let millis = timestamp(&[json!("millis")])?;
        let (Some(seconds), Some(millis)) = (seconds.as_i64(), millis.as_i64()) else {
            return Err("a timestamp is not an integer".into());
        };
        assert_eq!(millis / 1_000, seconds);
        Ok(())
    }

    #[test]
    fn an_unknown_timestamp_unit_is_an_error() {
        assert!(timestamp(&[json!("nope")]).is_err());
    }
}
