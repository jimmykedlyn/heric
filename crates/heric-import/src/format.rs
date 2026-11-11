//! Recognising which format a source is written in.

/// A format the importer reads.
///
/// Ordered by how specific its marker is, because detection walks the list: an
/// Insomnia v4 export is JSON that also looks like a HAR if read carelessly, and
/// guessing wrong loses the scripts a v4 file carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Format {
    /// Insomnia export format v4, JSON.
    InsomniaV4,
    /// Insomnia export format v5, YAML.
    InsomniaV5,
    /// Insomnia v1 to v3, with the v2 string-body upgrade.
    InsomniaLegacy,
    /// A Postman v2.0 or v2.1 collection.
    Postman,
    /// A Postman environment export.
    PostmanEnvironment,
    /// A Postman data dump inside an archive.
    PostmanDataDump,
    /// An HTTP Archive.
    Har,
    /// An OpenAPI 3 document.
    OpenApi3,
    /// A Swagger 2.0 document.
    Swagger2,
    /// A WSDL document.
    Wsdl,
    /// A cURL command line.
    Curl,
}

impl Format {
    /// A name for the import dialog.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InsomniaV4 => "Insomnia v4 export",
            Self::InsomniaV5 => "Insomnia v5 export",
            Self::InsomniaLegacy => "Insomnia v1–v3 export",
            Self::Postman => "Postman collection",
            Self::PostmanEnvironment => "Postman environment",
            Self::PostmanDataDump => "Postman data dump",
            Self::Har => "HTTP Archive",
            Self::OpenApi3 => "OpenAPI 3",
            Self::Swagger2 => "Swagger 2.0",
            Self::Wsdl => "WSDL",
            Self::Curl => "cURL command",
        }
    }

    /// Whether the format preserves Insomnia's full document model.
    ///
    /// Only its own exports do. Everything else converts, and the report says
    /// what was lost.
    #[must_use]
    pub fn is_lossless(self) -> bool {
        matches!(
            self,
            Self::InsomniaV4 | Self::InsomniaV5 | Self::InsomniaLegacy
        )
    }
}

/// Detects a source's format.
///
/// Returns `None` when nothing matches, which the caller reports as "unrecognised
/// file" rather than guessing. Detection is deliberately marker-driven: the
/// original checks `__export_format === 4` strictly, and a v4 file with a
/// mistyped marker must not be half-imported.
#[must_use]
pub fn detect(source: &str) -> Option<Format> {
    let trimmed = source.trim_start();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.starts_with("curl ") || trimmed.starts_with("curl\t") {
        return Some(Format::Curl);
    }

    if trimmed.starts_with('<') {
        if trimmed.contains("<definitions") && trimmed.contains("wsdl") {
            return Some(Format::Wsdl);
        }
        return None;
    }

    // Every remaining format is JSON or YAML.
    let value: serde_json::Value = match serde_json::from_str(trimmed) {
        Ok(value) => value,
        Err(_) => return detect_yaml(trimmed),
    };

    detect_json(&value).or_else(|| detect_yaml(trimmed))
}

fn detect_json(value: &serde_json::Value) -> Option<Format> {
    let object = value.as_object()?;

    if let Some(format) = object
        .get("__export_format")
        .and_then(serde_json::Value::as_i64)
    {
        return if format == 4 {
            Some(Format::InsomniaV4)
        } else {
            Some(Format::InsomniaLegacy)
        };
    }
    if object.contains_key("__export_source") || object.contains_key("_type") {
        return Some(Format::InsomniaLegacy);
    }
    if let Some(info) = object.get("info").and_then(serde_json::Value::as_object)
        && info.contains_key("schema")
        && info
            .get("schema")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|schema| schema.contains("schema.getpostman.com"))
    {
        return Some(Format::Postman);
    }
    if object.contains_key("values") && object.contains_key("_postman_variable_scope") {
        return Some(Format::PostmanEnvironment);
    }
    if object.contains_key("log") {
        return Some(Format::Har);
    }
    if let Some(swagger) = object.get("swagger").and_then(serde_json::Value::as_str) {
        return (swagger.starts_with('2')).then_some(Format::Swagger2);
    }
    if let Some(openapi) = object.get("openapi").and_then(serde_json::Value::as_str) {
        return openapi.starts_with('3').then_some(Format::OpenApi3);
    }
    None
}

/// A minimal YAML marker scan.
///
/// Real parsing happens in the importer; this only needs to recognise the two
/// top-level markers that distinguish a v5 export from a spec, and it runs only
/// after JSON parsing has already failed.
fn detect_yaml(source: &str) -> Option<Format> {
    let mut saw_export = None;
    let mut saw_spec = None;
    for line in source.lines().take(40) {
        let line = line.trim_end();
        if line.starts_with("_type:") || line.starts_with("__export_format:") {
            saw_export = Some(());
        }
        if line.starts_with("swagger:") {
            saw_spec = Some(Format::Swagger2);
        }
        if line.starts_with("openapi:") {
            saw_spec = Some(Format::OpenApi3);
        }
    }
    saw_export.map(|()| Format::InsomniaV5).or(saw_spec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_curl_command_is_recognised_first() {
        assert_eq!(
            detect("curl --request GET https://x.test"),
            Some(Format::Curl)
        );
    }

    #[test]
    fn a_v4_export_is_recognised_by_its_marker() {
        let source = r#"{"__export_format": 4, "resources": []}"#;
        assert_eq!(detect(source), Some(Format::InsomniaV4));
    }

    #[test]
    fn an_older_export_format_is_legacy_not_v4() {
        let source = r#"{"__export_format": 3, "resources": []}"#;
        assert_eq!(detect(source), Some(Format::InsomniaLegacy));
    }

    #[test]
    fn a_postman_collection_is_recognised_by_its_schema() {
        let source = r#"{"info": {"schema": "https://schema.getpostman.com/json/collection/v2.1.0/collection.json"}, "item": []}"#;
        assert_eq!(detect(source), Some(Format::Postman));
    }

    #[test]
    fn a_postman_environment_is_not_mistaken_for_a_collection() {
        let source = r#"{"values": [], "_postman_variable_scope": "environment"}"#;
        assert_eq!(detect(source), Some(Format::PostmanEnvironment));
    }

    #[test]
    fn a_har_is_recognised_by_its_log() {
        let source = r#"{"log": {"version": "1.2", "entries": []}}"#;
        assert_eq!(detect(source), Some(Format::Har));
    }

    #[test]
    fn swagger_and_openapi_are_distinguished() {
        assert_eq!(
            detect(r#"{"swagger": "2.0", "paths": {}}"#),
            Some(Format::Swagger2)
        );
        assert_eq!(
            detect(r#"{"openapi": "3.0.3", "paths": {}}"#),
            Some(Format::OpenApi3)
        );
    }

    #[test]
    fn a_v5_yaml_export_is_recognised() {
        assert_eq!(
            detect("_type: export\n__export_format: 5\n"),
            Some(Format::InsomniaV5)
        );
    }

    #[test]
    fn prose_matches_nothing() {
        assert_eq!(detect("this is a readme, not a request"), None);
    }

    #[test]
    fn only_insomnia_exports_are_lossless() {
        assert!(Format::InsomniaV5.is_lossless());
        assert!(!Format::Postman.is_lossless());
        assert!(!Format::Har.is_lossless());
    }
}
