//! HTTP Archive import.

use heric_model::{Body, DocKind, Header, PreparedRequest};

/// A HAR file.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HarLog {
    /// The archive version, `1.2` for everything the original writes.
    pub version: String,
    /// The creator's name and version.
    pub creator: Option<String>,
    /// The entries, in document order.
    pub entries: Vec<HarEntry>,
}

impl HarLog {
    /// Whether the archive carries anything importable.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Turns each entry into a request.
    ///
    /// The original imports requests only, and uses an entry's `comment` as its
    /// name because a HAR has no name field (`IO-004`). An entry with no comment
    /// is named after its method and URL path, which is what a user reads in a
    /// list.
    #[must_use]
    pub fn into_requests(self) -> Vec<(String, PreparedRequest)> {
        self.entries
            .into_iter()
            .map(|entry| {
                let name = entry
                    .comment
                    .clone()
                    .filter(|comment| !comment.trim().is_empty())
                    .unwrap_or_else(|| default_name(&entry.request.method, &entry.request.url));
                let body = entry.request.body();
                (
                    name,
                    PreparedRequest {
                        request_id: heric_model::Id::generate(DocKind::Request),
                        kind: DocKind::Request,
                        method: entry.request.method.clone(),
                        url: entry.request.url.clone(),
                        headers: entry
                            .request
                            .headers
                            .iter()
                            .map(|header| Header::new(&header.name, &header.value))
                            .collect(),
                        body,
                    },
                )
            })
            .collect()
    }
}

fn default_name(method: &str, url: &str) -> String {
    let path = url
        .split("://")
        .nth(1)
        .and_then(|rest| rest.split_once('/'))
        .map(|(_, path)| path)
        .unwrap_or_default()
        .split('?')
        .next()
        .unwrap_or_default();
    if path.is_empty() {
        format!("{method} /")
    } else {
        format!("{method} /{path}")
    }
}

/// One archive entry.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HarEntry {
    /// When it happened, in ISO 8601.
    pub started_date_time: String,
    /// How long it took, in milliseconds.
    pub time: f64,
    /// The request.
    pub request: HarRequest,
    /// The response, when the archive recorded one.
    pub response: Option<HarResponse>,
    /// The entry's comment, used as the imported request's name.
    pub comment: Option<String>,
}

/// A request inside an entry.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HarRequest {
    /// The method.
    pub method: String,
    /// The URL.
    pub url: String,
    /// The HTTP version.
    pub http_version: String,
    /// Headers, in wire order and with duplicates kept.
    pub headers: Vec<HarNameValue>,
    /// Query string parameters.
    pub query_string: Vec<HarNameValue>,
    /// Cookies sent.
    pub cookies: Vec<HarNameValue>,
    /// The body.
    pub post_data: Option<HarPostData>,
}

impl HarRequest {
    /// The request's body, as Heric's body type.
    #[must_use]
    pub fn body(&self) -> Body {
        match &self.post_data {
            None => Body::Empty,
            Some(data) => {
                let content_type = self
                    .headers
                    .iter()
                    .find(|header| header.name.eq_ignore_ascii_case("content-type"))
                    .map(|header| header.value.clone());
                let text = data.text.clone().or_else(|| {
                    data.params.as_ref().map(|params| {
                        params
                            .iter()
                            .map(|param| format!("{}={}", param.name, param.value))
                            .collect::<Vec<_>>()
                            .join("&")
                    })
                });
                match (text, content_type) {
                    (None, _) => Body::Empty,
                    (Some(text), Some(content_type)) => Body::text(text, content_type),
                    (Some(text), None) => Body::Bytes {
                        bytes: text.into_bytes(),
                        content_type: None,
                    },
                }
            }
        }
    }
}

/// A response inside an entry.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HarResponse {
    /// The status.
    pub status: i64,
    /// The status text.
    pub status_text: String,
    /// Headers.
    pub headers: Vec<HarNameValue>,
    /// The body.
    pub content: Option<HarContent>,
}

/// A response body.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HarContent {
    /// Its size in bytes.
    pub size: i64,
    /// The MIME type.
    pub mime_type: String,
    /// The body, when the archive stored it.
    pub text: Option<String>,
    /// The encoding, `base64` when the body is not UTF-8.
    pub encoding: Option<String>,
}

/// The body of a request in an entry.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct HarPostData {
    /// The MIME type.
    pub mime_type: String,
    /// The body as text.
    pub text: Option<String>,
    /// The body as form parameters.
    pub params: Option<Vec<HarNameValue>>,
}

/// A name and value, HAR's shape for headers, parameters and cookies.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HarNameValue {
    /// The name.
    pub name: String,
    /// The value.
    pub value: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_uncommented_entry_is_named_after_its_path() {
        assert_eq!(
            default_name("GET", "https://api.example.test/v1/users?page=2"),
            "GET /v1/users"
        );
    }

    #[test]
    fn an_entry_for_a_bare_host_is_named_with_a_slash() {
        assert_eq!(default_name("GET", "https://api.example.test"), "GET /");
    }

    #[test]
    fn a_comment_becomes_the_name() {
        let log = HarLog {
            version: "1.2".to_owned(),
            creator: None,
            entries: vec![HarEntry {
                comment: Some("List users".to_owned()),
                request: HarRequest {
                    method: "GET".to_owned(),
                    url: "https://x.test/users".to_owned(),
                    ..HarRequest::default()
                },
                ..HarEntry::default()
            }],
        };
        let requests = log.into_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].0, "List users");
    }

    #[test]
    fn a_post_with_text_uses_the_content_type() {
        let request = HarRequest {
            headers: vec![HarNameValue {
                name: "Content-Type".to_owned(),
                value: "application/json".to_owned(),
            }],
            post_data: Some(HarPostData {
                mime_type: "application/json".to_owned(),
                text: Some("{}".to_owned()),
                params: None,
            }),
            ..HarRequest::default()
        };
        let body = request.body();
        assert_eq!(body.content_type(), Some("application/json"));
    }

    #[test]
    fn form_params_are_rebuilt_into_a_body() -> Result<(), Box<dyn std::error::Error>> {
        let request = HarRequest {
            post_data: Some(HarPostData {
                mime_type: "application/x-www-form-urlencoded".to_owned(),
                text: None,
                params: Some(vec![
                    HarNameValue {
                        name: "a".to_owned(),
                        value: "1".to_owned(),
                    },
                    HarNameValue {
                        name: "b".to_owned(),
                        value: "2".to_owned(),
                    },
                ]),
            }),
            ..HarRequest::default()
        };
        match request.body() {
            Body::Bytes { bytes, .. } => {
                assert_eq!(String::from_utf8(bytes)?, "a=1&b=2");
            }
            other => panic!("expected a body, got {other:?}"),
        }
        Ok(())
    }

    #[test]
    fn an_empty_archive_reports_itself() {
        assert!(HarLog::default().is_empty());
    }
}
