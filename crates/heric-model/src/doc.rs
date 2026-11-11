//! The document model, queries, patches and change notifications.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::error::ModelError;
use crate::{DocKind, Id};

/// The fields every document has, plus whatever else it carries.
///
/// The original's 44 types share six base fields and differ in everything
/// else. A Rust enum with 44 payload variants would be type-safe and would make
/// the migration reader brittle: a field the inventory missed would be dropped
/// silently, which is the project's main anti-goal. So the document keeps its
/// body as JSON and typed views read from it on demand.
///
/// Unknown keys survive a round trip. This is deliberate: Heric's own store
/// keeps what it was given, and the pruning of keys outside a model's defaults
/// belongs to the migration layer, where it reproduces the original's
/// `initModel` exactly.
#[derive(Debug, Clone, PartialEq)]
pub struct Doc {
    /// The fields shared by every document.
    pub meta: DocMeta,
    /// Everything else, verbatim.
    pub body: serde_json::Map<String, Value>,
}

/// The keys [`Doc`] owns. Everything else in a document belongs to the body.
const META_KEYS: [&str; 7] = [
    "_id",
    "type",
    "parentId",
    "modified",
    "created",
    "isPrivate",
    "name",
];

impl serde::Serialize for Doc {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;

        let meta = serde_json::to_value(&self.meta).map_err(serde::ser::Error::custom)?;
        let serde_json::Value::Object(meta) = meta else {
            return Err(serde::ser::Error::custom(
                "doc meta must serialise to an object",
            ));
        };

        let mut map = serializer.serialize_map(Some(meta.len() + self.body.len()))?;
        for (key, value) in &meta {
            map.serialize_entry(key, value)?;
        }
        for (key, value) in &self.body {
            // A body key that shadows a base field would emit a duplicate key,
            // and the base field is the one that means something.
            if !META_KEYS.contains(&key.as_str()) {
                map.serialize_entry(key, value)?;
            }
        }
        map.end()
    }
}

impl<'de> serde::Deserialize<'de> for Doc {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = <Value as serde::Deserialize>::deserialize(deserializer)?;
        let Value::Object(mut body) = value else {
            return Err(serde::de::Error::custom("a document must be a JSON object"));
        };

        // `DocMeta` ignores unknown fields, so it reads the whole document and
        // the remaining keys become the body.
        let meta: DocMeta = serde_json::from_value(Value::Object(body.clone()))
            .map_err(serde::de::Error::custom)?;
        for key in META_KEYS {
            body.remove(key);
        }
        Ok(Self { meta, body })
    }
}

/// The fields every document in the original shares.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DocMeta {
    /// The identifier, serialised as `_id`.
    #[serde(rename = "_id")]
    pub id: Id,
    /// The entity type, serialised as `type`.
    #[serde(rename = "type")]
    pub kind: DocKind,
    /// The enclosing document, or `None` at the root.
    ///
    /// The original's schema declares this required and its runtime default is
    /// `null`, so it is optional here.
    #[serde(default, rename = "parentId", skip_serializing_if = "Option::is_none")]
    pub parent: Option<Id>,
    /// Last modification, in Unix milliseconds.
    #[serde(default)]
    pub modified: i64,
    /// Creation time, in Unix milliseconds.
    #[serde(default)]
    pub created: i64,
    /// Whether the document is excluded from sync.
    ///
    /// The original has no runtime default: the field exists only when a
    /// model's `init()` sets it or a caller patches it. `None` means absent,
    /// which is different from `false`.
    #[serde(default, rename = "isPrivate", skip_serializing_if = "Option::is_none")]
    pub private: Option<bool>,
    /// Display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl Doc {
    /// Returns the document's kind.
    #[must_use]
    pub fn kind(&self) -> DocKind {
        self.meta.kind
    }

    /// Returns the document's id.
    #[must_use]
    pub fn id(&self) -> &Id {
        &self.meta.id
    }

    /// Reads a field from the body.
    #[must_use]
    pub fn get(&self, field: &str) -> Option<&Value> {
        self.body.get(field)
    }

    /// Interprets the document as a [`Request`].
    ///
    /// # Errors
    ///
    /// Returns [`ModelError::WrongKind`] when the document is not a request.
    pub fn as_request(&self) -> Result<Request<'_>, ModelError> {
        if self.meta.kind != DocKind::Request {
            return Err(ModelError::WrongKind {
                expected: DocKind::Request,
                found: self.meta.kind,
            });
        }
        Ok(Request { doc: self })
    }

    /// Interprets the document as a [`Workspace`].
    ///
    /// # Errors
    ///
    /// Returns [`ModelError::WrongKind`] when the document is not a workspace.
    pub fn as_workspace(&self) -> Result<Workspace<'_>, ModelError> {
        if self.meta.kind != DocKind::Workspace {
            return Err(ModelError::WrongKind {
                expected: DocKind::Workspace,
                found: self.meta.kind,
            });
        }
        Ok(Workspace { doc: self })
    }
}

/// A typed, borrowed view of a request document.
///
/// Fields stay in the document's JSON body; the view only interprets them, so
/// reading never copies and never normalises. Writes go through [`Patch`].
#[derive(Debug, Clone, Copy)]
pub struct Request<'a> {
    doc: &'a Doc,
}

impl<'a> Request<'a> {
    /// The underlying document.
    #[must_use]
    pub fn doc(&self) -> &'a Doc {
        self.doc
    }

    /// The request URL, empty when absent.
    #[must_use]
    pub fn url(&self) -> &str {
        self.doc
            .body
            .get("url")
            .and_then(Value::as_str)
            .unwrap_or("")
    }

    /// The HTTP method.
    ///
    /// The original's schema requires this field and its `init()` defaults it
    /// to `GET`, so an absent method is a malformed document rather than a
    /// request that happens to use GET.
    ///
    /// # Errors
    ///
    /// Returns [`ModelError::MissingField`] when the field is absent or is not
    /// a string.
    pub fn method(&self) -> Result<&str, ModelError> {
        self.doc
            .body
            .get("method")
            .and_then(Value::as_str)
            .ok_or(ModelError::MissingField { field: "method" })
    }

    /// The request body object, if present.
    #[must_use]
    pub fn body(&self) -> Option<&Value> {
        self.doc.body.get("body")
    }

    /// Headers, as raw JSON.
    ///
    /// Raw rather than typed because a header value may carry template syntax
    /// and a disabled header carries no value at all; the renderer decides.
    #[must_use]
    pub fn headers(&self) -> &[Value] {
        self.doc
            .body
            .get("headers")
            .and_then(Value::as_array)
            .map_or(&[], Vec::as_slice)
    }

    /// Parameters, as raw JSON.
    #[must_use]
    pub fn parameters(&self) -> &[Value] {
        self.doc
            .body
            .get("parameters")
            .and_then(Value::as_array)
            .map_or(&[], Vec::as_slice)
    }

    /// The authentication block, as raw JSON.
    ///
    /// The original's union falls back to an empty object, so an unrecognised
    /// auth block is valid data and must not fail a read.
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

/// A typed, borrowed view of a workspace document.
#[derive(Debug, Clone, Copy)]
pub struct Workspace<'a> {
    doc: &'a Doc,
}

impl<'a> Workspace<'a> {
    /// The underlying document.
    #[must_use]
    pub fn doc(&self) -> &'a Doc {
        self.doc
    }

    /// The workspace name, empty when absent.
    #[must_use]
    pub fn name(&self) -> &str {
        self.doc.meta.name.as_deref().unwrap_or("")
    }

    /// Whether this is the scratchpad workspace.
    ///
    /// The scratchpad has the fixed id `wrk_scratchpad` in the original, so
    /// this is an id comparison rather than a stored flag.
    #[must_use]
    pub fn is_scratchpad(&self) -> bool {
        self.doc.meta.id.as_str() == "wrk_scratchpad"
    }
}

/// One field change.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum PatchOp {
    /// Set the field to this value.
    Set(Value),
    /// Remove the field.
    ///
    /// The original represents removal by setting the key to `undefined`,
    /// which JSON cannot express, so the sentinel has to be explicit here.
    Unset,
}

/// A set of field changes against one document.
///
/// Mutations are patches, never whole-document replacements: the original's
/// `docUpdate` merges patches and re-stamps `modified`, and concurrent edits
/// from the autosave buffer and from a script must not clobber each other.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Patch {
    /// Field changes, keyed by field name.
    pub fields: BTreeMap<String, PatchOp>,
}

impl Patch {
    /// An empty patch.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets a field.
    #[must_use]
    pub fn set(mut self, field: impl Into<String>, value: Value) -> Self {
        self.fields.insert(field.into(), PatchOp::Set(value));
        self
    }

    /// Removes a field.
    #[must_use]
    pub fn unset(mut self, field: impl Into<String>) -> Self {
        self.fields.insert(field.into(), PatchOp::Unset);
        self
    }

    /// Whether the patch changes nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
}

/// How to sort a query's results.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Order {
    /// Ascending by `created`, which is the original's default when no sort is
    /// specified.
    #[default]
    Created,
    /// Descending by `modified`.
    ModifiedDesc,
}

/// A document query.
///
/// The original has no secondary indexes and relies on full scans filtered by
/// `parentId`, so a query is a parent filter plus an optional kind filter.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Query {
    /// Restrict to documents with this parent.
    pub parent: Option<Id>,
    /// Restrict to documents of this kind.
    pub kind: Option<DocKind>,
    /// Sort order.
    pub order: Order,
    /// Maximum number of results.
    pub limit: Option<usize>,
}

impl Query {
    /// All documents.
    #[must_use]
    pub fn all() -> Self {
        Self::default()
    }

    /// Documents with the given parent.
    #[must_use]
    pub fn children_of(parent: impl Into<Id>) -> Self {
        Self {
            parent: Some(parent.into()),
            ..Self::default()
        }
    }

    /// Documents of the given kind.
    #[must_use]
    pub fn of_kind(kind: DocKind) -> Self {
        Self {
            kind: Some(kind),
            ..Self::default()
        }
    }
}

/// What happened to the store.
///
/// Mirrors the original's `db.changes` broadcast, which the tab bar depends on:
/// deleting a request closes its tab.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Change {
    /// A document was created.
    Created(Doc),
    /// A document was updated in place.
    Updated(Doc),
    /// Documents were removed, including descendants.
    Removed(Vec<Id>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request_doc() -> Result<Doc, serde_json::Error> {
        serde_json::from_value(json!({
            "_id": "req_1",
            "type": "Request",
            "parentId": "wrk_1",
            "modified": 1,
            "created": 0,
            "name": "Get users",
            "url": "https://example.com/users",
            "method": "GET",
            "headers": [{"name": "Accept", "value": "application/json"}],
            "unknownField": 42
        }))
    }

    #[test]
    fn unknown_keys_survive_a_round_trip() -> Result<(), serde_json::Error> {
        let doc = request_doc()?;
        let text = serde_json::to_string(&doc)?;
        let back: Doc = serde_json::from_str(&text)?;
        assert_eq!(back.get("unknownField"), Some(&json!(42)));
        Ok(())
    }

    #[test]
    fn absent_private_is_not_false() -> Result<(), serde_json::Error> {
        let doc = request_doc()?;
        assert_eq!(doc.meta.private, None);
        Ok(())
    }

    #[test]
    fn typed_view_reads_the_body() -> Result<(), Box<dyn std::error::Error>> {
        let doc = request_doc()?;
        let req = doc.as_request()?;
        assert_eq!(req.url(), "https://example.com/users");
        assert_eq!(req.method()?, "GET");
        assert_eq!(req.headers().len(), 1);
        Ok(())
    }

    #[test]
    fn wrong_view_is_an_error() -> Result<(), Box<dyn std::error::Error>> {
        let doc = request_doc()?;
        assert!(matches!(
            doc.as_workspace(),
            Err(ModelError::WrongKind {
                expected: DocKind::Workspace,
                ..
            })
        ));
        Ok(())
    }

    #[test]
    fn a_missing_method_is_an_error() -> Result<(), Box<dyn std::error::Error>> {
        let doc: Doc = serde_json::from_value(json!({
            "_id": "req_2", "type": "Request"
        }))?;
        let req = doc.as_request()?;
        assert!(matches!(
            req.method(),
            Err(ModelError::MissingField { field: "method" })
        ));
        Ok(())
    }

    #[test]
    fn patches_accumulate() {
        let patch = Patch::new()
            .set("url", json!("https://example.com"))
            .unset("description");
        assert_eq!(patch.fields.len(), 2);
        assert!(!patch.is_empty());
    }
}
