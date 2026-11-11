//! Template tag definitions and the tag host's error type.

use std::time::Duration;

use heric_model::{Diagnostic, MessageKey, MessageValue};

/// A tag this engine implements.
///
/// The list is data rather than code so the GUI can show which tags exist and
/// the docs can be generated from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TagDefinition {
    /// Tag name, without the delimiters.
    pub name: &'static str,
    /// One-line description.
    pub summary: &'static str,
    /// Whether the tag reaches outside the renderer.
    pub needs_host: bool,
    /// Whether the tag can only run during a send.
    pub send_only: bool,
}

/// Every tag the engine implements.
///
/// Superset of the original's local tags: `faker`, `base64`, `now`, `uuid`,
/// `os`, `hash`, `file`, `jsonpath`, `cookie`, `prompt`, `response`, `request`
/// and `timestamp`.
#[must_use]
pub fn tags() -> &'static [TagDefinition] {
    &TAGS
}

static TAGS: [TagDefinition; 13] = [
    TagDefinition {
        name: "base64",
        summary: "Encodes or decodes base64.",
        needs_host: false,
        send_only: false,
    },
    TagDefinition {
        name: "cookie",
        summary: "Reads a cookie from the workspace jar.",
        needs_host: true,
        send_only: false,
    },
    TagDefinition {
        name: "faker",
        summary: "Generates a fake value by category.",
        needs_host: false,
        send_only: false,
    },
    TagDefinition {
        name: "file",
        summary: "Reads a file from disk.",
        needs_host: true,
        send_only: false,
    },
    TagDefinition {
        name: "hash",
        summary: "Hashes a value with a named algorithm.",
        needs_host: false,
        send_only: false,
    },
    TagDefinition {
        name: "jsonpath",
        summary: "Extracts a value with a JSONPath expression.",
        needs_host: false,
        send_only: false,
    },
    TagDefinition {
        name: "now",
        summary: "Current time, with an offset and format.",
        needs_host: false,
        send_only: false,
    },
    TagDefinition {
        name: "os",
        summary: "Reads an environment variable or OS value.",
        needs_host: true,
        send_only: false,
    },
    TagDefinition {
        name: "prompt",
        summary: "Asks the user for a value.",
        needs_host: true,
        send_only: true,
    },
    TagDefinition {
        name: "request",
        summary: "Reads a field of the current request.",
        needs_host: false,
        send_only: false,
    },
    TagDefinition {
        name: "response",
        summary: "Reads a body field from another request's last response.",
        needs_host: true,
        send_only: false,
    },
    TagDefinition {
        name: "timestamp",
        summary: "Current Unix time, in seconds or milliseconds.",
        needs_host: false,
        send_only: false,
    },
    TagDefinition {
        name: "uuid",
        summary: "Generates a UUID.",
        needs_host: false,
        send_only: false,
    },
];

/// A failure inside a tag.
#[derive(Debug, thiserror::Error)]
pub enum TagError {
    /// The tag needs a person and this render may not ask.
    #[error("`{tag}` is only available while sending a request")]
    NotAvailableInThisPurpose {
        /// The tag's name.
        tag: &'static str,
    },

    /// The user did not answer in time.
    #[error("`{tag}` timed out after {waited:?}")]
    Timeout {
        /// The tag's name.
        tag: &'static str,
        /// How long it waited.
        waited: Duration,
    },

    /// The user dismissed the prompt.
    #[error("`{tag}` was cancelled")]
    Cancelled {
        /// The tag's name.
        tag: &'static str,
    },

    /// A path was outside the roots the host allows.
    #[error("path `{path}` is outside the allowed roots")]
    PathNotAllowed {
        /// The path as given.
        path: String,
    },

    /// A file could not be read.
    #[error("cannot read `{path}`: {detail}")]
    ReadFailed {
        /// The path as given.
        path: String,
        /// What went wrong.
        detail: String,
    },

    /// A request could not be executed.
    #[error("request failed: {0}")]
    SendFailed(String),

    /// The tag's arguments were malformed.
    #[error("invalid arguments for `{tag}`: {detail}")]
    BadArguments {
        /// The tag's name.
        tag: &'static str,
        /// What is wrong with them.
        detail: String,
    },
}

impl Diagnostic for TagError {
    fn message_key(&self) -> MessageKey {
        match self {
            Self::NotAvailableInThisPurpose { .. } => {
                MessageKey::new("heric-template.tag.not-available")
            }
            Self::Timeout { .. } => MessageKey::new("heric-template.tag.timeout"),
            Self::Cancelled { .. } => MessageKey::new("heric-template.tag.cancelled"),
            Self::PathNotAllowed { .. } => MessageKey::new("heric-template.tag.path-not-allowed"),
            Self::ReadFailed { .. } => MessageKey::new("heric-template.tag.read-failed"),
            Self::SendFailed(_) => MessageKey::new("heric-template.tag.send-failed"),
            Self::BadArguments { .. } => MessageKey::new("heric-template.tag.bad-arguments"),
        }
    }

    fn message_args(&self) -> Vec<(String, MessageValue)> {
        match self {
            Self::NotAvailableInThisPurpose { tag } | Self::Cancelled { tag } => {
                vec![("tag".to_owned(), (*tag).into())]
            }
            Self::Timeout { tag, waited } => vec![
                ("tag".to_owned(), (*tag).into()),
                ("seconds".to_owned(), (waited.as_secs() as i64).into()),
            ],
            Self::PathNotAllowed { path } | Self::ReadFailed { path, .. } => {
                vec![("path".to_owned(), path.as_str().into())]
            }
            Self::SendFailed(detail) => vec![("detail".to_owned(), detail.as_str().into())],
            Self::BadArguments { tag, detail } => vec![
                ("tag".to_owned(), (*tag).into()),
                ("detail".to_owned(), detail.as_str().into()),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tag_list_is_sorted_and_unique() {
        let mut names: Vec<&str> = tags().iter().map(|t| t.name).collect();
        let count = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), count);
    }

    #[test]
    fn only_prompt_is_send_only() {
        let send_only: Vec<&str> = tags()
            .iter()
            .filter(|t| t.send_only)
            .map(|t| t.name)
            .collect();
        assert_eq!(send_only, vec!["prompt"]);
    }

    #[test]
    fn host_tags_are_marked() {
        let names: Vec<&str> = tags()
            .iter()
            .filter(|t| t.needs_host)
            .map(|t| t.name)
            .collect();
        assert_eq!(names, vec!["cookie", "file", "os", "prompt", "response"]);
    }
}
