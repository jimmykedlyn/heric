//! Response history.
//!
//! A response document records the exchange and points at two blobs: the body
//! and the Console timeline. Neither is stored inline, because a body can be
//! megabytes and the document is what the sidebar queries.

use std::io::Read;

use serde_json::Value;

use crate::{Doc, DocKind, Execution, Id, ModelError};

/// How many responses a request keeps by default.
///
/// The original's `Settings.maxHistoryResponses` defaults to 20
/// (`models/settings.ts:62`) and its creation path then clamps with
/// `Math.max(1, maxResponses)` (`node-src/services/response.ts:64`). [`prune`]
/// performs no clamping, so a caller replaying the original passes
/// `max.max(1)` to get the same retention.
pub const DEFAULT_MAX_RESPONSES: usize = 20;

/// The compression sentinel for a body written before the field existed.
const NEEDS_MIGRATION: &str = "__NEEDS_MIGRATION__";
/// The compression value for a gzipped body.
const ZIP: &str = "zip";
/// The environment sentinel for a response recorded before environments were
/// attached.
const LEGACY_ENVIRONMENT: &str = "__LEGACY__";

/// Builds the response document for one exchange.
///
/// Every key starts from the model's `init()` defaults and is then overridden,
/// so a field the original added since this list was written keeps its default
/// rather than disappearing.
///
/// `environmentId` keeps the `__LEGACY__` sentinel, because this function has
/// no environment to name. The caller that knows one sets it before inserting,
/// which is what the original's `create(patch)` does — `environmentId` arrives
/// in the patch, not in the helper.
#[must_use]
pub fn create(
    request: &Doc,
    execution: &Execution,
    body_path: &str,
    timeline_path: &str,
    now: i64,
) -> Doc {
    build(request, execution, body_path, timeline_path, now, false)
}

/// [`create`] for a body that is stored gzipped.
#[must_use]
pub fn create_with_compression(
    request: &Doc,
    execution: &Execution,
    body_path: &str,
    timeline_path: &str,
    now: i64,
    body_compressed: bool,
) -> Doc {
    build(
        request,
        execution,
        body_path,
        timeline_path,
        now,
        body_compressed,
    )
}

/// The shared body of [`create`] and [`create_with_compression`].
fn build(
    request: &Doc,
    execution: &Execution,
    body_path: &str,
    timeline_path: &str,
    now: i64,
    body_compressed: bool,
) -> Doc {
    let mut doc = crate::init::document(DocKind::Response, now);
    doc.meta.parent = Some(request.meta.id.clone());

    let headers = execution
        .headers
        .iter()
        .map(|header| {
            let mut row = serde_json::Map::new();
            row.insert("name".to_owned(), Value::String(header.name.clone()));
            row.insert("value".to_owned(), Value::String(header.value.clone()));
            Value::Object(row)
        })
        .collect();

    let compression = if body_compressed {
        ZIP
    } else {
        NEEDS_MIGRATION
    };
    let error = execution.error.clone().unwrap_or_default();
    let content_type = execution.content_type.clone().unwrap_or_default();
    let bytes_read = i64::try_from(execution.bytes_read).unwrap_or(i64::MAX);
    let elapsed = i64::try_from(execution.elapsed.as_millis()).unwrap_or(i64::MAX);

    let overrides: [(&str, Value); 13] = [
        ("statusCode", Value::from(execution.status)),
        (
            "statusMessage",
            Value::String(execution.status_message.clone()),
        ),
        ("httpVersion", Value::String(execution.http_version.clone())),
        ("contentType", Value::String(content_type)),
        ("url", Value::String(execution.url.clone())),
        ("bytesRead", Value::from(bytes_read)),
        ("bytesContent", Value::from(execution.bytes_content)),
        // The original stores the duration under `elapsedTime`, not `elapsed`.
        ("elapsedTime", Value::from(elapsed)),
        ("headers", Value::Array(headers)),
        ("bodyPath", Value::String(body_path.to_owned())),
        ("timelinePath", Value::String(timeline_path.to_owned())),
        ("bodyCompression", Value::String(compression.to_owned())),
        ("error", Value::String(error)),
    ];
    for (field, value) in overrides {
        doc.body.insert(field.to_owned(), value);
    }

    // The version the request was in when it was sent, so history can restore
    // it. A request that carries none leaves the `null` default.
    if let Some(version) = request.body.get("requestVersionId").cloned() {
        doc.body.insert("requestVersionId".to_owned(), version);
    }
    doc
}

/// Reads a response body, decompressing it when it was stored gzipped.
///
/// Empty input is empty output before any decode is attempted: a response
/// without a body has a `bodyPath` of `""`, and treating that as corrupt would
/// make every 204 look broken.
///
/// # Errors
///
/// Returns [`ModelError::CorruptBody`] when `compressed` is set and the bytes do
/// not gunzip.
pub fn read_body(bytes: &[u8], compressed: bool) -> Result<Vec<u8>, ModelError> {
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    if !compressed {
        return Ok(bytes.to_vec());
    }
    let mut decoded = Vec::new();
    flate2::read::GzDecoder::new(bytes)
        .read_to_end(&mut decoded)
        .map_err(|error| ModelError::CorruptBody {
            detail: error.to_string(),
        })?;
    Ok(decoded)
}

/// Whether a stored response body needs decompressing.
///
/// The mapping from the document's `bodyCompression` field to [`read_body`]'s
/// boolean lives here so the caller does not repeat the sentinel comparison.
/// `__NEEDS_MIGRATION__` is not compressed: it is the marker a migrated
/// document carries, and the migration layer rewrites it.
#[must_use]
pub fn is_compressed(doc: &Doc) -> bool {
    doc.body.get("bodyCompression").and_then(Value::as_str) == Some(ZIP)
}

/// Keeps the newest `max_responses` documents.
///
/// Sorted by `modified` descending, ties broken by id ascending so the result is
/// deterministic. A limit of `0` yields an empty list: the original's
/// `Math.max(1, …)` clamp belongs to its creation service, not to the pure
/// helper, and clamping here would make the function's behaviour disagree with
/// its argument.
#[must_use]
pub fn prune(docs: Vec<Doc>, max_responses: usize) -> Vec<Doc> {
    let mut docs = docs;
    docs.sort_by(|left, right| {
        right
            .meta
            .modified
            .cmp(&left.meta.modified)
            .then_with(|| left.meta.id.as_str().cmp(right.meta.id.as_str()))
    });
    docs.truncate(max_responses);
    docs
}

/// The most recent response, optionally restricted to one environment.
///
/// `None` means no filter, which is what the original does when
/// `filterResponsesByEnv` is off (`node-src/services/response.ts:31-42`). With
/// a filter, only a response naming exactly that environment matches, so a
/// `__LEGACY__` response is excluded whenever the filter is on.
#[must_use]
pub fn latest<'a>(docs: &'a [Doc], environment: Option<&Id>) -> Option<&'a Doc> {
    docs.iter()
        .filter(|doc| match environment {
            None => true,
            Some(wanted) => {
                doc.body.get("environmentId").and_then(Value::as_str) == Some(wanted.as_str())
            }
        })
        .max_by(|left, right| {
            left.meta
                .modified
                .cmp(&right.meta.modified)
                // `max_by` keeps the last maximum, so compare ids in reverse to
                // leave the smallest id on a tie.
                .then_with(|| right.meta.id.as_str().cmp(left.meta.id.as_str()))
        })
}

/// Whether a response was recorded before environments were tracked.
#[must_use]
pub fn is_legacy_environment(doc: &Doc) -> bool {
    doc.body.get("environmentId").and_then(Value::as_str) == Some(LEGACY_ENVIRONMENT)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use serde_json::json;

    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// A request that has been saved at least once, so it carries a version.
    fn request_doc() -> Result<Doc, ModelError> {
        let mut doc = crate::init::document(DocKind::Request, 1);
        doc.meta.id = Id::new("req_1");
        doc.body.insert(
            "requestVersionId".to_owned(),
            Value::String("rvr_1".to_owned()),
        );
        Ok(doc)
    }

    /// A response document with the given `modified` stamp and environment.
    fn response(id: &str, modified: i64, environment: &str) -> Result<Doc, ModelError> {
        let mut doc = crate::init::document(DocKind::Response, modified);
        doc.meta.id = Id::new(id);
        doc.meta.modified = modified;
        doc.body.insert(
            "environmentId".to_owned(),
            Value::String(environment.to_owned()),
        );
        Ok(doc)
    }

    #[test]
    fn a_response_snapshots_the_request_version_and_paths() -> TestResult {
        let request = request_doc()?;
        let response = create(&request, &Execution::ok(204), "b/res_1", "t/res_1", 10);
        assert_eq!(response.body.get("statusCode"), Some(&json!(204)));
        assert_eq!(response.body.get("bodyPath"), Some(&json!("b/res_1")));
        assert_eq!(response.body.get("requestVersionId"), Some(&json!("rvr_1")));
        assert_eq!(response.body.get("timelinePath"), Some(&json!("t/res_1")));
        assert_eq!(response.meta.kind, DocKind::Response);
        assert_eq!(response.meta.parent, Some(Id::new("req_1")));
        Ok(())
    }

    #[test]
    fn a_response_records_the_elapsed_time_under_the_original_name() -> TestResult {
        let response = create(&request_doc()?, &Execution::ok(200), "", "", 1);
        // The plan's prose says `elapsed`; the original stores `elapsedTime`.
        assert!(response.body.contains_key("elapsedTime"));
        assert!(!response.body.contains_key("elapsed"));
        Ok(())
    }

    #[test]
    fn a_response_copies_every_field_of_the_execution() -> TestResult {
        let mut execution = Execution::ok(201);
        execution.status_message = "Created".to_owned();
        execution.url = "https://x.test/items".to_owned();
        execution.content_type = Some("application/json".to_owned());
        execution.bytes_read = 12;
        execution.bytes_content = 10;
        execution.headers = vec![crate::Header::new("content-type", "application/json")];
        let response = create(&request_doc()?, &execution, "b", "t", 1);

        assert_eq!(response.body.get("statusMessage"), Some(&json!("Created")));
        assert_eq!(response.body.get("httpVersion"), Some(&json!("HTTP/1.1")));
        assert_eq!(
            response.body.get("url"),
            Some(&json!("https://x.test/items"))
        );
        assert_eq!(
            response.body.get("contentType"),
            Some(&json!("application/json"))
        );
        assert_eq!(response.body.get("bytesRead"), Some(&json!(12)));
        assert_eq!(response.body.get("bytesContent"), Some(&json!(10)));
        assert_eq!(
            response.body.get("headers"),
            Some(&json!([{"name": "content-type", "value": "application/json"}]))
        );
        Ok(())
    }

    #[test]
    fn a_failed_execution_keeps_its_error_and_zero_status() -> TestResult {
        let mut execution = Execution::ok(0);
        execution.error = Some("could not resolve host".to_owned());
        let response = create(&request_doc()?, &execution, "", "", 1);
        assert_eq!(
            response.body.get("error"),
            Some(&json!("could not resolve host"))
        );
        assert_eq!(response.body.get("statusCode"), Some(&json!(0)));
        Ok(())
    }

    #[test]
    fn an_unattributed_response_carries_the_legacy_environment_sentinel() -> TestResult {
        let response = create(&request_doc()?, &Execution::ok(200), "", "", 1);
        assert_eq!(
            response.body.get("environmentId"),
            Some(&json!(LEGACY_ENVIRONMENT))
        );
        assert!(is_legacy_environment(&response));
        Ok(())
    }

    #[test]
    fn an_uncompressed_response_carries_the_migration_sentinel() -> TestResult {
        let response = create(&request_doc()?, &Execution::ok(200), "", "", 1);
        assert_eq!(
            response.body.get("bodyCompression"),
            Some(&json!(NEEDS_MIGRATION))
        );
        assert!(!is_compressed(&response));
        Ok(())
    }

    #[test]
    fn a_compressed_response_is_marked_zip_and_reads_back() -> TestResult {
        let response =
            create_with_compression(&request_doc()?, &Execution::ok(200), "", "", 1, true);
        assert_eq!(response.body.get("bodyCompression"), Some(&json!("zip")));
        assert!(is_compressed(&response));
        Ok(())
    }

    #[test]
    fn an_empty_body_is_read_as_empty_not_as_corrupt() {
        assert_eq!(read_body(&[], false), Ok(Vec::new()));
        // Even when the caller says it is compressed: there is nothing to
        // decode and no error to report.
        assert_eq!(read_body(&[], true), Ok(Vec::new()));
    }

    #[test]
    fn a_gzip_body_is_decompressed_and_a_plain_one_passes_through() -> TestResult {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder
            .write_all(b"hello")
            .map_err(|error| ModelError::CorruptBody {
                detail: error.to_string(),
            })?;
        let zipped = encoder.finish().map_err(|error| ModelError::CorruptBody {
            detail: error.to_string(),
        })?;

        assert_eq!(read_body(&zipped, true)?, b"hello");
        assert_eq!(read_body(b"plain", false)?, b"plain");
        Ok(())
    }

    #[test]
    fn bytes_that_are_not_gzip_are_corrupt() {
        assert!(matches!(
            read_body(b"not gzip at all", true),
            Err(ModelError::CorruptBody { .. })
        ));
    }

    #[test]
    fn pruning_keeps_the_newest_twenty() -> TestResult {
        let mut docs = Vec::new();
        for index in 0..25 {
            docs.push(response(&format!("res_{index:02}"), index, "env_a")?);
        }
        let kept = prune(docs, DEFAULT_MAX_RESPONSES);
        assert_eq!(kept.len(), 20);
        let ids: Vec<&str> = kept.iter().map(|doc| doc.meta.id.as_str()).collect();
        // The newest 20 are stamps 24 down to 5.
        assert_eq!(ids.first(), Some(&"res_24"));
        assert_eq!(ids.last(), Some(&"res_05"));
        Ok(())
    }

    #[test]
    fn pruning_by_zero_keeps_nothing() -> TestResult {
        let docs = vec![response("res_1", 1, "env_a")?];
        assert!(prune(docs, 0).is_empty());
        Ok(())
    }

    #[test]
    fn the_latest_response_is_filtered_by_environment_and_sorted_desc() -> TestResult {
        let docs = vec![
            response("res_legacy", 30, LEGACY_ENVIRONMENT)?,
            response("res_a", 10, "env_a")?,
            response("res_b", 20, "env_b")?,
        ];

        // No filter: the legacy response, which is the newest, wins.
        assert_eq!(
            latest(&docs, None).map(|doc| doc.meta.id.as_str()),
            Some("res_legacy")
        );

        // Filtered: the legacy response is excluded even though it is newer.
        assert_eq!(
            latest(&docs, Some(&Id::new("env_a"))).map(|doc| doc.meta.id.as_str()),
            Some("res_a")
        );
        assert_eq!(
            latest(&docs, Some(&Id::new("env_b"))).map(|doc| doc.meta.id.as_str()),
            Some("res_b")
        );
        assert_eq!(latest(&docs, Some(&Id::new("env_none"))), None);
        Ok(())
    }

    #[test]
    fn the_latest_response_breaks_a_tie_on_the_smallest_id() -> TestResult {
        let docs = vec![
            response("res_z", 5, "env_a")?,
            response("res_a", 5, "env_a")?,
        ];
        assert_eq!(
            latest(&docs, None).map(|doc| doc.meta.id.as_str()),
            Some("res_a")
        );
        Ok(())
    }

    #[test]
    fn a_legacy_environment_is_recognised() -> TestResult {
        assert!(is_legacy_environment(&response(
            "res_1",
            1,
            LEGACY_ENVIRONMENT
        )?));
        assert!(!is_legacy_environment(&response("res_2", 2, "env_a")?));
        Ok(())
    }
}
