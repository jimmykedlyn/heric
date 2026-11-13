//! Borrowed reads over the documents the navigation tree and the environment
//! editor need.
//!
//! These are views in the same sense as [`crate::Request`]: they interpret the
//! JSON body and never mutate it. Everything here is infallible except where a
//! document's kind makes the question meaningless, and the shared helpers
//! (`midpoint`, `copy_name`) are pure functions with no document at all.

use serde_json::Value;

use crate::{Doc, ModelError};

/// A typed, borrowed view of a request group (a folder).
#[derive(Debug, Clone, Copy)]
pub struct RequestGroup<'a> {
    doc: &'a Doc,
}

/// A typed, borrowed view of an environment.
#[derive(Debug, Clone, Copy)]
pub struct Environment<'a> {
    doc: &'a Doc,
}

/// A typed, borrowed view of a project.
#[derive(Debug, Clone, Copy)]
pub struct Project<'a> {
    doc: &'a Doc,
}

/// A typed, borrowed view of a response.
#[derive(Debug, Clone, Copy)]
pub struct Response<'a> {
    doc: &'a Doc,
}

impl Doc {
    /// Interprets the document as a [`RequestGroup`].
    ///
    /// # Errors
    ///
    /// Returns [`ModelError::WrongKind`] when the document is not a request
    /// group.
    pub fn as_group(&self) -> Result<RequestGroup<'_>, ModelError> {
        if self.meta.kind != crate::DocKind::RequestGroup {
            return Err(ModelError::WrongKind {
                expected: crate::DocKind::RequestGroup,
                found: self.meta.kind,
            });
        }
        Ok(RequestGroup { doc: self })
    }

    /// Interprets the document as an [`Environment`].
    ///
    /// # Errors
    ///
    /// Returns [`ModelError::WrongKind`] when the document is not an
    /// environment.
    pub fn as_environment(&self) -> Result<Environment<'_>, ModelError> {
        if self.meta.kind != crate::DocKind::Environment {
            return Err(ModelError::WrongKind {
                expected: crate::DocKind::Environment,
                found: self.meta.kind,
            });
        }
        Ok(Environment { doc: self })
    }

    /// Interprets the document as a [`Project`].
    ///
    /// # Errors
    ///
    /// Returns [`ModelError::WrongKind`] when the document is not a project.
    pub fn as_project(&self) -> Result<Project<'_>, ModelError> {
        if self.meta.kind != crate::DocKind::Project {
            return Err(ModelError::WrongKind {
                expected: crate::DocKind::Project,
                found: self.meta.kind,
            });
        }
        Ok(Project { doc: self })
    }

    /// Interprets the document as a [`Response`].
    ///
    /// # Errors
    ///
    /// Returns [`ModelError::WrongKind`] when the document is not a response.
    pub fn as_response(&self) -> Result<Response<'_>, ModelError> {
        if self.meta.kind != crate::DocKind::Response {
            return Err(ModelError::WrongKind {
                expected: crate::DocKind::Response,
                found: self.meta.kind,
            });
        }
        Ok(Response { doc: self })
    }
}

impl<'a> RequestGroup<'a> {
    /// The underlying document.
    #[must_use]
    pub fn doc(&self) -> &'a Doc {
        self.doc
    }

    /// The group's name, empty when absent.
    #[must_use]
    pub fn name(&self) -> &str {
        self.doc.meta.name.as_deref().unwrap_or("")
    }

    /// Headers, as raw JSON.
    #[must_use]
    pub fn headers(&self) -> &[Value] {
        array_field(self.doc, "headers")
    }

    /// The group's authentication block, if any.
    #[must_use]
    pub fn authentication(&self) -> Option<&Value> {
        self.doc.body.get("authentication")
    }

    /// The pre-request script, if any.
    #[must_use]
    pub fn pre_request_script(&self) -> Option<&str> {
        self.doc
            .body
            .get("preRequestScript")
            .and_then(Value::as_str)
    }

    /// The after-response script, if any.
    #[must_use]
    pub fn after_response_script(&self) -> Option<&str> {
        self.doc
            .body
            .get("afterResponseScript")
            .and_then(Value::as_str)
    }
}

impl<'a> Environment<'a> {
    /// The underlying document.
    #[must_use]
    pub fn doc(&self) -> &'a Doc {
        self.doc
    }

    /// The environment's name, empty when absent.
    #[must_use]
    pub fn name(&self) -> &str {
        self.doc.meta.name.as_deref().unwrap_or("")
    }

    /// The variables as a JSON object.
    ///
    /// The original stores them as an object, so a document that carries one
    /// is returned as it is.
    #[must_use]
    pub fn data(&self) -> Option<&Value> {
        self.doc.body.get("data")
    }

    /// The sub-environments, empty when absent.
    #[must_use]
    pub fn sub_environments(&self) -> &[Value] {
        array_field(self.doc, "subEnvironments")
    }

    /// The colour, which is `null` when unset.
    #[must_use]
    pub fn color(&self) -> Option<&Value> {
        self.doc.body.get("color")
    }

    /// Whether this is the base environment of its workspace.
    ///
    /// The original decides this by `parentId === baseEnvironmentId` on the
    /// workspace, but a base environment also has no sub-environment parent and
    /// the well-known name, so the name is what a document can tell us on its
    /// own.
    #[must_use]
    pub fn is_base(&self) -> bool {
        self.name() == crate::environment::BASE_NAME
    }
}

impl<'a> Project<'a> {
    /// The underlying document.
    #[must_use]
    pub fn doc(&self) -> &'a Doc {
        self.doc
    }

    /// The project's name, empty when absent.
    #[must_use]
    pub fn name(&self) -> &str {
        self.doc.meta.name.as_deref().unwrap_or("")
    }

    /// The cloud identifier, `None` for a local or git project.
    #[must_use]
    pub fn remote_id(&self) -> Option<&str> {
        self.doc.body.get("remoteId").and_then(Value::as_str)
    }

    /// The git repository identifier, `None` for a non-git project.
    #[must_use]
    pub fn git_repository_id(&self) -> Option<&str> {
        self.doc.body.get("gitRepositoryId").and_then(Value::as_str)
    }
}

impl<'a> Response<'a> {
    /// The underlying document.
    #[must_use]
    pub fn doc(&self) -> &'a Doc {
        self.doc
    }

    /// The HTTP status code, when present and numeric.
    #[must_use]
    pub fn status_code(&self) -> Option<i64> {
        self.doc.body.get("statusCode").and_then(Value::as_i64)
    }

    /// The request this response answers.
    #[must_use]
    pub fn request_id(&self) -> Option<&str> {
        self.doc.body.get("requestId").and_then(Value::as_str)
    }

    /// The response body as text, when the body is text.
    #[must_use]
    pub fn body_text(&self) -> Option<&str> {
        self.doc.body.get("body").and_then(Value::as_str)
    }
}

/// A body field read as an array slice, empty when absent or not an array.
fn array_field<'a>(doc: &'a Doc, field: &str) -> &'a [Value] {
    doc.body
        .get(field)
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// The sort key that lands half-way between two neighbours.
///
/// The original's duplicate helpers all compute
/// `(anchor.metaSortKey + nextSortKey) / 2`, and every insert path uses the same
/// formula. The "next key or +100" fallback belongs to the caller, which is the
/// only place that knows whether there is a next sibling, so it is not here.
#[must_use]
pub fn midpoint(a: f64, b: f64) -> f64 {
    (a + b) / 2.0
}

/// The name a copy gets.
///
/// The original appends `" (Copy)"` unconditionally, with no check for an
/// existing suffix, so copying twice gives `"Foo (Copy) (Copy)"`. That reads
/// like a bug and is the original's behaviour, which is what a user's existing
/// documents already show.
#[must_use]
pub fn copy_name(name: &str) -> String {
    format!("{name} (Copy)")
}

/// The headers a request inherits from its ancestors, resolved.
///
/// This is the original's `getOrInheritHeaders`. `ancestors` is leaf-first
/// (nearest folder first) — the original documents `requestGroups` that way —
/// so it is reversed here before merging, letting a nearer ancestor win.
///
/// Rules, in order:
/// - a row with `disabled == true` is skipped, and so is one whose name is
///   blank;
/// - a single-value header (`content-type`, `host`, …) is *replaced* by the
///   closer scope rather than joined;
/// - any other repeated header is joined with `", "`, in the casing of the last
///   writer;
/// - the result is sorted by lowercased name.
#[must_use]
pub fn inherited_headers(request: &Doc, ancestors: &[Doc]) -> Vec<Value> {
    let mut contexts: Vec<&Doc> = ancestors.iter().rev().collect();
    contexts.push(request);

    // Lowercased name -> (original casing, value), as the original's
    // `originalCaseMap` does.
    let mut merged: std::collections::BTreeMap<String, (String, String)> =
        std::collections::BTreeMap::new();

    for doc in contexts {
        for row in array_field(doc, "headers") {
            let name = row.get("name").and_then(Value::as_str).unwrap_or("");
            if name.trim().is_empty() {
                continue;
            }
            if row.get("disabled").and_then(Value::as_bool) == Some(true) {
                continue;
            }
            let value = row.get("value").and_then(Value::as_str).unwrap_or("");
            let key = name.to_ascii_lowercase();
            let single = is_single_value_header(&key);
            match merged.get_mut(&key) {
                Some((casing, existing)) if !single => {
                    existing.push_str(", ");
                    existing.push_str(value);
                    *casing = name.to_owned();
                }
                Some((casing, existing)) => {
                    *casing = name.to_owned();
                    *existing = value.to_owned();
                }
                None => {
                    merged.insert(key, (name.to_owned(), value.to_owned()));
                }
            }
        }
    }

    // `BTreeMap` iterates in key order, and the keys are the lowercased names,
    // which is exactly the order the original's sort produces.
    merged
        .into_iter()
        .map(|(_, (name, value))| {
            let mut object = serde_json::Map::new();
            object.insert("name".to_owned(), Value::String(name));
            object.insert("value".to_owned(), Value::String(value));
            Value::Object(object)
        })
        .collect()
}

/// Headers that may appear only once, so the closest scope replaces rather than
/// joins. Transcribed from the original's `common-headers.ts`.
fn is_single_value_header(lowercased: &str) -> bool {
    matches!(
        lowercased,
        "proxy-authorization"
            | "content-length"
            | "content-type"
            | "content-encoding"
            | "content-location"
            | "connection"
            | "host"
            | "upgrade"
            | "range"
            | "trailer"
    )
}

/// The authentication a request inherits from its ancestors.
///
/// The request's own block wins when it is a usable one — a non-empty object
/// with a `type` and not disabled. Otherwise the nearest ancestor with a usable
/// block supplies it. `None` means "no auth", which is how Heric spells the
/// original's literal `{type: "none"}`.
///
/// A request that says `{"type": "none"}` is *not* overridden: that is a real
/// choice, not an absence.
#[must_use]
pub fn inherited_authentication(request: &Doc, ancestors: &[Doc]) -> Option<Value> {
    let mut contexts: Vec<&Doc> = vec![request];
    contexts.extend(ancestors.iter());

    contexts
        .into_iter()
        .filter_map(|doc| doc.body.get("authentication"))
        .find(|auth| is_usable_auth(auth))
        .cloned()
}

/// Whether an authentication block is something a request can use.
///
/// Mirrors the original's `getAuthObjectOrNull(...) && isAuthEnabled(...)`: not
/// null, not an empty object, with a `type`, and not explicitly disabled.
fn is_usable_auth(auth: &Value) -> bool {
    let Value::Object(map) = auth else {
        return false;
    };
    if map.is_empty() {
        return false;
    }
    if map.get("disabled").and_then(Value::as_bool) == Some(true) {
        return false;
    }
    map.get("type").and_then(Value::as_str).is_some()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{DocKind, Id, init};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    /// A document of `kind` carrying `body`.
    fn doc(kind: DocKind, id: &str, body: Value) -> Result<Doc, Box<dyn std::error::Error>> {
        let mut value = json!({"_id": id, "type": kind.as_str()});
        let Value::Object(map) = &mut value else {
            return Err("document must be an object".into());
        };
        if let Value::Object(fields) = body {
            for (key, field) in fields {
                map.insert(key, field);
            }
        }
        Ok(serde_json::from_value(value)?)
    }

    #[test]
    fn a_disabled_header_row_is_skipped_by_inheritance() -> TestResult {
        let folder = doc(
            DocKind::RequestGroup,
            "fld_1",
            json!({"headers": [
                {"name": "A", "value": "1"},
                {"name": "B", "value": "2", "disabled": true}
            ]}),
        )?;
        let request = doc(DocKind::Request, "req_1", json!({"headers": []}))?;
        let headers = inherited_headers(&request, &[folder]);
        assert_eq!(headers, vec![json!({"name": "A", "value": "1"})]);
        Ok(())
    }

    #[test]
    fn a_blank_name_is_skipped_by_inheritance() -> TestResult {
        let folder = doc(
            DocKind::RequestGroup,
            "fld_1",
            json!({"headers": [{"name": "   ", "value": "x"}]}),
        )?;
        let request = doc(DocKind::Request, "req_1", json!({"headers": []}))?;
        assert!(inherited_headers(&request, &[folder]).is_empty());
        Ok(())
    }

    #[test]
    fn a_repeated_header_is_joined_with_a_comma() -> TestResult {
        let folder = doc(
            DocKind::RequestGroup,
            "fld_1",
            json!({"headers": [{"name": "X-Trace", "value": "a"}]}),
        )?;
        let request = doc(
            DocKind::Request,
            "req_1",
            json!({"headers": [{"name": "X-Trace", "value": "b"}]}),
        )?;
        assert_eq!(
            inherited_headers(&request, &[folder]),
            vec![json!({"name": "X-Trace", "value": "a, b"})]
        );
        Ok(())
    }

    #[test]
    fn a_single_value_header_is_replaced_not_joined() -> TestResult {
        let folder = doc(
            DocKind::RequestGroup,
            "fld_1",
            json!({"headers": [{"name": "Content-Type", "value": "text/plain"}]}),
        )?;
        let request = doc(
            DocKind::Request,
            "req_1",
            json!({"headers": [{"name": "content-type", "value": "application/json"}]}),
        )?;
        assert_eq!(
            inherited_headers(&request, &[folder]),
            vec![json!({"name": "content-type", "value": "application/json"})]
        );
        Ok(())
    }

    #[test]
    fn inheritance_is_sorted_by_lowercased_name() -> TestResult {
        let request = doc(
            DocKind::Request,
            "req_1",
            json!({"headers": [
                {"name": "b", "value": "2"},
                {"name": "A", "value": "1"}
            ]}),
        )?;
        let headers = inherited_headers(&request, &[]);
        let names: Vec<&str> = headers
            .iter()
            .filter_map(|row| row.get("name").and_then(Value::as_str))
            .collect();
        assert_eq!(names, ["A", "b"]);
        Ok(())
    }

    #[test]
    fn a_folder_auth_block_is_inherited_only_when_the_request_has_none() -> TestResult {
        let folder = doc(
            DocKind::RequestGroup,
            "fld_1",
            json!({"authentication": {"type": "bearer", "token": "t"}}),
        )?;
        let empty = doc(DocKind::Request, "req_1", json!({"authentication": {}}))?;
        assert_eq!(
            inherited_authentication(&empty, std::slice::from_ref(&folder)),
            Some(json!({"type": "bearer", "token": "t"}))
        );

        let own = doc(
            DocKind::Request,
            "req_2",
            json!({"authentication": {"type": "basic", "username": "u"}}),
        )?;
        assert_eq!(
            inherited_authentication(&own, std::slice::from_ref(&folder)),
            Some(json!({"type": "basic", "username": "u"}))
        );
        Ok(())
    }

    #[test]
    fn an_explicit_none_auth_wins_over_inheritance() -> TestResult {
        let folder = doc(
            DocKind::RequestGroup,
            "fld_1",
            json!({"authentication": {"type": "bearer", "token": "t"}}),
        )?;
        let none = doc(
            DocKind::Request,
            "req_1",
            json!({"authentication": {"type": "none"}}),
        )?;
        assert_eq!(
            inherited_authentication(&none, std::slice::from_ref(&folder)),
            Some(json!({"type": "none"}))
        );
        Ok(())
    }

    #[test]
    fn no_auth_anywhere_is_none() -> TestResult {
        let request = doc(DocKind::Request, "req_1", json!({}))?;
        assert_eq!(inherited_authentication(&request, &[]), None);
        Ok(())
    }

    #[test]
    fn a_disabled_auth_block_is_not_inherited() -> TestResult {
        let folder = doc(
            DocKind::RequestGroup,
            "fld_1",
            json!({"authentication": {"type": "bearer", "disabled": true}}),
        )?;
        let request = doc(DocKind::Request, "req_1", json!({}))?;
        assert_eq!(inherited_authentication(&request, &[folder]), None);
        Ok(())
    }

    #[test]
    fn the_middle_sort_key_lands_between_its_neighbours() {
        assert!((midpoint(-10.0, 0.0) - -5.0).abs() < f64::EPSILON);
        assert!((midpoint(0.0, 10.0) - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_copy_is_named_with_the_suffix_once() {
        assert_eq!(copy_name("Foo"), "Foo (Copy)");
        assert_eq!(copy_name("Foo (Copy)"), "Foo (Copy) (Copy)");
    }

    #[test]
    fn the_four_views_reject_a_wrong_kind() -> TestResult {
        let request = doc(DocKind::Request, "req_1", json!({}))?;
        assert!(matches!(
            request.as_group(),
            Err(ModelError::WrongKind {
                expected: DocKind::RequestGroup,
                ..
            })
        ));
        assert!(matches!(
            request.as_environment(),
            Err(ModelError::WrongKind {
                expected: DocKind::Environment,
                ..
            })
        ));
        assert!(matches!(
            request.as_project(),
            Err(ModelError::WrongKind {
                expected: DocKind::Project,
                ..
            })
        ));
        assert!(matches!(
            request.as_response(),
            Err(ModelError::WrongKind {
                expected: DocKind::Response,
                ..
            })
        ));
        Ok(())
    }

    #[test]
    fn the_views_read_their_own_fields() -> TestResult {
        let group = doc(
            DocKind::RequestGroup,
            "fld_1",
            json!({"name": "Root Group", "headers": [{"name": "A", "value": "1"}]}),
        )?;
        let view = group.as_group()?;
        assert_eq!(view.name(), "Root Group");
        assert_eq!(view.headers().len(), 1);

        let response = doc(DocKind::Response, "res_1", json!({"statusCode": 204}))?;
        assert_eq!(response.as_response()?.status_code(), Some(204));
        Ok(())
    }

    #[test]
    fn a_base_environment_is_recognised_by_its_name() -> TestResult {
        let base = doc(
            DocKind::Environment,
            "env_1",
            json!({"name": crate::environment::BASE_NAME}),
        )?;
        assert!(base.as_environment()?.is_base());
        let other = doc(DocKind::Environment, "env_2", json!({"name": "Staging"}))?;
        assert!(!other.as_environment()?.is_base());
        Ok(())
    }

    #[test]
    fn an_initialised_request_has_no_inherited_headers_of_its_own() -> TestResult {
        // `init::document` gives a request an empty `headers` array, so
        // inheritance over it yields nothing rather than failing.
        let request = init::document(DocKind::Request, 0);
        assert!(inherited_headers(&request, &[]).is_empty());
        assert_eq!(request.meta.id, Id::new(request.meta.id.as_str()));
        Ok(())
    }
}
