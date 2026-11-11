//! cURL import.
//!
//! A cURL command is the most common way a request arrives from outside: a bug
//! report, a colleague's terminal, a vendor's documentation page. The parser is
//! a flag whitelist rather than a shell parser, which is what the original does
//! (`IO-005`), and it matters: a command copied from a browser contains dozens of
//! connection flags Heric must ignore, and a shell parser would also execute the
//! quoting.

use heric_model::{Body, DocKind, Header, PreparedRequest};

/// What a cURL command line was parsed into.
#[derive(Debug, Clone, PartialEq)]
pub struct CurlParse {
    /// The request, ready to become a document.
    pub request: PreparedRequest,
    /// Flags the parser recognised but Heric does not model.
    pub ignored_flags: Vec<String>,
    /// Warnings, such as an `Authorization` flag overriding `-u`.
    pub warnings: Vec<String>,
}

/// Parses a cURL command line.
///
/// Returns `None` when the text is not a cURL command, which is the signal the
/// paste hook uses to leave the clipboard alone rather than replacing the user's
/// text with a misparse.
///
/// # Errors
///
/// Returns [`crate::ImportError::Parse`] when the text starts with `curl` but an
/// argument is malformed, such as a missing URL.
pub fn parse_curl(text: &str) -> Result<Option<CurlParse>, crate::ImportError> {
    let trimmed = text.trim();
    if !(trimmed.starts_with("curl ") || trimmed.starts_with("curl\t")) {
        return Ok(None);
    }

    let tokens = tokenize(trimmed)?;
    let mut method: Option<String> = None;
    let mut url: Option<String> = None;
    let mut headers: Vec<Header> = Vec::new();
    let mut body: Option<String> = None;
    let mut user: Option<String> = None;
    let mut ignored = Vec::new();
    let mut warnings = Vec::new();

    let mut index = 1;
    while index < tokens.len() {
        let token = &tokens[index];
        let (flag, inline_value) = match token.split_once('=') {
            Some((flag, value)) if flag.starts_with('-') => (flag, Some(value.to_owned())),
            _ => (token.as_str(), None),
        };
        let next = |index: &mut usize| -> Result<String, crate::ImportError> {
            if inline_value.is_some() {
                return Ok(inline_value.clone().unwrap_or_default());
            }
            *index += 1;
            tokens
                .get(*index)
                .cloned()
                .ok_or_else(|| crate::ImportError::Parse {
                    format: "cURL",
                    detail: format!("`{flag}` needs a value"),
                })
        };

        match flag {
            "-X" | "--request" => method = Some(next(&mut index)?),
            "-H" | "--header" => {
                let raw = next(&mut index)?;
                if let Some((name, value)) = raw.split_once(':') {
                    headers.push(Header::new(name.trim(), value.trim()));
                }
            }
            "-d" | "--data" | "--data-raw" | "--data-binary" | "--data-ascii" => {
                body = Some(next(&mut index)?);
            }
            "-u" | "--user" => user = Some(next(&mut index)?),
            "-A" | "--user-agent" => {
                let value = next(&mut index)?;
                headers.push(Header::new("User-Agent", value));
            }
            "-b" | "--cookie" => {
                let value = next(&mut index)?;
                headers.push(Header::new("Cookie", value));
            }
            "-e" | "--referer" => {
                let value = next(&mut index)?;
                headers.push(Header::new("Referer", value));
            }
            "-F" | "--form" => {
                return Err(crate::ImportError::Parse {
                    format: "cURL",
                    detail: "-F/--form is not imported yet; convert the command to use -d"
                        .to_owned(),
                });
            }
            // Flags that take a value but do not change the request Heric models.
            "--connect-timeout" | "--max-time" | "-o" | "--output" | "--cacert" | "--cert"
            | "--key" | "-x" | "--proxy" | "--resolve" | "--retry" => {
                let _ = next(&mut index)?;
                ignored.push(flag.to_owned());
            }
            // Flags with no value that Heric deliberately drops.
            "-s" | "--silent" | "-S" | "--show-error" | "-L" | "--location" | "-k"
            | "--insecure" | "-i" | "--include" | "-v" | "--verbose" | "--compressed" | "-g"
            | "--globoff" => ignored.push(flag.to_owned()),
            other if other.starts_with('-') => ignored.push(other.to_owned()),
            other => {
                if url.is_none() {
                    url = Some(other.to_owned());
                }
            }
        }
        index += 1;
    }

    let Some(url) = url else {
        return Err(crate::ImportError::Parse {
            format: "cURL",
            detail: "the command has no URL".to_owned(),
        });
    };

    // An explicit `Authorization` header beats `-u`, which is what the original's
    // table test asserts (FIX-003): a user who pastes both meant the header.
    if let Some(user) = user {
        let has_auth = headers
            .iter()
            .any(|header| header.name.eq_ignore_ascii_case("authorization"));
        if has_auth {
            warnings.push("`-u` was ignored because an Authorization header is present".to_owned());
        } else {
            let encoded = base64_encode(&user);
            headers.push(Header::new("Authorization", format!("Basic {encoded}")));
        }
    }

    let has_body = body.is_some();
    let method = method.unwrap_or_else(|| {
        if has_body {
            "POST".to_owned()
        } else {
            "GET".to_owned()
        }
    });

    let body = match body {
        None => Body::Empty,
        Some(text) => {
            let content_type = headers
                .iter()
                .find(|header| header.name.eq_ignore_ascii_case("content-type"))
                .map(|header| header.value.clone());
            if let Some(path) = text.strip_prefix('@') {
                Body::File {
                    path: std::path::PathBuf::from(path),
                    content_type,
                }
            } else {
                match content_type {
                    Some(content_type) => Body::text(text, content_type),
                    None => Body::Bytes {
                        bytes: text.into_bytes(),
                        content_type: None,
                    },
                }
            }
        }
    };

    Ok(Some(CurlParse {
        request: PreparedRequest {
            request_id: heric_model::Id::generate(DocKind::Request),
            kind: DocKind::Request,
            method: method.to_uppercase(),
            url,
            headers,
            body,
        },
        ignored_flags: ignored,
        warnings,
    }))
}

/// Splits a command line into tokens, honouring single and double quotes and
/// backslash escapes.
///
/// Written rather than shelled out, because running the user's text through a
/// shell to split it would execute it.
fn tokenize(input: &str) -> Result<Vec<String>, crate::ImportError> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;
    let mut started = false;

    for character in input.chars() {
        if escaped {
            current.push(character);
            escaped = false;
            started = true;
            continue;
        }
        match character {
            '\\' if quote != Some('\'') => escaped = true,
            '\'' | '"' => {
                if quote == Some(character) {
                    quote = None;
                } else if quote.is_none() {
                    quote = Some(character);
                    started = true;
                } else {
                    current.push(character);
                }
            }
            c if c.is_whitespace() && quote.is_none() => {
                if started || !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            c => {
                current.push(c);
                started = true;
            }
        }
    }

    if quote.is_some() {
        return Err(crate::ImportError::Parse {
            format: "cURL",
            detail: "a quote is not closed".to_owned(),
        });
    }
    if started || !current.is_empty() {
        tokens.push(current);
    }
    Ok(tokens)
}

/// Base64, for the `-u` to `Authorization` conversion.
///
/// A local encoder rather than a dependency: this is twelve lines and it keeps
/// the HTTP crate's dependency set out of the importer.
fn base64_encode(input: &str) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = input.as_bytes();
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(char::from(ALPHABET[((n >> 18) & 63) as usize]));
        out.push(char::from(ALPHABET[((n >> 12) & 63) as usize]));
        out.push(if chunk.len() > 1 {
            char::from(ALPHABET[((n >> 6) & 63) as usize])
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            char::from(ALPHABET[(n & 63) as usize])
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(command: &str) -> Result<CurlParse, Box<dyn std::error::Error>> {
        Ok(parse_curl(command)?.ok_or("not a cURL command")?)
    }

    #[test]
    fn prose_that_is_not_curl_returns_none() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(parse_curl("hello world")?, None);
        Ok(())
    }

    #[test]
    fn a_simple_get_is_parsed() -> Result<(), Box<dyn std::error::Error>> {
        let parsed = parsed("curl https://example.test/a")?;
        assert_eq!(parsed.request.method, "GET");
        assert_eq!(parsed.request.url, "https://example.test/a");
        assert_eq!(parsed.request.body, Body::Empty);
        Ok(())
    }

    #[test]
    fn a_data_flag_implies_post() -> Result<(), Box<dyn std::error::Error>> {
        let parsed = parsed("curl -d '{\"a\":1}' https://example.test/a")?;
        assert_eq!(parsed.request.method, "POST");
        assert!(matches!(parsed.request.body, Body::Bytes { .. }));
        Ok(())
    }

    #[test]
    fn an_explicit_method_wins_over_the_body_default() -> Result<(), Box<dyn std::error::Error>> {
        let parsed = parsed("curl -X PUT -d 'x' https://example.test/a")?;
        assert_eq!(parsed.request.method, "PUT");
        Ok(())
    }

    #[test]
    fn headers_are_split_on_the_first_colon() -> Result<(), Box<dyn std::error::Error>> {
        let parsed = parsed("curl -H 'Authorization: Bearer a:b:c' https://x.test")?;
        assert_eq!(parsed.request.headers.len(), 1);
        assert_eq!(parsed.request.headers[0].name, "Authorization");
        assert_eq!(parsed.request.headers[0].value, "Bearer a:b:c");
        Ok(())
    }

    #[test]
    fn user_becomes_a_basic_authorization_header() -> Result<(), Box<dyn std::error::Error>> {
        let parsed = parsed("curl -u 'me:s3cret' https://x.test")?;
        assert_eq!(parsed.request.headers.len(), 1);
        assert_eq!(parsed.request.headers[0].value, "Basic bWU6czNjcmV0");
        assert!(parsed.warnings.is_empty());
        Ok(())
    }

    #[test]
    fn an_explicit_authorization_header_beats_user() -> Result<(), Box<dyn std::error::Error>> {
        let parsed = parsed("curl -u 'me:s3cret' -H 'Authorization: Token abc' https://x.test")?;
        assert_eq!(parsed.request.headers.len(), 1);
        assert_eq!(parsed.request.headers[0].value, "Token abc");
        assert_eq!(parsed.warnings.len(), 1);
        Ok(())
    }

    #[test]
    fn connection_flags_are_recorded_as_ignored() -> Result<(), Box<dyn std::error::Error>> {
        let parsed = parsed("curl -s --compressed --max-time 5 https://x.test")?;
        assert!(parsed.ignored_flags.contains(&"-s".to_owned()));
        assert!(parsed.ignored_flags.contains(&"--compressed".to_owned()));
        assert!(parsed.ignored_flags.contains(&"--max-time".to_owned()));
        assert_eq!(parsed.request.url, "https://x.test");
        Ok(())
    }

    #[test]
    fn a_missing_url_is_an_error() {
        assert!(parse_curl("curl -s").is_err());
    }

    #[test]
    fn an_unclosed_quote_is_an_error() {
        assert!(parse_curl("curl -H 'oops https://x.test").is_err());
    }

    #[test]
    fn a_file_body_uses_the_at_prefix() -> Result<(), Box<dyn std::error::Error>> {
        let parsed = parsed("curl -d @payload.json https://x.test")?;
        assert!(matches!(parsed.request.body, Body::File { .. }));
        Ok(())
    }

    #[test]
    fn base64_matches_the_known_vectors() {
        assert_eq!(base64_encode(""), "");
        assert_eq!(base64_encode("f"), "Zg==");
        assert_eq!(base64_encode("fo"), "Zm8=");
        assert_eq!(base64_encode("foo"), "Zm9v");
        assert_eq!(base64_encode("foobar"), "Zm9vYmFy");
    }
}
