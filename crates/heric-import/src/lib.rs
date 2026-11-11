//! Import and export.
//!
//! Reads every format the original reads and writes the three it writes. The
//! formats divide into two families, and the difference decides how much can be
//! preserved:
//!
//! - **Lossless**: Insomnia's own v4 JSON and v5 YAML. A round trip through
//!   Heric must be a fixed point once ids and timestamps are normalised
//!   (`STO-009`), which is the strongest correctness claim in this crate and is
//!   tested as one.
//! - **Lossy**: Postman, HAR, cURL, OpenAPI, Swagger and WSDL. These describe
//!   requests, not Insomnia's document model, so a conversion loses what the
//!   source never had. [`ImportReport::warnings`] records what was dropped, and
//!   never silently.
//!
//! Nothing here writes to the store: an import produces documents and a report,
//! and the caller decides whether to insert them.

#![deny(unsafe_code)]

mod curl;
mod error;
mod export;
mod format;
mod har;
mod import;
mod iteration;
mod report;
mod sanitize;

pub use curl::{CurlParse, parse_curl};
pub use error::{ImportDiagnostic, ImportError};
pub use export::{ExportFormat, ExportOptions, ExportedFile, export};
pub use format::{Format, detect};
pub use har::{HarEntry, HarLog};
pub use import::{ImportOptions, ImportResult, import, import_detected};
pub use iteration::{IterationData, IterationRow};
pub use report::{ImportReport, ImportWarning, LossWarning, RedactionMode};
pub use sanitize::{sanitize_filename, sanitize_title};

/// Returns the version of `heric-import`.
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Whether a source's format can be imported at all.
///
/// Used before showing a file picker's result, so the user is told "this is an
/// OpenAPI 2.0 document" rather than getting an error after choosing a
/// destination.
#[must_use]
pub fn is_supported(source: &str) -> bool {
    detect(source).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_semver_like() {
        assert_eq!(super::version().split('.').count(), 3);
    }

    #[test]
    fn an_unrecognised_source_is_not_supported() {
        assert!(!is_supported("just some prose, not a request"));
    }
}
