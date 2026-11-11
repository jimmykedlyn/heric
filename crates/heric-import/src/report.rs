//! What an import did, and what it lost.

/// How much of a value to write into an export or a CLI report.
///
/// The original's CLI offers three modes and its papertrail output defaults to
/// the redacted one (`IO-041`). Heric makes the mode explicit everywhere a value
/// leaves the process, so a script that pipes the output somewhere cannot leak a
/// token by accident.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RedactionMode {
    /// Replace every secret with a mask.
    #[default]
    Redact,
    /// Write values verbatim.
    Plaintext,
    /// Write the template tag back instead of the value.
    TemplateTag,
}

impl RedactionMode {
    /// The mode's name, as the CLI flag spells it.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Redact => "redact",
            Self::Plaintext => "plaintext",
            Self::TemplateTag => "templateTag",
        }
    }

    /// Whether a value may be written as it is.
    ///
    /// Part of the crate's public contract: a caller that writes a value out of
    /// the process asks the mode whether the value may appear verbatim.
    #[must_use]
    pub fn reveals(self) -> bool {
        matches!(self, Self::Plaintext)
    }
}

/// Something a conversion could not carry over.
///
/// Every one of these is shown to the user before the import is committed. A
/// silent loss is the failure mode this type exists to prevent: a Postman
/// pre-request script that Heic cannot translate must say so at import time, not
/// fail at send time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LossWarning {
    /// A script was dropped or translated only partly.
    Script {
        /// The request or collection it belonged to.
        location: String,
        /// Why.
        reason: String,
    },

    /// An authentication method has no equivalent.
    Auth {
        /// The request.
        location: String,
        /// The method named in the source.
        method: String,
    },

    /// A body type has no equivalent, such as a Postman GraphQL body.
    Body {
        /// The request.
        location: String,
        /// The body type.
        kind: String,
    },

    /// A field was dropped because the target format has no place for it.
    Field {
        /// The request.
        location: String,
        /// The field's name.
        field: String,
    },

    /// A test or assertion was dropped.
    Test {
        /// Where it was.
        location: String,
    },

    /// A value was masked, so the imported document needs it entered again.
    Secret {
        /// Where it was.
        location: String,
        /// What it was for.
        purpose: String,
    },
}

impl LossWarning {
    /// A one-line summary for the report list.
    #[must_use]
    pub fn summary(&self) -> String {
        match self {
            Self::Script { location, reason } => {
                format!("{location}: script not fully imported ({reason})")
            }
            Self::Auth { location, method } => {
                format!("{location}: authentication `{method}` has no equivalent")
            }
            Self::Body { location, kind } => {
                format!("{location}: body type `{kind}` was not imported")
            }
            Self::Field { location, field } => {
                format!("{location}: field `{field}` was dropped")
            }
            Self::Test { location } => format!("{location}: tests were not imported"),
            Self::Secret { location, purpose } => {
                format!("{location}: {purpose} must be entered again")
            }
        }
    }

    /// Where the loss happened, for grouping in the report.
    #[must_use]
    pub fn location(&self) -> &str {
        match self {
            Self::Script { location, .. }
            | Self::Auth { location, .. }
            | Self::Body { location, .. }
            | Self::Field { location, .. }
            | Self::Test { location }
            | Self::Secret { location, .. } => location,
        }
    }

    /// Whether the loss needs the user to act before the collection is usable.
    ///
    /// A dropped description does not; a dropped credential does.
    #[must_use]
    pub fn needs_action(&self) -> bool {
        matches!(self, Self::Secret { .. } | Self::Auth { .. })
    }
}

/// One warning with a severity the UI can colour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportWarning {
    /// What was lost.
    pub loss: LossWarning,
    /// Whether it blocks using the collection.
    pub blocking: bool,
}

/// The result of an import.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportReport {
    /// What the source was detected as.
    pub source_format: Option<&'static str>,
    /// How many documents were produced, by kind.
    pub counts: Vec<(&'static str, usize)>,
    /// What was lost in translation.
    pub warnings: Vec<ImportWarning>,
    /// Whether the source was lossless, so nothing could have been lost.
    pub lossless: bool,
}

impl ImportReport {
    /// An empty report for a lossless source.
    #[must_use]
    pub fn lossless(format: &'static str) -> Self {
        Self {
            source_format: Some(format),
            lossless: true,
            ..Self::default()
        }
    }

    /// Adds a loss.
    pub fn warn(&mut self, loss: LossWarning) {
        let blocking = loss.needs_action();
        self.warnings.push(ImportWarning { loss, blocking });
    }

    /// Adds a count for a document kind.
    pub fn count(&mut self, kind: &'static str, count: usize) {
        match self.counts.iter_mut().find(|(name, _)| *name == kind) {
            Some(entry) => entry.1 += count,
            None => self.counts.push((kind, count)),
        }
    }

    /// How many documents in total.
    #[must_use]
    pub fn total(&self) -> usize {
        self.counts.iter().map(|(_, count)| count).sum()
    }

    /// Whether anything was lost.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.warnings.is_empty()
    }

    /// Whether the user must do something before the collection works.
    #[must_use]
    pub fn needs_action(&self) -> bool {
        self.warnings.iter().any(|warning| warning.blocking)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lossless_import_has_no_warnings() {
        let report = ImportReport::lossless("Insomnia v5");
        assert!(report.is_clean());
        assert!(!report.needs_action());
        assert!(report.lossless);
        assert_eq!(report.source_format, Some("Insomnia v5"));
    }

    #[test]
    fn a_dropped_credential_needs_action_and_a_dropped_field_does_not() {
        let mut report = ImportReport::default();
        report.warn(LossWarning::Field {
            location: "GET /a".to_owned(),
            field: "x-internal".to_owned(),
        });
        assert!(!report.needs_action());
        report.warn(LossWarning::Secret {
            location: "GET /a".to_owned(),
            purpose: "the bearer token".to_owned(),
        });
        assert!(report.needs_action());
        assert_eq!(report.warnings.len(), 2);
    }

    #[test]
    fn counts_accumulate_per_kind() {
        let mut report = ImportReport::default();
        report.count("request", 3);
        report.count("request", 2);
        report.count("requestGroup", 1);
        assert_eq!(report.total(), 6);
        assert_eq!(report.counts.len(), 2);
    }

    #[test]
    fn every_warning_has_a_readable_summary_naming_its_location() {
        let warning = LossWarning::Auth {
            location: "GET /a".to_owned(),
            method: "awsv4".to_owned(),
        };
        let summary = warning.summary();
        assert!(summary.contains("GET /a"));
        assert!(summary.contains("awsv4"));
        assert_eq!(warning.location(), "GET /a");
    }

    #[test]
    fn redaction_names_match_the_cli_flags() {
        assert_eq!(RedactionMode::Redact.as_str(), "redact");
        assert!(RedactionMode::Plaintext.reveals());
        assert!(!RedactionMode::Redact.reveals());
        assert!(!RedactionMode::TemplateTag.reveals());
    }
}
