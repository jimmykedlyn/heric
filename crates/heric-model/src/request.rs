//! Request-side helpers: path parameters and version snapshots.
//!
//! Two things the original keeps in `insomnia-data/src/models/request.ts` and
//! in its `request-version` service, and which the send pipeline and the
//! history panel both need: turning `:name` URL segments into values, and
//! recording a request's state so history can restore it.

use std::io::{Read, Write};

use serde_json::{Map, Value};

use crate::{Doc, DocKind, ModelError};

/// Body fields a snapshot must not carry back over a live request.
///
/// Transcribed from `request-version.ts:22-31`. The original also lists the
/// identity fields `_id`, `type`, `created`, `modified` and `parentId`, which
/// in Heric are meta and therefore cannot reach the body at all; they are
/// omitted here rather than filtered twice.
const FIELDS_NOT_RESTORED: [&str; 3] = ["metaSortKey", "description", "name"];

/// The `RequestVersion` body field holding the compressed request.
const COMPRESSED_REQUEST: &str = "compressedRequest";

/// Characters `encodeURIComponent` leaves alone.
///
/// Transcribed from the ECMAScript spec's `unescapedURISet` rather than
/// guessed: `- _ . ! ~ * ' ( )` plus the alphanumerics. Anything else is
/// percent-encoded as UTF-8, which is what makes a value with a space or a
/// slash safe to place in a path segment.
fn is_uri_component_safe(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'-' | b'_' | b'.' | b'!' | b'~' | b'*' | b'\'' | b'(' | b')'
        )
}

/// Percent-encodes a value the way `encodeURIComponent` does.
#[must_use]
pub fn encode_component(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        if is_uri_component_safe(*byte) {
            encoded.push(char::from(*byte));
        } else {
            encoded.push('%');
            encoded.push_str(&format!("{byte:02X}"));
        }
    }
    encoded
}

/// The name of the `:name` segment starting at `start`, and the byte offset
/// just past it.
///
/// Returns `None` for a bare `:` at the end of the URL. The character set is
/// the original's `PATH_PARAMETER_REGEX`, `/:[^/?#:]+`: the name ends at a
/// slash, a query, a fragment or another colon.
fn segment_name(url: &str, start: usize) -> Option<(&str, usize)> {
    let bytes = url.as_bytes();
    let mut end = start;
    while end < bytes.len() && !matches!(bytes[end], b'/' | b'?' | b'#' | b':') {
        end += 1;
    }
    if end == start {
        return None;
    }
    Some((url.get(start..end)?, end))
}

/// The value a parameter row supplies, if the row names `name`.
///
/// A row whose `value` is absent counts as empty, which is what the original's
/// falsy check does: the segment is left alone.
fn value_for<'a>(parameters: &'a [Value], name: &str) -> Result<&'a str, ModelError> {
    for row in parameters {
        let row_name = row.get("name").and_then(Value::as_str);
        if row_name != Some(name) {
            continue;
        }
        return match row.get("value") {
            None | Some(Value::Null) => Ok(""),
            Some(Value::String(value)) => Ok(value),
            // Rows written by the original always carry strings. A number here
            // came from a hand-edited file, and guessing at its spelling would
            // quietly send something the user did not write.
            Some(_) => Err(ModelError::MissingField { field: "value" }),
        };
    }
    Ok("")
}

/// Turns `:name` URL segments into the values the request supplies.
///
/// Returns the rewritten URL and the parameters the URL actually contains, in
/// URL order and deduplicated by name, paired with the value that was applied
/// (`""` when none was). That second list is what the editor shows: a saved
/// parameter whose segment has been deleted from the URL is not in it, which is
/// the original's `getCombinedPathParametersFromUrl` behaviour.
///
/// A parameter with an empty value leaves its `:name` segment untouched, so the
/// URL still reads as a template instead of becoming `/users/`.
///
/// # Errors
///
/// Returns [`ModelError::MissingField`] when a row has a non-string `value`.
pub fn path_parameters(
    url: &str,
    parameters: &[Value],
) -> Result<(String, Vec<(String, String)>), ModelError> {
    let bytes = url.as_bytes();
    let mut rewritten = String::with_capacity(url.len());
    let mut found: Vec<(String, String)> = Vec::new();
    let mut cursor = 0;

    while cursor < bytes.len() {
        // A segment starts at a `/` followed by `:`.
        let is_start = bytes[cursor] == b'/' && bytes.get(cursor + 1) == Some(&b':');
        if !is_start {
            // Copy one whole UTF-8 character so multi-byte text survives.
            let step = url
                .get(cursor..)
                .and_then(|rest| rest.chars().next())
                .map_or(1, char::len_utf8);
            if let Some(text) = url.get(cursor..cursor + step) {
                rewritten.push_str(text);
            }
            cursor += step;
            continue;
        }

        let name_start = cursor + 2;
        let Some((name, end)) = segment_name(url, name_start) else {
            rewritten.push('/');
            cursor += 1;
            continue;
        };
        let value = value_for(parameters, name)?;

        if !found.iter().any(|(seen, _)| seen == name) {
            found.push((name.to_owned(), value.to_owned()));
        }

        if value.is_empty() {
            // Keep the segment as written.
            if let Some(text) = url.get(cursor..end) {
                rewritten.push_str(text);
            }
        } else {
            rewritten.push('/');
            rewritten.push_str(&encode_component(value));
        }
        cursor = end;
    }

    Ok((rewritten, found))
}

/// Merges the parameters the editor has saved with those the URL holds.
///
/// Saved rows survive only when the URL still names them, in their saved order;
/// the rest are appended with an empty value, so a newly typed `:id` appears as
/// a blank field rather than silently keeping an older value.
#[must_use]
pub fn merge_parameters(saved: &[Value], unsaved: &[Value]) -> Vec<Value> {
    let names: Vec<&str> = unsaved
        .iter()
        .filter_map(|row| row.get("name").and_then(Value::as_str))
        .collect();

    let mut merged: Vec<Value> = Vec::new();
    let mut kept: Vec<&str> = Vec::new();
    for row in saved {
        let Some(name) = row.get("name").and_then(Value::as_str) else {
            continue;
        };
        if names.contains(&name) && !kept.contains(&name) {
            merged.push(row.clone());
            kept.push(name);
        }
    }
    for row in unsaved {
        let Some(name) = row.get("name").and_then(Value::as_str) else {
            continue;
        };
        if kept.contains(&name) {
            continue;
        }
        let mut entry = Map::new();
        entry.insert("name".to_owned(), Value::String(name.to_owned()));
        entry.insert("value".to_owned(), Value::String(String::new()));
        merged.push(Value::Object(entry));
        kept.push(name);
    }
    merged
}

/// The base64 alphabet, standard (RFC 4648) with padding.
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encodes bytes as standard base64 with padding.
///
/// Hand-rolled because the plan's Global Constraints name `rusqlite`, `flate2`
/// and `tempfile` as this milestone's only new crates, and a base64 crate is
/// not among them. The payload is a request document — a few kilobytes at
/// most — so a table lookup loop is the right size of implementation.
#[must_use]
pub fn base64_encode(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let first = u32::from(chunk.first().copied().unwrap_or_default());
        let second = u32::from(chunk.get(1).copied().unwrap_or_default());
        let third = u32::from(chunk.get(2).copied().unwrap_or_default());
        let triple = (first << 16) | (second << 8) | third;

        let indexes = [
            (triple >> 18) & 0x3f,
            (triple >> 12) & 0x3f,
            (triple >> 6) & 0x3f,
            triple & 0x3f,
        ];
        // A short chunk has fewer real bytes, so the trailing slots become `=`.
        for (position, index) in indexes.iter().enumerate() {
            if position <= chunk.len() {
                let symbol = ALPHABET
                    .get(usize::try_from(*index).unwrap_or_default())
                    .copied()
                    .unwrap_or(b'A');
                encoded.push(char::from(symbol));
            } else {
                encoded.push('=');
            }
        }
    }
    encoded
}

/// Decodes standard base64, tolerating missing padding.
///
/// # Errors
///
/// Returns [`ModelError::CorruptBody`] when a character is not in the alphabet,
/// when padding appears anywhere but at the end, or when the tail cannot be a
/// whole number of bytes.
pub fn base64_decode(text: &str) -> Result<Vec<u8>, ModelError> {
    let mut decoded = Vec::with_capacity(text.len() / 4 * 3);
    let mut accumulator: u32 = 0;
    let mut bits: u32 = 0;
    let mut padding = 0_usize;

    for character in text.bytes() {
        if character == b'=' {
            padding += 1;
            continue;
        }
        if padding > 0 {
            // A real symbol after padding means the padding was in the middle.
            return Err(ModelError::CorruptBody {
                detail: "base64 padding before the end".to_owned(),
            });
        }
        let value = match character {
            b'A'..=b'Z' => u32::from(character - b'A'),
            b'a'..=b'z' => u32::from(character - b'a') + 26,
            b'0'..=b'9' => u32::from(character - b'0') + 52,
            b'+' => 62,
            b'/' => 63,
            _ => {
                return Err(ModelError::CorruptBody {
                    detail: format!("invalid base64 character `{}`", char::from(character)),
                });
            }
        };
        accumulator = (accumulator << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            decoded.push(u8::try_from((accumulator >> bits) & 0xff).unwrap_or_default());
        }
    }

    // Leftover bits that do not fill a byte are only valid when they are the
    // zero padding a short final group implies. Two leftover bits is one byte
    // of data, four is two: anything else means the text was truncated.
    if bits >= 6 || padding > 2 {
        return Err(ModelError::CorruptBody {
            detail: "truncated base64".to_owned(),
        });
    }
    Ok(decoded)
}

/// Gzips and base64-encodes a document, the way the original stores a
/// `RequestVersion` (`request-version.ts:41-44`).
///
/// # Errors
///
/// Returns [`ModelError::CorruptBody`] when the document cannot be serialised
/// or the encoder fails.
pub fn compress_request(request: &Doc) -> Result<String, ModelError> {
    let json = serde_json::to_vec(request).map_err(|error| ModelError::CorruptBody {
        detail: error.to_string(),
    })?;
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder
        .write_all(&json)
        .map_err(|error| ModelError::CorruptBody {
            detail: error.to_string(),
        })?;
    let gzipped = encoder.finish().map_err(|error| ModelError::CorruptBody {
        detail: error.to_string(),
    })?;
    Ok(base64_encode(&gzipped))
}

/// Reverses [`compress_request`].
///
/// # Errors
///
/// Returns [`ModelError::CorruptBody`] when the text is not base64, does not
/// gunzip, or does not deserialise into a document.
pub fn decompress_request(payload: &str) -> Result<Doc, ModelError> {
    let gzipped = base64_decode(payload)?;
    let mut json = Vec::new();
    flate2::read::GzDecoder::new(gzipped.as_slice())
        .read_to_end(&mut json)
        .map_err(|error| ModelError::CorruptBody {
            detail: error.to_string(),
        })?;
    serde_json::from_slice(&json).map_err(|error| ModelError::CorruptBody {
        detail: error.to_string(),
    })
}

/// Builds the `RequestVersion` document that records `request`'s state.
///
/// The version is a child of the request it snapshots, so a request's history
/// is one query by `parentId`. `kind` names the kind of version document to
/// create: every versioned request lands in `RequestVersion` in this milestone,
/// and naming it at the call site keeps the wrapper's type visible rather than
/// implied.
///
/// A request already snapshotted without changes does not need a second record.
/// [`differs`] answers that question; deciding to skip the insert belongs to
/// the caller, which is where the store lives.
#[must_use]
pub fn snapshot(request: &Doc, kind: DocKind, now: i64) -> Doc {
    let mut version = crate::init::document(kind, now);
    version.meta.parent = Some(request.meta.id.clone());
    match compress_request(request) {
        Ok(compressed) => {
            version
                .body
                .insert(COMPRESSED_REQUEST.to_owned(), Value::String(compressed));
        }
        // A document that will not serialise cannot be snapshotted, and the
        // version is still worth recording: an empty payload reads as "no
        // restore available" in the history panel, which is honest, whereas a
        // missing version would let the response point at nothing.
        Err(_) => {
            version
                .body
                .insert(COMPRESSED_REQUEST.to_owned(), Value::Null);
        }
    }
    version
}

/// Whether `request` differs from what a previous snapshot recorded.
///
/// Compares every body field except the three the original ignores, which is
/// its `_diffRequests` (`request-version.ts:126-145`). Identity fields live in
/// [`crate::DocMeta`] and are not part of the body, so they cannot cause a
/// spurious difference.
///
/// A snapshot whose payload is missing or unreadable counts as different: the
/// alternative is reusing a snapshot nobody can restore.
#[must_use]
pub fn differs(snapshot: &Doc, request: &Doc) -> bool {
    let Some(payload) = snapshot
        .body
        .get(COMPRESSED_REQUEST)
        .and_then(Value::as_str)
    else {
        return true;
    };
    let Ok(previous) = decompress_request(payload) else {
        return true;
    };
    if previous.meta.kind != request.meta.kind {
        return true;
    }

    let comparable = |doc: &Doc| -> Map<String, Value> {
        doc.body
            .iter()
            .filter(|(key, _)| !FIELDS_NOT_RESTORED.contains(&key.as_str()))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    };
    comparable(&previous) != comparable(request)
}

/// The body fields a restore should write back onto the live request.
///
/// Everything the snapshot holds except the three fields the original deletes
/// before updating, so a restore cannot rename the request or move it. The live
/// document's own meta stays untouched by construction.
///
/// # Errors
///
/// Returns [`ModelError::CorruptBody`] when the payload will not decode.
pub fn restore_body(snapshot: &Doc) -> Result<Map<String, Value>, ModelError> {
    let Some(payload) = snapshot
        .body
        .get(COMPRESSED_REQUEST)
        .and_then(Value::as_str)
    else {
        return Ok(Map::new());
    };
    let previous = decompress_request(payload)?;
    Ok(previous
        .body
        .into_iter()
        .filter(|(key, _)| !FIELDS_NOT_RESTORED.contains(&key.as_str()))
        .collect())
}

/// Whether `doc` is a request kind the original versions.
///
/// `request-version.ts:61-66` accepts `Request`, `WebSocketRequest`,
/// `SocketIORequest`, `GrpcRequest` and `McpRequest`, and refuses everything
/// else.
#[must_use]
pub fn is_versionable(kind: DocKind) -> bool {
    matches!(
        kind,
        DocKind::Request
            | DocKind::WebSocketRequest
            | DocKind::SocketIORequest
            | DocKind::GrpcRequest
            | DocKind::McpRequest
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::Id;

    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// A request document with one header, for the snapshot tests.
    fn request_doc() -> Doc {
        let mut doc = crate::init::document(DocKind::Request, 1_000);
        doc.meta.id = Id::new("req_1");
        doc.body.insert(
            "url".to_owned(),
            Value::String("https://x.test/".to_owned()),
        );
        doc.body.insert(
            "headers".to_owned(),
            json!([{"name": "accept", "value": "application/json"}]),
        );
        doc
    }

    #[test]
    fn a_saved_path_parameter_is_merged_over_the_url_template() -> TestResult {
        let (url, found) = path_parameters(
            "https://x.test/users/:id",
            &[json!({"name": "id", "value": "7"})],
        )?;
        assert_eq!(url, "https://x.test/users/7");
        assert_eq!(found, [("id".to_owned(), "7".to_owned())]);
        Ok(())
    }

    #[test]
    fn an_empty_or_absent_value_leaves_the_segment_as_written() -> TestResult {
        let template = "https://x.test/users/:id/posts/:post";
        let (url, found) = path_parameters(template, &[json!({"name": "id", "value": ""})])?;
        assert_eq!(url, template);
        assert_eq!(
            found,
            [
                ("id".to_owned(), String::new()),
                ("post".to_owned(), String::new())
            ]
        );

        // No parameters at all is the same URL, unchanged.
        let (same, _) = path_parameters(template, &[])?;
        assert_eq!(same, template);
        Ok(())
    }

    #[test]
    fn a_value_is_percent_encoded_per_segment() -> TestResult {
        let (url, _) = path_parameters(
            "https://x.test/search/:query/next",
            &[json!({"name": "query", "value": "a b/c"})],
        )?;
        // The slash is encoded, so it cannot split the path; the space too.
        assert_eq!(url, "https://x.test/search/a%20b%2Fc/next");
        Ok(())
    }

    #[test]
    fn the_encoding_set_matches_encode_uri_component() {
        // `encodeURIComponent` leaves exactly these unescaped.
        assert_eq!(encode_component("aZ0-_.!~*'()"), "aZ0-_.!~*'()");
        assert_eq!(encode_component("&="), "%26%3D");
        assert_eq!(encode_component("é"), "%C3%A9");
    }

    #[test]
    fn a_repeated_segment_is_listed_once_but_rewritten_everywhere() -> TestResult {
        let (url, found) = path_parameters(
            "https://x.test/:id/a/:id",
            &[json!({"name": "id", "value": "5"})],
        )?;
        assert_eq!(url, "https://x.test/5/a/5");
        assert_eq!(found, [("id".to_owned(), "5".to_owned())]);
        Ok(())
    }

    #[test]
    fn a_segment_ends_at_a_colon_and_a_query_is_not_a_segment() -> TestResult {
        // The second colon starts a new segment, so `:b` is a parameter and the
        // first `:a` stops at it.
        let (url, found) =
            path_parameters("https://x.test/:a:b", &[json!({"name": "a", "value": "1"})])?;
        assert_eq!(url, "https://x.test/1:b");
        // `:b` is not preceded by a slash, so the original's
        // `/:[^/?#:]+` never matches it and it is not a parameter.
        assert_eq!(found, [("a".to_owned(), "1".to_owned())]);

        // A query string's `:` is not at a segment start.
        let (query, found) =
            path_parameters("https://x.test?q=:x", &[json!({"name": "x", "value": "9"})])?;
        assert_eq!(query, "https://x.test?q=:x");
        assert!(found.is_empty());
        Ok(())
    }

    #[test]
    fn a_non_string_parameter_value_is_an_error() {
        assert!(matches!(
            path_parameters("https://x.test/:id", &[json!({"name": "id", "value": 7})]),
            Err(ModelError::MissingField { field: "value" })
        ));
    }

    #[test]
    fn merging_keeps_saved_values_and_appends_new_names() {
        let saved = [
            json!({"name": "id", "value": "7"}),
            json!({"name": "gone", "value": "x"}),
        ];
        let unsaved = [
            json!({"name": "id", "value": ""}),
            json!({"name": "post", "value": ""}),
        ];
        assert_eq!(
            merge_parameters(&saved, &unsaved),
            [
                json!({"name": "id", "value": "7"}),
                json!({"name": "post", "value": ""})
            ]
        );
    }

    #[test]
    fn base64_matches_the_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn base64_round_trips_every_tail_length() -> TestResult {
        for length in 0..8 {
            let bytes: Vec<u8> = (0..length)
                .map(|value| u8::try_from(value).unwrap_or(0))
                .collect();
            let encoded = base64_encode(&bytes);
            assert_eq!(base64_decode(&encoded)?, bytes, "for length {length}");
        }
        Ok(())
    }

    #[test]
    fn malformed_base64_is_rejected() {
        assert!(matches!(
            base64_decode("!!!!"),
            Err(ModelError::CorruptBody { .. })
        ));
        assert!(matches!(
            base64_decode("Zg==Zg=="),
            Err(ModelError::CorruptBody { .. })
        ));
    }

    #[test]
    fn a_request_snapshot_compresses_and_round_trips() -> TestResult {
        let request = request_doc();
        let version = snapshot(&request, DocKind::RequestVersion, 2_000);
        assert_eq!(version.meta.kind, DocKind::RequestVersion);
        assert_eq!(version.meta.parent, Some(Id::new("req_1")));

        let payload = version
            .body
            .get(COMPRESSED_REQUEST)
            .and_then(Value::as_str)
            .ok_or("the snapshot carried no payload")?;
        let restored = decompress_request(payload)?;
        assert_eq!(restored.body, request.body);
        assert_eq!(restored.meta.id, request.meta.id);
        Ok(())
    }

    #[test]
    fn an_unchanged_request_does_not_differ_from_its_snapshot() -> TestResult {
        let request = request_doc();
        let version = snapshot(&request, DocKind::RequestVersion, 2_000);
        assert!(!differs(&version, &request));
        Ok(())
    }

    #[test]
    fn a_changed_header_makes_the_request_differ() -> TestResult {
        let request = request_doc();
        let version = snapshot(&request, DocKind::RequestVersion, 2_000);

        let mut changed = request.clone();
        changed
            .body
            .insert("url".to_owned(), json!("https://x.test/other"));
        assert!(differs(&version, &changed));

        let mut relabelled = request.clone();
        relabelled
            .body
            .insert("metaSortKey".to_owned(), json!(-1.0));
        // `metaSortKey` is in the ignore list, so this is not a change.
        assert!(!differs(&version, &relabelled));

        let mut described = request.clone();
        described
            .body
            .insert("description".to_owned(), json!("new"));
        assert!(!differs(&version, &described));
        Ok(())
    }

    #[test]
    fn a_snapshot_with_no_payload_always_differs() -> TestResult {
        let mut version = snapshot(&request_doc(), DocKind::RequestVersion, 1);
        version
            .body
            .insert(COMPRESSED_REQUEST.to_owned(), Value::Null);
        assert!(differs(&version, &request_doc()));
        Ok(())
    }

    #[test]
    fn restoring_returns_the_body_without_the_ignored_fields() -> TestResult {
        let mut request = request_doc();
        request.body.insert("description".to_owned(), json!("old"));
        let version = snapshot(&request, DocKind::RequestVersion, 2_000);

        let body = restore_body(&version)?;
        assert_eq!(body.get("url"), Some(&json!("https://x.test/")));
        assert!(!body.contains_key("description"));
        assert!(!body.contains_key("metaSortKey"));
        Ok(())
    }

    #[test]
    fn a_versionless_snapshot_restores_nothing() -> TestResult {
        let mut version = snapshot(&request_doc(), DocKind::RequestVersion, 1);
        version
            .body
            .insert(COMPRESSED_REQUEST.to_owned(), Value::Null);
        assert!(restore_body(&version)?.is_empty());
        Ok(())
    }

    #[test]
    fn only_request_kinds_are_versioned() {
        assert!(is_versionable(DocKind::Request));
        assert!(is_versionable(DocKind::GrpcRequest));
        assert!(!is_versionable(DocKind::Response));
        assert!(!is_versionable(DocKind::Workspace));
    }
}
