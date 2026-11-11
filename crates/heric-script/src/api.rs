//! The `insomnia.*` API surface, as types.
//!
//! These describe what a script sees. They are not the JS bindings: the bindings
//! are installed by the sandbox and read their data from here, which is what
//! keeps the API's shape testable without a JavaScript engine.

use heric_model::Id;

/// `insomnia.request`, in an after-response script.
///
/// Read-only in that phase: the original lets a script read what was sent but
/// not rewrite history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestApi {
    /// The method as sent.
    pub method: String,
    /// The URL as sent.
    pub url: String,
    /// Headers as sent.
    pub headers: Vec<(String, String)>,
    /// Query parameters as sent.
    pub parameters: Vec<(String, String)>,
    /// The body as sent.
    pub body: Option<String>,
    /// How many bytes the body is.
    pub size: usize,
}

/// `insomnia.response`.
#[derive(Debug, Clone, PartialEq)]
pub struct ResponseApi {
    /// The status code.
    pub code: u16,
    /// The status message.
    pub status: String,
    /// Response headers.
    pub headers: Vec<(String, String)>,
    /// Cookies the response set.
    pub cookies: Vec<(String, String)>,
    /// The body, or `None` when it went to a file and was not materialised.
    pub body: Option<Vec<u8>>,
    /// The content type, parsed from the headers.
    pub content_type: Option<String>,
    /// How long the exchange took, in milliseconds.
    pub response_time: i64,
    /// The request that produced it.
    pub request: RequestApi,
}

impl ResponseApi {
    /// The body as text, lossily for bytes that are not UTF-8.
    #[must_use]
    pub fn text(&self) -> Option<std::borrow::Cow<'_, str>> {
        self.body
            .as_ref()
            .map(|bytes| String::from_utf8_lossy(bytes))
    }

    /// The body parsed as JSON.
    ///
    /// # Errors
    ///
    /// Returns the parse error, which the script sees as a thrown value, matching
    /// the original's `response.json()`.
    pub fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        match &self.body {
            Some(bytes) => serde_json::from_slice(bytes),
            None => serde_json::from_str("null"),
        }
    }
}

/// `insomnia.test` and its assertion helpers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestsApi {
    /// The tests registered with `insomnia.test`.
    pub names: Vec<String>,
}

/// `insomnia.cookies`, the synchronous list view.
///
/// The original also offers a callback-style `jar()` API over the same data,
/// because the jar lives behind an IPC bridge. Heric's jar is in-process, so both
/// shapes read the same source (`SCR-016`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Cookies {
    /// Cookies currently in the jar.
    pub entries: Vec<CookieView>,
}

/// One cookie as a script sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CookieView {
    /// Name.
    pub name: String,
    /// Value.
    pub value: String,
    /// Domain.
    pub domain: String,
    /// Path.
    pub path: String,
    /// Whether it was set by the user rather than by a server.
    pub manual: bool,
}

/// The id a script can use to look a document up.
///
/// A thin wrapper so a script's `insomnia.parentFolders` result has a declared
/// type rather than a bare string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderRef {
    /// The folder's id.
    pub id: Id,
    /// Its name.
    pub name: String,
}
