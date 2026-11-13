//! `{% base64 'encode'|'decode', 'normal'|'url'|'hex', value %}`.
//!
//! Transcribed from `common/templating/local-template-tags.ts:34-92`. The
//! original builds the encoding on `btoa`/`atob`, which work on a binary string,
//! so a UTF-8 value is first widened byte by byte; the `hex` kind instead reads
//! the value as hex pairs. `url` is `normal` with the URL-safe alphabet and no
//! padding.
//!
//! No base64 crate is on the plan's allowed-new list, so the codec is written
//! here. It is the same decision Task 16 takes for the same reason.

use serde_json::Value;

use crate::tag::TagError;
use crate::truthy::to_output;

/// The alphabet a `normal` encoding uses.
const STANDARD: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// The alphabet a `url` encoding uses.
const URL_SAFE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Runs the tag.
///
/// # Errors
///
/// Returns [`TagError::BadArguments`] for an unknown action or kind, and for a
/// value that is not valid base64 or hex when decoding.
pub(super) fn run(args: &[Value]) -> Result<Value, TagError> {
    let action = args.first().map(to_output).unwrap_or_default();
    let kind = args.get(1).map(to_output).unwrap_or_default();
    let value = args.get(2).map(to_output).unwrap_or_default();
    let kind = Kind::parse(&kind)?;
    match action.as_str() {
        "encode" => Ok(Value::String(encode(&value, kind))),
        "decode" => decode(&value, kind).map(Value::String),
        other => Err(TagError::BadArguments {
            tag: "base64",
            detail: format!("`{other}` is not an action; use encode or decode"),
        }),
    }
}

/// Which alphabet and input interpretation to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// The standard alphabet, with padding.
    Normal,
    /// The URL-safe alphabet, without padding.
    Url,
    /// The standard alphabet, over hex input.
    Hex,
}

impl Kind {
    /// Parses the tag's second argument.
    fn parse(text: &str) -> Result<Self, TagError> {
        match text {
            "normal" => Ok(Self::Normal),
            "url" => Ok(Self::Url),
            "hex" => Ok(Self::Hex),
            other => Err(TagError::BadArguments {
                tag: "base64",
                detail: format!("`{other}` is not a kind; use normal, url or hex"),
            }),
        }
    }
}

/// Encodes a value.
///
/// The original widens the text to UTF-8 and then hands the bytes to `btoa` as
/// a binary string, so the encoding is over UTF-8 bytes
/// (`templating.md` § Custom tags, `base64`).
fn encode(value: &str, kind: Kind) -> String {
    let bytes = match kind {
        Kind::Hex => hex_to_bytes(value),
        Kind::Normal | Kind::Url => value.as_bytes().to_vec(),
    };
    let alphabet = match kind {
        Kind::Url => URL_SAFE,
        Kind::Normal | Kind::Hex => STANDARD,
    };
    let encoded = encode_bytes(&bytes, alphabet);
    match kind {
        // The original strips the padding for the URL-safe form, because a
        // padded value is not safe in a path segment.
        Kind::Url => encoded.trim_end_matches('=').to_owned(),
        Kind::Normal | Kind::Hex => encoded,
    }
}

/// Decodes a value.
fn decode(value: &str, kind: Kind) -> Result<String, TagError> {
    let alphabet = match kind {
        Kind::Url => URL_SAFE,
        Kind::Normal | Kind::Hex => STANDARD,
    };
    let bytes = decode_bytes(value, alphabet).ok_or_else(|| TagError::BadArguments {
        tag: "base64",
        detail: "the value is not valid base64".to_owned(),
    })?;
    Ok(match kind {
        Kind::Hex => bytes_to_hex(&bytes),
        // `atob` yields a binary string, which the original reads back as
        // UTF-8; invalid sequences are replaced rather than failing the render.
        Kind::Normal | Kind::Url => String::from_utf8_lossy(&bytes).into_owned(),
    })
}

/// Reads hex pairs into bytes.
///
/// An odd trailing nibble is dropped, which is what the original's
/// `/.{1,2}/g` match does.
fn hex_to_bytes(text: &str) -> Vec<u8> {
    let digits: Vec<u8> = text
        .bytes()
        .filter(|byte| byte.is_ascii_hexdigit())
        .collect();
    digits
        .chunks(2)
        .filter_map(|pair| {
            let high = hex_digit(pair[0])?;
            let low = pair.get(1).and_then(|byte| hex_digit(*byte))?;
            Some(high << 4 | low)
        })
        .collect()
}

/// Writes bytes as lowercase hex, as the original's `toString('hex')` does.
fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0'));
        out.push(char::from_digit(u32::from(byte & 0x0f), 16).unwrap_or('0'));
    }
    out
}

/// The value of one hex digit.
fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Encodes bytes with the given alphabet, padding with `=`.
fn encode_bytes(bytes: &[u8], alphabet: &[u8; 64]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = u32::from(chunk[0]);
        let second = chunk.get(1).copied().map_or(0, u32::from);
        let third = chunk.get(2).copied().map_or(0, u32::from);
        let triple = first << 16 | second << 8 | third;
        out.push(char::from(alphabet[(triple >> 18) as usize & 0x3f]));
        out.push(char::from(alphabet[(triple >> 12) as usize & 0x3f]));
        out.push(if chunk.len() > 1 {
            char::from(alphabet[(triple >> 6) as usize & 0x3f])
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            char::from(alphabet[triple as usize & 0x3f])
        } else {
            '='
        });
    }
    out
}

/// Decodes base64 with the given alphabet, or `None` when the input is not
/// valid.
fn decode_bytes(text: &str, alphabet: &[u8; 64]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() / 4 * 3);
    let mut buffer: u32 = 0;
    let mut bits = 0;
    for byte in text.bytes() {
        if byte == b'=' || byte.is_ascii_whitespace() {
            continue;
        }
        let value = alphabet.iter().position(|candidate| *candidate == byte)?;
        buffer = buffer << 6 | value as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn run_args(action: &str, kind: &str, value: &str) -> Result<String, TagError> {
        run(&[json!(action), json!(kind), json!(value)]).map(|value| to_output(&value))
    }

    #[test]
    fn a_normal_encoding_matches_the_standard_alphabet() -> Result<(), TagError> {
        assert_eq!(run_args("encode", "normal", "hello")?, "aGVsbG8=");
        assert_eq!(run_args("encode", "normal", "a")?, "YQ==");
        assert_eq!(run_args("encode", "normal", "ab")?, "YWI=");
        assert_eq!(run_args("encode", "normal", "")?, "");
        Ok(())
    }

    #[test]
    fn a_normal_decoding_reverses_it() -> Result<(), TagError> {
        assert_eq!(run_args("decode", "normal", "aGVsbG8=")?, "hello");
        assert_eq!(run_args("decode", "normal", "YQ==")?, "a");
        Ok(())
    }

    #[test]
    fn a_url_encoding_uses_the_safe_alphabet_and_no_padding() -> Result<(), TagError> {
        // `\u{fb}\u{ff}` is `c3 bb c3 bf` in UTF-8, which is `w7vDvw==`.
        let standard = run_args("encode", "normal", "\u{fb}\u{ff}")?;
        assert_eq!(standard, "w7vDvw==");
        assert_eq!(run_args("encode", "url", "\u{fb}\u{ff}")?, "w7vDvw");
        Ok(())
    }

    #[test]
    fn a_url_decoding_accepts_the_safe_alphabet() -> Result<(), TagError> {
        assert_eq!(run_args("decode", "url", "w7vDvw")?, "\u{fb}\u{ff}");
        Ok(())
    }

    #[test]
    fn the_url_alphabet_replaces_the_standard_one() -> Result<(), TagError> {
        // `\u{ff}\u{ff}\u{ff}` is `c3 bf c3 bf c3 bf`, which uses `+` and `/`
        // in the standard alphabet and `-` and `_` in the URL-safe one.
        let standard = run_args("encode", "normal", "\u{ff}\u{ff}\u{ff}")?;
        assert!(
            standard.contains('+') || standard.contains('/'),
            "{standard}"
        );
        let url = run_args("encode", "url", "\u{ff}\u{ff}\u{ff}")?;
        assert!(!url.contains('+') && !url.contains('/'), "{url}");
        assert!(!url.contains('='), "{url}");
        Ok(())
    }

    #[test]
    fn a_hex_encoding_reads_hex_pairs() -> Result<(), TagError> {
        // `68656c6c6f` is "hello".
        assert_eq!(run_args("encode", "hex", "68656c6c6f")?, "aGVsbG8=");
        Ok(())
    }

    #[test]
    fn a_hex_decoding_writes_lowercase() -> Result<(), TagError> {
        assert_eq!(run_args("decode", "hex", "aGVsbG8=")?, "68656c6c6f");
        Ok(())
    }

    #[test]
    fn an_odd_hex_length_drops_the_trailing_nibble() -> Result<(), TagError> {
        // The original's `/.{1,2}/g` match drops the last nibble silently.
        assert_eq!(run_args("encode", "hex", "68656c6c6f7")?, "aGVsbG8=");
        Ok(())
    }

    #[test]
    fn an_unknown_action_or_kind_is_an_error() {
        assert!(run_args("nope", "normal", "x").is_err());
        assert!(run_args("encode", "nope", "x").is_err());
    }

    #[test]
    fn invalid_base64_is_an_error() {
        assert!(run_args("decode", "normal", "!!!!").is_err());
    }

    #[test]
    fn a_round_trip_keeps_utf8() -> Result<(), TagError> {
        let text = "ação 😀";
        let encoded = run_args("encode", "normal", text)?;
        assert_eq!(run_args("decode", "normal", &encoded)?, text);
        Ok(())
    }
}
