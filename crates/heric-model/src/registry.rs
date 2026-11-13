//! Per-kind capabilities, and rewriting references after a duplicate.
//!
//! The original declares two booleans on every model — `canDuplicate` and
//! `canSync` — and a duplicating operation that walks the copies and rewrites
//! the ids they reference. This module carries the capability table verbatim
//! and the reference rewriting; the cascading duplicate itself is a
//! storage-level operation over a query, so it lives with the store.
//!
//! The table has one asymmetry: [`DocKind::ProjectLintRuleset`] syncs without
//! being duplicable, so neither column may be derived from the other.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::{Doc, DocKind, Id};

/// Whether each kind may be duplicated, and whether it synchronises.
///
/// One table, two readers, so the columns cannot drift. Every
/// [`DocKind::ALL`] entry appears exactly once; a new kind must be added here,
/// and a test fails until it is.
#[rustfmt::skip]
const CAPABILITIES: [(DocKind, bool, bool); 44] = [
    (DocKind::ApiSpec,            true,  true),
    (DocKind::CaCertificate,      true,  false),
    (DocKind::ClientCertificate,  true,  false),
    (DocKind::CloudCredential,    false, false),
    (DocKind::CookieJar,          true,  false),
    (DocKind::Environment,        true,  true),
    (DocKind::GitCredentials,     false, false),
    (DocKind::GitRepository,      false, false),
    (DocKind::GrpcRequest,        true,  true),
    (DocKind::GrpcRequestMeta,    false, false),
    (DocKind::McpPayload,         true,  false),
    (DocKind::McpRequest,         true,  true),
    (DocKind::McpResponse,        false, false),
    (DocKind::MockRoute,          true,  true),
    (DocKind::MockServer,         true,  true),
    (DocKind::OAuth2Token,        false, false),
    (DocKind::PluginData,         false, false),
    (DocKind::Project,            false, false),
    (DocKind::ProjectLintRuleset, false, true),
    (DocKind::ProtoDirectory,     true,  true),
    (DocKind::ProtoFile,          true,  true),
    (DocKind::Request,            true,  true),
    (DocKind::RequestGroup,       true,  true),
    (DocKind::RequestGroupMeta,   false, false),
    (DocKind::RequestMeta,        false, false),
    (DocKind::RequestVersion,     false, false),
    (DocKind::Response,           false, false),
    (DocKind::RunnerTestResult,   false, false),
    (DocKind::Settings,           false, false),
    (DocKind::SocketIOPayload,    true,  true),
    (DocKind::SocketIORequest,    true,  true),
    (DocKind::SocketIORequestMeta, false, false),
    (DocKind::SocketIOResponse,   false, false),
    (DocKind::Stats,              false, false),
    (DocKind::UnitTest,           true,  true),
    (DocKind::UnitTestResult,     false, false),
    (DocKind::UnitTestSuite,      true,  true),
    (DocKind::UserSession,        false, false),
    (DocKind::WebSocketPayload,   true,  true),
    (DocKind::WebSocketRequest,   true,  true),
    (DocKind::WebSocketRequestMeta, false, false),
    (DocKind::WebSocketResponse,  false, false),
    (DocKind::Workspace,          true,  true),
    (DocKind::WorkspaceMeta,      false, false),
];

/// The kinds that synchronise, in [`DocKind`] declaration order.
///
/// This repeats the `sync` column of [`CAPABILITIES`] as a literal so
/// [`kinds_that_sync`] can hand out a `&'static [DocKind]` without a lock. A
/// test checks the two agree.
#[rustfmt::skip]
const SYNC_KINDS: [DocKind; 18] = [
    DocKind::ApiSpec,
    DocKind::Environment,
    DocKind::GrpcRequest,
    DocKind::McpRequest,
    DocKind::MockRoute,
    DocKind::MockServer,
    DocKind::ProjectLintRuleset,
    DocKind::ProtoDirectory,
    DocKind::ProtoFile,
    DocKind::Request,
    DocKind::RequestGroup,
    DocKind::SocketIOPayload,
    DocKind::SocketIORequest,
    DocKind::UnitTest,
    DocKind::UnitTestSuite,
    DocKind::WebSocketPayload,
    DocKind::WebSocketRequest,
    DocKind::Workspace,
];

/// The body fields a duplicated request has its references rewritten in.
const REQUEST_FIELDS: [&str; 8] = [
    "url",
    "body",
    "parameters",
    "pathParameters",
    "headers",
    "authentication",
    "preRequestScript",
    "afterResponseScript",
];

/// The body fields a duplicated folder has its references rewritten in.
const REQUEST_GROUP_FIELDS: [&str; 6] = [
    "authentication",
    "headers",
    "preRequestScript",
    "afterResponseScript",
    "environment",
    "kvPairData",
];

/// Returns whether a document of `kind` may be duplicated.
#[must_use]
pub fn can_duplicate(kind: DocKind) -> bool {
    CAPABILITIES
        .iter()
        .any(|&(other, duplicate, _)| other == kind && duplicate)
}

/// Returns the kinds that synchronise to a remote, in [`DocKind`] declaration
/// order.
#[must_use]
pub fn kinds_that_sync() -> &'static [DocKind] {
    &SYNC_KINDS
}

/// Rewrites the references a document makes to other documents, in place.
///
/// Every id in `mapping` is substituted for its counterpart. The substitution
/// runs over each field's serialised JSON text rather than over individual
/// strings, so an id used as an object key or embedded in a longer string — a
/// template tag, a response reference — is replaced too. That is what the
/// original does, and it is the point of the operation.
///
/// A request also has `konnectRouteKey` set to `null` and a folder
/// `konnectRouteId`, unconditionally: the original's object spread re-emits
/// those keys even when the copy did not carry them.
///
/// Heric goes beyond the original in one place: the document's own `parentId`
/// is rewritten when it is a key in `mapping`. The original re-parents at the
/// duplicate call site instead.
///
/// The function is infallible. Ids come from [`Id::generate`](crate::Id::generate)
/// or from files this application did not write; a pair whose old or new id
/// contains a quote or a backslash could break the JSON round trip, so such a
/// pair is skipped and the field is left as it was.
pub fn rewrite_references(doc: &mut Doc, mapping: &BTreeMap<Id, Id>) {
    let fields: &[&str] = match doc.meta.kind {
        DocKind::Request => &REQUEST_FIELDS,
        DocKind::RequestGroup => &REQUEST_GROUP_FIELDS,
        _ => &[],
    };
    for field in fields {
        rewrite_field(doc, field, mapping);
    }

    match doc.meta.kind {
        DocKind::Request => {
            doc.body.insert("konnectRouteKey".to_owned(), Value::Null);
        }
        DocKind::RequestGroup => {
            doc.body.insert("konnectRouteId".to_owned(), Value::Null);
        }
        _ => {}
    }

    if let Some(replacement) = doc
        .meta
        .parent
        .as_ref()
        .and_then(|parent| mapping.get(parent))
        .cloned()
    {
        doc.meta.parent = Some(replacement);
    }
}

/// Substitutes every mapped id in one body field, leaving it alone when the
/// field is absent or nothing changed.
fn rewrite_field(doc: &mut Doc, field: &str, mapping: &BTreeMap<Id, Id>) {
    let Some(value) = doc.body.get(field) else {
        return;
    };
    let Ok(original) = serde_json::to_string(value) else {
        return;
    };
    let replaced = mapping.iter().fold(original.clone(), |acc, (old, new)| {
        if is_round_trip_safe(old) && is_round_trip_safe(new) {
            acc.replace(old.as_str(), new.as_str())
        } else {
            acc
        }
    });
    if replaced == original {
        return;
    }
    if let Ok(patched) = serde_json::from_str::<Value>(&replaced) {
        doc.body.insert(field.to_owned(), patched);
    }
}

/// Returns whether substituting this id keeps the surrounding JSON valid.
///
/// A quote or a backslash in an id would terminate or escape a JSON string, so
/// the parse-back would fail. Generated ids never contain either; this guards
/// data that was migrated from elsewhere.
fn is_round_trip_safe(id: &Id) -> bool {
    let raw = id.as_str();
    !raw.contains('"') && !raw.contains('\\')
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use serde_json::{Map, Value, json};

    use super::*;
    use crate::DocMeta;

    /// Builds a document with an empty meta and the given JSON body.
    fn doc_of(kind: DocKind, body: Value) -> Doc {
        let body = match body {
            Value::Object(map) => map,
            other => {
                let mut map = Map::new();
                map.insert("value".to_owned(), other);
                map
            }
        };
        Doc {
            meta: DocMeta {
                id: Id::generate(kind),
                kind,
                parent: None,
                modified: 0,
                created: 0,
                private: None,
                name: None,
            },
            body,
        }
    }

    fn rename(old: &str, new: &str) -> BTreeMap<Id, Id> {
        let mut mapping = BTreeMap::new();
        mapping.insert(Id::new(old), Id::new(new));
        mapping
    }

    #[test]
    fn the_capability_table_covers_every_kind() {
        assert_eq!(CAPABILITIES.len(), 44);
        assert_eq!(CAPABILITIES.len(), DocKind::ALL.len());

        let mut seen = BTreeSet::new();
        for &(kind, _, _) in &CAPABILITIES {
            assert!(seen.insert(kind), "{kind} appears more than once");
        }
        for kind in DocKind::ALL {
            assert!(seen.contains(kind), "{kind} is missing from the table");
        }

        // The second reader repeats the sync column; keep the two in step.
        let expected: Vec<DocKind> = CAPABILITIES
            .iter()
            .filter(|&&(_, _, sync)| sync)
            .map(|&(kind, _, _)| kind)
            .collect();
        assert_eq!(expected, kinds_that_sync());
    }

    #[test]
    fn project_lint_rulesets_sync_without_duplicating() {
        assert!(!can_duplicate(DocKind::ProjectLintRuleset));
        assert!(kinds_that_sync().contains(&DocKind::ProjectLintRuleset));
    }

    #[test]
    fn a_duplicated_request_gets_its_parent_rewritten() {
        let mut doc = doc_of(DocKind::Request, json!({ "url": "" }));
        doc.meta.parent = Some(Id::new("fld_old"));

        rewrite_references(&mut doc, &rename("fld_old", "fld_new"));

        assert_eq!(doc.meta.parent, Some(Id::new("fld_new")));
    }

    #[test]
    fn an_id_inside_a_url_is_substituted() {
        let mut doc = doc_of(
            DocKind::Request,
            json!({ "url": "https://x/{{ req_old }}/y" }),
        );

        rewrite_references(&mut doc, &rename("req_old", "req_new"));

        let url = doc.get("url").and_then(Value::as_str).unwrap_or_default();
        assert!(url.contains("req_new"), "url was {url}");
        assert!(!url.contains("req_old"), "url was {url}");
    }

    #[test]
    fn a_response_reference_inside_a_body_is_substituted() {
        let mut doc = doc_of(
            DocKind::Request,
            json!({ "body": { "text": "{% response 'body', 'req_old' %}" } }),
        );

        rewrite_references(&mut doc, &rename("req_old", "req_new"));

        let body = doc.get("body").cloned().unwrap_or(Value::Null);
        let text = body.to_string();
        assert!(text.contains("req_new"), "body was {text}");
        assert!(!text.contains("req_old"), "body was {text}");
    }

    #[test]
    fn rewriting_nulls_the_konnect_route_key_for_a_request() {
        let mut doc = doc_of(
            DocKind::Request,
            json!({ "url": "", "konnectRouteKey": "route-1" }),
        );

        rewrite_references(&mut doc, &BTreeMap::new());

        assert_eq!(doc.get("konnectRouteKey"), Some(&Value::Null));
    }

    #[test]
    fn rewriting_nulls_the_konnect_route_id_for_a_group() {
        let mut doc = doc_of(
            DocKind::RequestGroup,
            json!({ "environment": {}, "konnectRouteId": "route-1" }),
        );

        rewrite_references(&mut doc, &BTreeMap::new());

        assert_eq!(doc.get("konnectRouteId"), Some(&Value::Null));
    }

    #[test]
    fn an_unmodelled_kind_is_untouched() {
        let mut doc = doc_of(
            DocKind::Response,
            json!({ "bodyPath": "res_old/body", "url": "https://x/res_old" }),
        );
        let before = doc.body.clone();

        rewrite_references(&mut doc, &rename("res_old", "res_new"));

        assert_eq!(doc.body, before);
    }

    #[test]
    fn an_empty_mapping_changes_nothing() {
        for kind in [DocKind::Request, DocKind::RequestGroup] {
            let mut doc = doc_of(kind, json!({ "url": "https://x/req_old" }));
            let mut before = doc.body.clone();

            rewrite_references(&mut doc, &BTreeMap::new());

            // Only the konnect key differs, and only for these two kinds.
            let konnect = match kind {
                DocKind::Request => "konnectRouteKey",
                _ => "konnectRouteId",
            };
            assert_eq!(doc.get(konnect), Some(&Value::Null));
            assert!(!before.contains_key(konnect));
            doc.body.remove(konnect);
            before.remove(konnect);
            assert_eq!(doc.body, before);
        }
    }
}
