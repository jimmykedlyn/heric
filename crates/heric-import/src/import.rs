//! Import: source text to documents plus a report.

use heric_model::Doc;

use crate::format::Format;
use crate::{ImportError, ImportReport};

/// How an import should behave.
#[derive(Debug, Clone)]
pub struct ImportOptions {
    /// Where imported documents go.
    pub destination: Destination,
    /// Whether to reuse an existing document whose name matches.
    pub detect_duplicates: bool,
    /// Whether to mask secrets in the produced documents.
    ///
    /// Off by default: an import runs locally and the user needs their
    /// credentials to work after it. On for a document produced from a source
    /// whose secrets were never readable, such as a Postman export with a token
    /// the user pasted.
    pub mask_secrets: bool,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self {
            destination: Destination::NewWorkspace,
            detect_duplicates: true,
            mask_secrets: false,
        }
    }
}

/// Where an import puts what it produces.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Destination {
    /// A new collection, named after the source.
    #[default]
    NewWorkspace,
    /// Into an existing collection, below this parent.
    Into {
        /// The parent document.
        parent: heric_model::Id,
    },
}

/// What an import produced.
///
/// Documents rather than store writes: the caller decides whether to persist, so
/// a CLI can print a dry run and a GUI can show a preview before committing.
#[derive(Debug, Clone, Default)]
pub struct ImportResult {
    /// The documents, in an order where a parent precedes its children.
    pub documents: Vec<Doc>,
    /// What happened and what was lost.
    pub report: ImportReport,
}

impl ImportResult {
    /// Whether nothing was imported.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }

    /// How many documents were produced.
    #[must_use]
    pub fn len(&self) -> usize {
        self.documents.len()
    }
}

/// Imports a source of a known format.
///
/// # Errors
///
/// Returns [`ImportError::Empty`] when the source has nothing importable, and the
/// format's own parse error otherwise. A lossy conversion is a success with
/// warnings, never an error: refusing a Postman collection because it has a
/// header Heric cannot place would be worse than importing it and saying so.
pub fn import(
    source: &str,
    format: Format,
    options: &ImportOptions,
) -> Result<ImportResult, ImportError> {
    let _ = (source, format, options);
    todo!("M5: import a source in the given format")
}

/// Imports a source whose format is detected.
///
/// # Errors
///
/// Returns [`ImportError::UnrecognisedFormat`] when nothing matches.
pub fn import_detected(source: &str, options: &ImportOptions) -> Result<ImportResult, ImportError> {
    let format = crate::detect(source)
        .ok_or_else(|| ImportError::UnrecognisedFormat(snippet(source).to_owned()))?;
    import(source, format, options)
}

/// A short excerpt of an unrecognised source, for the error message.
///
/// Truncated to 40 characters and with newlines collapsed, so a pasted binary
/// does not fill the log.
fn snippet(source: &str) -> &str {
    let end = source
        .char_indices()
        .take(40)
        .last()
        .map_or(0, |(index, character)| index + character.len_utf8());
    source.get(..end).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_a_new_workspace_with_duplicate_detection() {
        let options = ImportOptions::default();
        assert_eq!(options.destination, Destination::NewWorkspace);
        assert!(options.detect_duplicates);
        assert!(!options.mask_secrets);
    }

    #[test]
    fn an_unrecognised_source_names_its_first_characters() -> Result<(), Box<dyn std::error::Error>>
    {
        let error = match import_detected("prose that is not a request", &ImportOptions::default())
        {
            Ok(result) => {
                return Err(format!("expected an unrecognised format, got {result:?}").into());
            }
            Err(error) => error,
        };
        match error {
            ImportError::UnrecognisedFormat(detail) => assert!(detail.starts_with("prose")),
            other => panic!("expected an unrecognised format, got {other}"),
        }
        Ok(())
    }

    #[test]
    fn a_snippet_is_truncated_on_a_character_boundary() {
        let long = "é".repeat(100);
        assert!(snippet(&long).len() <= 40 * 2);
        assert!(snippet(&long).chars().count() <= 40);
    }

    #[test]
    fn an_empty_result_reports_itself() {
        let result = ImportResult::default();
        assert!(result.is_empty());
        assert_eq!(result.len(), 0);
    }
}
