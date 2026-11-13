//! `{% faker 'function' %}`: generated values.
//!
//! The original exposes 117 functions from the `@faker-js/faker` object
//! (`packages/insomnia-data/common-src/faker-functions.ts`). Heric implements
//! the ones a request template plausibly uses — identifiers, timestamps,
//! booleans, integers and colours — and reports the rest as unavailable rather
//! than making up a value.
//!
//! Randomness comes from [`uuid::Uuid::new_v4`], which is already in the tree.
//! A faker output is not a secret; what matters is that two calls do not return
//! the same string, and four v4 UUIDs per call give more entropy than any of
//! these generators needs.

use serde_json::Value;

use crate::tag::TagError;
use crate::truthy::to_output;

/// Runs the tag.
///
/// # Errors
///
/// Returns [`TagError::BadArguments`] for a function that is not implemented,
/// naming the ones that are.
pub(super) fn run(args: &[Value]) -> Result<Value, TagError> {
    let Some(function) = args.first().map(to_output) else {
        return Err(TagError::BadArguments {
            tag: "faker",
            detail: "needs a function name".to_owned(),
        });
    };
    match function.as_str() {
        "guid" | "randomUUID" => Ok(Value::String(uuid::Uuid::new_v4().to_string())),
        "timestamp" => Ok(Value::from(unix_seconds())),
        "isoTimestamp" => Ok(Value::String(iso_8601_now())),
        "randomBoolean" => Ok(Value::Bool(source().next_byte().is_multiple_of(2))),
        "randomInt" => Ok(Value::from(i64::from(source().next_byte()))),
        "randomAlphaNumeric" => Ok(Value::String({
            let mut text = String::new();
            for _ in 0..16 {
                text.push(
                    u32::from(source().next_byte() % 36)
                        .to_string()
                        .chars()
                        .next()
                        .unwrap_or('0'),
                );
            }
            text
        })),
        "randomHexColor" => Ok(Value::String(format!(
            "#{:06x}",
            source().next_u32() & 0x00ff_ffff
        ))),
        "randomColor" => Ok(Value::String({
            const NAMES: [&str; 8] = [
                "red", "green", "blue", "yellow", "purple", "orange", "pink", "black",
            ];
            NAMES[usize::from(source().next_byte()) % NAMES.len()].to_owned()
        })),
        other => Err(TagError::BadArguments {
            tag: "faker",
            detail: format!(
                "`{other}` is not implemented; try guid, timestamp, isoTimestamp, randomUUID, randomBoolean, randomInt, randomAlphaNumeric, randomColor or randomHexColor"
            ),
        }),
    }
}

/// The current time in whole seconds since the epoch.
fn unix_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX)
        })
}

/// The current time as an ISO-8601 string.
fn iso_8601_now() -> String {
    super::time::now(&[Value::String("iso-8601".to_owned())])
        .map(|value| to_output(&value))
        .unwrap_or_default()
}

/// A stream of pseudo-random bytes.
///
/// Built on [`uuid::Uuid::new_v4`] so no random crate enters the tree. Each
/// call draws a fresh UUID when the current one is exhausted, sixteen bytes at
/// a time.
struct Source {
    buffer: [u8; 16],
    position: usize,
}

impl Source {
    /// A source seeded from one UUID.
    fn new() -> Self {
        Self {
            buffer: *uuid::Uuid::new_v4().as_bytes(),
            position: 0,
        }
    }

    /// The next byte, drawing a new UUID when the buffer runs out.
    fn next_byte(&mut self) -> u8 {
        if self.position >= self.buffer.len() {
            self.buffer = *uuid::Uuid::new_v4().as_bytes();
            self.position = 0;
        }
        let byte = self.buffer.get(self.position).copied().unwrap_or(0);
        self.position = self.position.saturating_add(1);
        byte
    }

    /// The next four bytes as an integer.
    fn next_u32(&mut self) -> u32 {
        let bytes = [
            self.next_byte(),
            self.next_byte(),
            self.next_byte(),
            self.next_byte(),
        ];
        u32::from_be_bytes(bytes)
    }
}

/// A fresh source, so `faker::run` reads as one expression per call.
fn source() -> Source {
    Source::new()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn run_args(function: &str) -> Result<String, TagError> {
        run(&[json!(function)]).map(|value| to_output(&value))
    }

    #[test]
    fn a_guid_is_a_uuid() -> Result<(), TagError> {
        let guid = run_args("guid")?;
        assert_eq!(guid.len(), 36);
        assert_eq!(guid.as_bytes().get(14), Some(&b'4'));
        Ok(())
    }

    #[test]
    fn two_calls_do_not_agree() -> Result<(), TagError> {
        assert_ne!(run_args("guid")?, run_args("guid")?);
        assert_ne!(
            run_args("randomAlphaNumeric")?,
            run_args("randomAlphaNumeric")?
        );
        Ok(())
    }

    #[test]
    fn a_timestamp_is_a_number() -> Result<(), Box<dyn std::error::Error>> {
        let timestamp = run_args("timestamp")?;
        let seconds: i64 = timestamp.parse()?;
        assert!(seconds > 1_600_000_000, "{seconds}");
        Ok(())
    }

    #[test]
    fn an_iso_timestamp_ends_in_z() -> Result<(), TagError> {
        let text = run_args("isoTimestamp")?;
        assert!(text.ends_with('Z'), "{text}");
        assert_eq!(text.len(), 24, "{text}");
        Ok(())
    }

    #[test]
    fn a_random_boolean_is_true_or_false() -> Result<(), TagError> {
        for _ in 0..8 {
            let value = run_args("randomBoolean")?;
            assert!(value == "true" || value == "false", "{value}");
        }
        Ok(())
    }

    #[test]
    fn a_hex_colour_has_six_digits() -> Result<(), TagError> {
        let colour = run_args("randomHexColor")?;
        assert_eq!(colour.len(), 7, "{colour}");
        assert!(colour.starts_with('#'), "{colour}");
        assert!(
            colour[1..].chars().all(|c| c.is_ascii_hexdigit()),
            "{colour}"
        );
        Ok(())
    }

    #[test]
    fn an_unknown_function_is_an_error() {
        assert!(run_args("randomNonsense").is_err());
    }
}
