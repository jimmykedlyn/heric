//! Export.

use crate::{ImportError, RedactionMode};
use heric_model::Doc;

/// A format Heric can write.
///
/// Deliberately shorter than [`crate::Format`]: the original writes its own two
/// export shapes plus HAR, and nothing else. A user who wants Postman can export
/// v5 and convert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    /// Insomnia v5 YAML, one file per collection.
    InsomniaV5,
    /// Insomnia v4 JSON, for the original's own importer and for interoperability.
    InsomniaV4,
    /// HTTP Archive 1.2.
    Har,
}

impl ExportFormat {
    /// The format's name, for the dialog.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InsomniaV5 => "Insomnia v5",
            Self::InsomniaV4 => "Insomnia v4",
            Self::Har => "HAR 1.2",
        }
    }

    /// The file extension.
    #[must_use]
    pub fn extension(self) -> &'static str {
        match self {
            Self::InsomniaV5 => "yaml",
            Self::InsomniaV4 => "json",
            Self::Har => "har",
        }
    }
}

/// How to write an export.
#[derive(Debug, Clone)]
pub struct ExportOptions {
    /// Which format.
    pub format: ExportFormat,
    /// How to treat secrets.
    pub redaction: RedactionMode,
    /// Only these documents, or every document when empty.
    ///
    /// The original takes a `requestIds` list and keeps every request group
    /// regardless (`IO-002`), because a request without its ancestors has no
    /// place in the tree.
    pub only: Vec<heric_model::Id>,
    /// Whether to include the environment.
    pub include_environment: bool,
}

impl ExportOptions {
    /// The defaults the export dialog starts from.
    #[must_use]
    pub fn new(format: ExportFormat) -> Self {
        Self {
            format,
            redaction: RedactionMode::Redact,
            only: Vec::new(),
            include_environment: true,
        }
    }
}

/// An exported file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportedFile {
    /// The file's name, already sanitised.
    pub name: String,
    /// Its contents.
    pub contents: String,
}

/// Writes documents in a format.
///
/// # Errors
///
/// Returns [`ImportError::Empty`] when there is nothing to write, and
/// [`ImportError::NotExportable`] when the documents use something the target
/// format cannot represent.
pub fn export(
    documents: &[Doc],
    options: &ExportOptions,
) -> Result<Vec<ExportedFile>, ImportError> {
    let _ = (documents, options);
    todo!("M5: export documents in the requested format")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_redact_secrets() {
        let options = ExportOptions::new(ExportFormat::InsomniaV5);
        assert_eq!(options.redaction, RedactionMode::Redact);
        assert!(options.include_environment);
        assert!(options.only.is_empty());
    }

    #[test]
    fn extensions_are_distinct() {
        assert_eq!(ExportFormat::InsomniaV5.extension(), "yaml");
        assert_eq!(ExportFormat::InsomniaV4.extension(), "json");
        assert_eq!(ExportFormat::Har.extension(), "har");
    }
}
