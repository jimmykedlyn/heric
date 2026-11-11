//! Errors from the application core.

use heric_model::{Diagnostic, MessageKey, MessageValue};

/// Something a service could not do.
///
/// Wraps the leaf crates' errors rather than flattening them, because the UI
/// needs to know whether a failed send was a transport problem (retry), a
/// credential problem (open settings) or a template problem (fix the request), and
/// each of those comes from a different crate.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    /// The storage layer failed.
    #[error(transparent)]
    Storage(#[from] heric_storage::StorageError),

    /// The template renderer failed.
    #[error(transparent)]
    Template(#[from] heric_template::RenderError),

    /// The HTTP engine failed.
    #[error(transparent)]
    Http(#[from] heric_http::HttpError),

    /// Credential resolution failed.
    #[error(transparent)]
    Auth(#[from] heric_auth::AuthError),

    /// The script sandbox failed.
    #[error(transparent)]
    Script(#[from] heric_script::ScriptError),

    /// A protocol's transport failed.
    #[error(transparent)]
    Protocol(#[from] heric_protocols::ProtocolError),

    /// Import or export failed.
    #[error(transparent)]
    Import(#[from] heric_import::ImportError),

    /// OpenAPI parsing or conversion failed.
    #[error(transparent)]
    OpenApi(#[from] heric_openapi::OpenApiError),

    /// Snippet generation failed.
    #[error(transparent)]
    Codegen(#[from] heric_codegen::CodegenError),

    /// Git failed.
    #[error(transparent)]
    Git(#[from] heric_git::GitError),

    /// A document is not the kind the operation needs.
    #[error(transparent)]
    Model(#[from] heric_model::ModelError),

    /// Nothing is open.
    #[error("no workspace is open")]
    NoWorkspace,

    /// A run was cancelled.
    #[error("cancelled")]
    Cancelled,

    /// The runtime the core owns is not running.
    #[error("the core is shut down")]
    Shutdown,

    /// A background task panicked, which the core reports rather than aborting
    /// the process.
    #[error("a background task failed: {0}")]
    Task(String),
}

impl Diagnostic for CoreError {
    fn message_key(&self) -> MessageKey {
        match self {
            Self::Storage(error) => error.message_key(),
            Self::Template(error) => error.message_key(),
            Self::Http(error) => error.message_key(),
            Self::Auth(error) => error.message_key(),
            Self::Script(error) => error.message_key(),
            Self::Protocol(error) => error.message_key(),
            Self::Import(error) => error.message_key(),
            Self::OpenApi(error) => error.message_key(),
            Self::Codegen(error) => error.message_key(),
            Self::Git(error) => error.message_key(),
            Self::Model(_) => MessageKey::new("heric-core.model"),
            Self::NoWorkspace => MessageKey::new("heric-core.no-workspace"),
            Self::Cancelled => MessageKey::new("heric-core.cancelled"),
            Self::Shutdown => MessageKey::new("heric-core.shutdown"),
            Self::Task(_) => MessageKey::new("heric-core.task"),
        }
    }

    fn message_args(&self) -> Vec<(String, MessageValue)> {
        match self {
            Self::Storage(error) => error.message_args(),
            Self::Template(error) => error.message_args(),
            Self::Http(error) => error.message_args(),
            Self::Auth(error) => error.message_args(),
            Self::Script(error) => error.message_args(),
            Self::Protocol(error) => error.message_args(),
            Self::Import(error) => error.message_args(),
            Self::OpenApi(error) => error.message_args(),
            Self::Codegen(error) => error.message_args(),
            Self::Git(error) => error.message_args(),
            Self::Model(error) => vec![("detail".to_owned(), error.to_string().into())],
            Self::Task(detail) => vec![("detail".to_owned(), detail.as_str().into())],
            Self::NoWorkspace | Self::Cancelled | Self::Shutdown => Vec::new(),
        }
    }

    fn hint_key(&self) -> Option<MessageKey> {
        match self {
            Self::Storage(error) => error.hint_key(),
            Self::Template(error) => error.hint_key(),
            Self::Http(error) => error.hint_key(),
            Self::Auth(error) => error.hint_key(),
            Self::Script(error) => error.hint_key(),
            Self::Protocol(error) => error.hint_key(),
            Self::Import(error) => error.hint_key(),
            Self::OpenApi(error) => error.hint_key(),
            Self::Codegen(error) => error.hint_key(),
            Self::Git(error) => error.hint_key(),
            Self::NoWorkspace => Some(MessageKey::new("heric-core.hint.open-a-workspace")),
            _ => None,
        }
    }
}

/// Alias for symmetry with the other crates.
pub type CoreDiagnostic = CoreError;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_workspace_suggests_opening_one() {
        assert_eq!(
            CoreError::NoWorkspace.hint_key(),
            Some(MessageKey::new("heric-core.hint.open-a-workspace"))
        );
    }

    #[test]
    fn a_wrapped_error_keeps_the_inner_message_key() {
        let error = CoreError::Storage(heric_storage::StorageError::NotFound("req_1".to_owned()));
        assert_eq!(
            error.message_key(),
            MessageKey::new("heric-storage.not-found")
        );
    }

    #[test]
    fn a_task_failure_carries_its_detail() {
        let error = CoreError::Task("panicked".to_owned());
        let args = error.message_args();
        assert_eq!(args.len(), 1);
        assert_eq!(args[0].0, "detail");
    }
}
