//! The port for tags that reach outside the renderer.

use std::path::Path;
use std::time::Duration;

use heric_model::{Execution, PreparedRequest};

use crate::RenderPurpose;
use crate::tag::TagError;

/// Why a tag is reading a file.
///
/// The original refuses a file read that would escape the allowed roots, and
/// the purpose decides which roots are legal: an export may read a template
/// fixture, a send may read a certificate, neither may read a key it was not
/// asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadPurpose {
    /// A `{% file %}` tag in a request.
    RequestBody,
    /// A `{% file %}` tag in a script.
    Script,
    /// A certificate or key the engine loads.
    Transport,
}

/// A request for a value from the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptRequest {
    /// The request id this prompt belongs to, so a late answer can be matched.
    pub request_id: Option<heric_model::Id>,
    /// The question.
    pub title: String,
    /// Optional explanation.
    pub description: Option<String>,
    /// A default value the user can accept.
    pub default_value: Option<String>,
    /// Whether the answer must be masked while typing.
    pub secret: bool,
    /// How long to wait before giving up.
    pub timeout: Duration,
}

/// The user's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptAnswer {
    /// The value the user supplied.
    Value(String),
    /// The user dismissed the prompt.
    Cancelled,
}

/// A duration, kept in the signature so the host's timeout is part of the
/// contract rather than a hidden constant.
pub type Timeout = Duration;

/// The capabilities the template tags need from the outside world.
///
/// Implemented by `heric-core`, never by this crate: rendering a preview must
/// not open a socket or touch the filesystem except through what the host
/// permits.
pub trait TagHost {
    /// Reads a file.
    ///
    /// # Errors
    ///
    /// Returns [`TagError`] when the path is outside the allowed roots, does
    /// not exist, or exceeds the size limit.
    fn read_file(&self, path: &Path, purpose: ReadPurpose) -> Result<Vec<u8>, TagError>;

    /// Asks the user for a value.
    ///
    /// # Errors
    ///
    /// Returns [`TagError::NotAvailableInThisPurpose`] outside a send, and
    /// [`TagError::Timeout`] when the user does not answer in time, so a
    /// headless run or a closed window cannot hang a request.
    fn prompt(&self, request: &PromptRequest) -> Result<PromptAnswer, TagError>;

    /// Sends a request, as `{% response %}` does.
    ///
    /// # Errors
    ///
    /// Returns [`TagError`] when the engine cannot execute the request.
    fn send(&self, request: &PreparedRequest) -> Result<Execution, TagError>;

    /// Which purpose this render is for, so a host can refuse early.
    fn purpose(&self) -> RenderPurpose;
}
