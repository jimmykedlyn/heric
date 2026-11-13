//! The original's `initModel`: fresh-document defaults and key pruning.
//!
//! The original builds a new document by merging global defaults, the model's
//! `init()` and the caller's patches, then deletes every key that is neither a
//! default nor listed in the model's `optionalKeys`
//! (`packages/insomnia-data/node-src/database/init-model/index.ts`). This
//! module reproduces that table-driven behaviour; the kind-specific migrations
//! the original runs before pruning belong to the migration layer, not here.
//!
//! Two things differ from the original's merge by necessity. `name` is a meta
//! key in Heric ([`DocMeta::name`]), so a model's default name lives in
//! [`ModelSpec::default_name`] rather than in the body. `metaSortKey` depends
//! on the creation time, so [`ModelSpec::sort_key`] records how to compute it
//! instead of storing a value a constant cannot express.

use serde_json::{Map, Value};

use crate::error::ModelError;
use crate::{Doc, DocKind, DocMeta, Id};

/// Id of the scratchpad workspace, which exists in every installation.
pub const SCRATCHPAD_WORKSPACE: &str = "wrk_scratchpad";

/// Id of the scratchpad project, the parent of [`SCRATCHPAD_WORKSPACE`].
pub const SCRATCHPAD_PROJECT: &str = "proj_scratchpad";

/// Id of the organization the scratchpad project belongs to.
///
/// The original has no organization document: the id is a constant owned by
/// the cloud API package. Heric only ever uses it as the scratchpad project's
/// `parentId`.
pub const SCRATCHPAD_ORGANIZATION: &str = "org_scratchpad";

/// The `metaSortKey` body key, computed by [`document`] because it depends on
/// the creation time.
const META_SORT_KEY: &str = "metaSortKey";

/// A JSON value in a [`ModelSpec`] table.
///
/// The tables are constants, which cannot hold a [`Value`] because `Value`
/// owns strings and collections. Only the shapes the original's `init()`
/// functions actually use are represented.
#[derive(Debug, Clone, Copy)]
enum DefaultValue {
    /// JSON `null`.
    Null,
    /// A boolean.
    Bool(bool),
    /// An integer.
    Int(i64),
    /// A string literal.
    Str(&'static str),
    /// JSON `[]`.
    EmptyArray,
    /// JSON `{}`.
    EmptyObject,
}

impl DefaultValue {
    /// Returns the JSON value this entry stands for.
    fn to_value(self) -> Value {
        match self {
            Self::Null => Value::Null,
            Self::Bool(value) => Value::Bool(value),
            Self::Int(value) => Value::from(value),
            Self::Str(value) => Value::from(value),
            Self::EmptyArray => Value::Array(Vec::new()),
            Self::EmptyObject => Value::Object(Map::new()),
        }
    }
}

/// How a model's `metaSortKey` is computed from the creation time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortKey {
    /// The model has no `metaSortKey`.
    None,
    /// `-now`, so a newer document sorts first. The original uses this for
    /// requests and folders.
    NegatedNow,
    /// `now`, which the original uses for environments only.
    PositiveNow,
}

impl SortKey {
    /// Returns the `metaSortKey` for a document created at `now`.
    fn value(self, now: i64) -> Option<Value> {
        // Emitted as a float, as the original's `Date.now()` arithmetic
        // produces: `serde_json` compares numbers by value, so `-7` and `-7.0`
        // are different values even though they are equal as JSON.
        match self {
            Self::None => None,
            Self::NegatedNow => Some(Value::from(-(now as f64))),
            Self::PositiveNow => Some(Value::from(now as f64)),
        }
    }
}

/// The `init()` table for one model kind.
struct ModelSpec {
    /// Body keys the model's `init()` gives a value, excluding `metaSortKey`,
    /// which [`SortKey`] computes.
    defaults: &'static [(&'static str, DefaultValue)],
    /// Keys the model lists in `optionalKeys`: absent from `init()` but kept
    /// when a document carries them.
    optional: &'static [&'static str],
    /// The model's default display name.
    default_name: Option<&'static str>,
    /// How `metaSortKey` is derived from the creation time.
    sort_key: SortKey,
}

/// Returns whether the model's `init()` sets `isPrivate: false`.
///
/// `isPrivate` is a meta key in Heric, but whether a fresh document carries it
/// is still model-dependent: exactly five of the original's models set it
/// (`request.ts:442`, `grpc-request.ts:80`, `ca-certificate.ts:29`,
/// `environment.ts:70`, `client-certificate.ts:38`), and the pruning loop at
/// `init-model/index.ts:73-78` *deletes* it for every other type. So a fresh
/// `Workspace` has no `isPrivate` at all, and [`DocMeta::private`] must stay
/// `None` for it rather than becoming `Some(false)`: the two mean different
/// things (see [`DocMeta::private`]), and sync reads the difference.
///
/// Three of the five are unmodelled here, so this is a property of the kind and
/// not of a [`ModelSpec`].
const fn sets_private(kind: DocKind) -> bool {
    matches!(
        kind,
        DocKind::Request
            | DocKind::GrpcRequest
            | DocKind::Environment
            | DocKind::CaCertificate
            | DocKind::ClientCertificate
    )
}

impl ModelSpec {
    /// Returns whether `field` may appear in a document's body.
    fn declares(&self, field: &str) -> bool {
        let sort_key = field == META_SORT_KEY && self.sort_key != SortKey::None;
        sort_key
            || self.optional.contains(&field)
            || self.defaults.iter().any(|(key, _)| *key == field)
    }
}

/// The spec for a modelled kind, or `None` for the 33 kinds this layer leaves
/// alone.
///
/// Values are transcribed from each model's `export function init()` under
/// `packages/insomnia-data/src/models/`, with `name` moved to
/// [`ModelSpec::default_name`] and `parentId: null` omitted because
/// [`DocMeta::parent`] already means absent.
fn spec(kind: DocKind) -> Option<&'static ModelSpec> {
    match kind {
        DocKind::Workspace => Some(&WORKSPACE),
        DocKind::WorkspaceMeta => Some(&WORKSPACE_META),
        DocKind::Project => Some(&PROJECT),
        DocKind::Request => Some(&REQUEST),
        DocKind::RequestMeta => Some(&REQUEST_META),
        DocKind::RequestGroup => Some(&REQUEST_GROUP),
        DocKind::RequestGroupMeta => Some(&REQUEST_GROUP_META),
        DocKind::Environment => Some(&ENVIRONMENT),
        DocKind::Response => Some(&RESPONSE),
        DocKind::CookieJar => Some(&COOKIE_JAR),
        // Settings has 66 init keys owned by Task 8. It stays unmodelled until
        // that task lands, so pruning cannot delete what it writes; `apply`
        // therefore leaves a Settings document exactly as it found it.
        _ => None,
    }
}

/// `models/workspace.ts`
static WORKSPACE: ModelSpec = ModelSpec {
    defaults: &[
        ("description", DefaultValue::Str("")),
        ("scope", DefaultValue::Str("collection")),
    ],
    optional: &["konnectServiceId"],
    default_name: Some("New API Collection"),
    sort_key: SortKey::None,
};

/// `models/workspace-meta.ts`
static WORKSPACE_META: ModelSpec = ModelSpec {
    defaults: &[
        ("activeActivity", DefaultValue::Null),
        ("activeEnvironmentId", DefaultValue::Null),
        ("activeGlobalEnvironmentId", DefaultValue::Null),
        ("activeRequestId", DefaultValue::Null),
        ("activeUnitTestSuiteId", DefaultValue::Null),
        ("gitRepositoryId", DefaultValue::Null),
        ("gitFilePath", DefaultValue::Null),
        ("gitFileLastSyncTime", DefaultValue::Null),
        ("pushSnapshotOnInitialize", DefaultValue::Bool(false)),
        ("hasUncommittedChanges", DefaultValue::Bool(false)),
        ("hasUnpushedChanges", DefaultValue::Bool(false)),
    ],
    optional: &[],
    default_name: None,
    sort_key: SortKey::None,
};

/// `models/project.ts`
static PROJECT: ModelSpec = ModelSpec {
    defaults: &[
        ("remoteId", DefaultValue::Null),
        ("gitRepositoryId", DefaultValue::Null),
        ("mcpStdioAccess", DefaultValue::Bool(false)),
    ],
    optional: &[
        "konnectControlPlaneId",
        "konnectClusterType",
        "konnectDeploymentType",
        "konnectRegion",
    ],
    default_name: Some("My Project"),
    sort_key: SortKey::None,
};

/// `models/request.ts`
static REQUEST: ModelSpec = ModelSpec {
    defaults: &[
        ("url", DefaultValue::Str("")),
        ("description", DefaultValue::Str("")),
        ("method", DefaultValue::Str("GET")),
        ("body", DefaultValue::EmptyObject),
        ("parameters", DefaultValue::EmptyArray),
        ("headers", DefaultValue::EmptyArray),
        ("authentication", DefaultValue::EmptyObject),
        ("settingStoreCookies", DefaultValue::Bool(true)),
        ("settingSendCookies", DefaultValue::Bool(true)),
        ("settingDisableRenderRequestBody", DefaultValue::Bool(false)),
        ("settingEncodeUrl", DefaultValue::Bool(true)),
        ("settingRebuildPath", DefaultValue::Bool(true)),
        ("settingFollowRedirects", DefaultValue::Str("global")),
    ],
    // `preRequestScript`, `afterResponseScript` and `pathParameters` are
    // declared by the schema but are `undefined` in `init()`, so they are
    // optional rather than defaulted.
    optional: &[
        "preRequestScript",
        "afterResponseScript",
        "pathParameters",
        "konnectRouteKey",
        "konnectManagedHeaderNames",
        "disableUserAgentHeader",
    ],
    default_name: Some("New Request"),
    sort_key: SortKey::NegatedNow,
};

/// `models/request-meta.ts`
static REQUEST_META: ModelSpec = ModelSpec {
    defaults: &[
        ("previewMode", DefaultValue::Str("friendly")),
        ("responseFilter", DefaultValue::Str("")),
        ("responseFilterHistory", DefaultValue::EmptyArray),
        ("activeResponseId", DefaultValue::Null),
        ("savedRequestBody", DefaultValue::EmptyObject),
        ("pinned", DefaultValue::Bool(false)),
        ("lastActive", DefaultValue::Int(0)),
        ("downloadPath", DefaultValue::Null),
        ("expandedAccordionKeys", DefaultValue::EmptyObject),
        ("activeMcpPrimitive", DefaultValue::Null),
        ("streamSummaryPath", DefaultValue::Null),
        ("streamSummaryRenderMarkdown", DefaultValue::Bool(false)),
    ],
    optional: &[],
    default_name: None,
    sort_key: SortKey::None,
};

/// `models/request-group.ts`
static REQUEST_GROUP: ModelSpec = ModelSpec {
    defaults: &[
        ("description", DefaultValue::Str("")),
        ("environment", DefaultValue::EmptyObject),
    ],
    // `preRequestScript`, `afterResponseScript`, `authentication` and `headers`
    // are `undefined` in `init()`; the rest are listed in `optionalKeys`.
    optional: &[
        "preRequestScript",
        "afterResponseScript",
        "authentication",
        "headers",
        "kvPairData",
        "environmentType",
        "konnectRouteId",
        "environmentPropertyOrder",
    ],
    default_name: Some("New Folder"),
    sort_key: SortKey::NegatedNow,
};

/// `models/request-group-meta.ts`
static REQUEST_GROUP_META: ModelSpec = ModelSpec {
    defaults: &[("collapsed", DefaultValue::Bool(false))],
    optional: &[],
    default_name: None,
    sort_key: SortKey::None,
};

/// `models/environment.ts`
static ENVIRONMENT: ModelSpec = ModelSpec {
    defaults: &[
        ("data", DefaultValue::EmptyObject),
        ("color", DefaultValue::Null),
    ],
    optional: &["kvPairData", "environmentType", "dataPropertyOrder"],
    default_name: Some("New Environment"),
    sort_key: SortKey::PositiveNow,
};

/// `models/response.ts`
static RESPONSE: ModelSpec = ModelSpec {
    defaults: &[
        ("statusCode", DefaultValue::Int(0)),
        ("statusMessage", DefaultValue::Str("")),
        ("httpVersion", DefaultValue::Str("")),
        ("contentType", DefaultValue::Str("")),
        ("url", DefaultValue::Str("")),
        ("bytesRead", DefaultValue::Int(0)),
        ("bytesContent", DefaultValue::Int(-1)),
        ("elapsedTime", DefaultValue::Int(0)),
        ("headers", DefaultValue::EmptyArray),
        ("timelinePath", DefaultValue::Str("")),
        ("bodyPath", DefaultValue::Str("")),
        ("bodyCompression", DefaultValue::Str("__NEEDS_MIGRATION__")),
        ("error", DefaultValue::Str("")),
        ("requestVersionId", DefaultValue::Null),
        ("settingStoreCookies", DefaultValue::Null),
        ("settingSendCookies", DefaultValue::Null),
        ("environmentId", DefaultValue::Str("__LEGACY__")),
        ("requestTestResults", DefaultValue::EmptyArray),
        ("globalEnvironmentId", DefaultValue::Null),
    ],
    optional: &["bodyBuffer"],
    default_name: None,
    sort_key: SortKey::None,
};

/// `models/cookie-jar.ts`
static COOKIE_JAR: ModelSpec = ModelSpec {
    defaults: &[("cookies", DefaultValue::EmptyArray)],
    optional: &[],
    default_name: Some("Default Jar"),
    sort_key: SortKey::None,
};

/// Builds a fresh document of `kind`, stamped with `now`.
///
/// The id is generated from the kind's prefix, `modified` and `created` are
/// `now` and there is no parent, matching the original's `initModel` global
/// defaults. `isPrivate` is set only for the five kinds whose `init()` declares
/// it, and absent for the rest — see [`sets_private`]. The body comes from the
/// kind's [`ModelSpec`]; an unmodelled kind gets an empty body.
#[must_use]
pub fn document(kind: DocKind, now: i64) -> Doc {
    let mut body = Map::new();
    let mut name = None;
    if let Some(spec) = spec(kind) {
        for (key, value) in spec.defaults {
            body.insert((*key).to_owned(), value.to_value());
        }
        if let Some(moment) = spec.sort_key.value(now) {
            body.insert(META_SORT_KEY.to_owned(), moment);
        }
        name = spec.default_name.map(str::to_owned);
    }

    Doc {
        meta: DocMeta {
            id: Id::generate(kind),
            kind,
            parent: None,
            modified: now,
            created: now,
            private: sets_private(kind).then_some(false),
            name,
        },
        body,
    }
}

/// Fills in absent defaults and prunes keys the document's model does not
/// declare.
///
/// A present key is never overwritten, so this is safe to run on a document
/// that came from another version. Unmodelled kinds are left untouched: there
/// is no table to prune against, and their keys are the only record of what
/// they carry.
///
/// # Errors
///
/// Pruning cannot fail today. The result is fallible because the original runs
/// a kind-specific migration between merging and pruning, and that step will
/// live here when the migration layer needs it.
pub fn apply(doc: &mut Doc) -> Result<(), ModelError> {
    let Some(spec) = spec(doc.meta.kind) else {
        return Ok(());
    };

    for (key, value) in spec.defaults {
        doc.body
            .entry((*key).to_owned())
            .or_insert_with(|| value.to_value());
    }
    if let Some(moment) = spec.sort_key.value(doc.meta.modified) {
        doc.body.entry(META_SORT_KEY.to_owned()).or_insert(moment);
    }

    // `name` is a meta key, so the model's default name never sits in the
    // body. A body key that shadows a meta key is already invisible to
    // `Doc::get`; removing it keeps the stored document honest.
    let undeclared: Vec<String> = doc
        .body
        .keys()
        .filter(|key| !spec.declares(key))
        .cloned()
        .collect();
    for key in undeclared {
        doc.body.remove(&key);
    }

    Ok(())
}

/// Returns the two documents that make up the scratchpad, project first.
///
/// The ids are fixed so that a scratchpad document is recognisable across
/// runs and installations. The project hangs off [`SCRATCHPAD_ORGANIZATION`],
/// an id with no document behind it, exactly as the original's
/// `entry.main.ts` sets it up.
#[must_use]
pub fn scratchpad() -> Vec<Doc> {
    let now = 0;
    let mut project = document(DocKind::Project, now);
    project.meta.id = Id::new(SCRATCHPAD_PROJECT);
    project.meta.parent = Some(Id::new(SCRATCHPAD_ORGANIZATION));
    // The original names the scratchpad project after `appConfig.productName`,
    // which is "Insomnia". Heric does not carry that name, and no test pins the
    // project's, so both sentinels share the workspace's name.
    project.meta.name = Some("Scratch Pad".to_owned());

    let mut workspace = document(DocKind::Workspace, now);
    workspace.meta.id = Id::new(SCRATCHPAD_WORKSPACE);
    workspace.meta.parent = Some(Id::new(SCRATCHPAD_PROJECT));
    workspace.meta.name = Some("Scratch Pad".to_owned());

    vec![project, workspace]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init;

    fn document_of_kind(kind: DocKind) -> Doc {
        document(kind, 5)
    }

    #[test]
    fn a_new_request_gets_get_and_an_empty_body() -> Result<(), Box<dyn std::error::Error>> {
        let doc = init::document(DocKind::Request, 1_000);
        assert_eq!(doc.body.get("method"), Some(&serde_json::json!("GET")));
        assert_eq!(doc.body.get("headers"), Some(&serde_json::json!([])));
        assert_eq!(doc.body.get("authentication"), Some(&serde_json::json!({})));
        assert_eq!(doc.meta.private, Some(false));
        assert_eq!(
            doc.body.get("metaSortKey"),
            Some(&serde_json::json!(-1_000.0))
        );
        assert!(doc.meta.id.as_str().starts_with("req_"));
        assert_eq!(doc.meta.name.as_deref(), Some("New Request"));
        Ok(())
    }

    #[test]
    fn pruning_drops_an_undeclared_key_for_a_modelled_kind()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut doc = init::document(DocKind::Request, 0);
        doc.body
            .insert("notAModelField".into(), serde_json::json!(1));
        init::apply(&mut doc)?;
        assert!(!doc.body.contains_key("notAModelField"));
        Ok(())
    }

    #[test]
    fn an_unmodelled_kind_keeps_every_key_it_was_given() -> Result<(), Box<dyn std::error::Error>> {
        let mut doc = init::document(DocKind::MockRoute, 0);
        doc.body.insert("whatever".into(), serde_json::json!(1));
        init::apply(&mut doc)?;
        assert_eq!(doc.body.get("whatever"), Some(&serde_json::json!(1)));
        Ok(())
    }

    #[test]
    fn the_scratchpad_is_a_project_then_a_workspace() -> Result<(), Box<dyn std::error::Error>> {
        let docs = init::scratchpad();
        let ids: Vec<&str> = docs.iter().map(|doc| doc.meta.id.as_str()).collect();
        assert_eq!(ids, [SCRATCHPAD_PROJECT, SCRATCHPAD_WORKSPACE]);
        let project = docs.first().ok_or("no project")?;
        assert_eq!(project.meta.kind, DocKind::Project);
        assert_eq!(
            project.meta.parent.as_ref().map(Id::as_str),
            Some(SCRATCHPAD_ORGANIZATION)
        );
        let workspace = docs.get(1).ok_or("no workspace")?;
        assert_eq!(workspace.meta.kind, DocKind::Workspace);
        assert_eq!(
            workspace.meta.parent.as_ref().map(Id::as_str),
            Some(SCRATCHPAD_PROJECT)
        );
        Ok(())
    }

    #[test]
    fn a_fresh_workspace_gets_a_generated_id_not_the_scratchpad_one() {
        assert_ne!(
            init::document(DocKind::Workspace, 0).meta.id.as_str(),
            SCRATCHPAD_WORKSPACE
        );
    }

    #[test]
    fn an_absent_default_is_filled_and_a_present_one_is_kept()
    -> Result<(), Box<dyn std::error::Error>> {
        let mut absent = init::document(DocKind::Request, 0);
        absent.body.remove("method");
        init::apply(&mut absent)?;
        assert_eq!(absent.body.get("method"), Some(&serde_json::json!("GET")));

        let mut present = init::document(DocKind::Request, 0);
        present
            .body
            .insert("method".into(), serde_json::json!("POST"));
        init::apply(&mut present)?;
        assert_eq!(present.body.get("method"), Some(&serde_json::json!("POST")));
        Ok(())
    }

    #[test]
    fn a_declared_optional_key_survives_pruning() -> Result<(), Box<dyn std::error::Error>> {
        let mut doc = init::document(DocKind::Request, 0);
        doc.body.insert(
            "pathParameters".into(),
            serde_json::json!([{"name": "id", "value": "1"}]),
        );
        doc.body.insert(
            "preRequestScript".into(),
            serde_json::json!("console.log(1)"),
        );
        init::apply(&mut doc)?;
        assert_eq!(
            doc.body.get("pathParameters"),
            Some(&serde_json::json!([{"name": "id", "value": "1"}]))
        );
        assert_eq!(
            doc.body.get("preRequestScript"),
            Some(&serde_json::json!("console.log(1)"))
        );
        Ok(())
    }

    #[test]
    fn every_kind_gets_a_document_that_round_trips() -> Result<(), Box<dyn std::error::Error>> {
        for kind in DocKind::ALL {
            let doc = document_of_kind(*kind);
            let serialised = serde_json::to_value(&doc)?;
            let back: Doc = serde_json::from_value(serialised)?;
            assert_eq!(back, doc, "{kind} did not survive a round trip");
        }
        Ok(())
    }

    #[test]
    fn the_sort_key_sign_follows_the_kind() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            init::document(DocKind::Request, 7).body.get("metaSortKey"),
            Some(&serde_json::json!(-7.0))
        );
        assert_eq!(
            init::document(DocKind::RequestGroup, 7)
                .body
                .get("metaSortKey"),
            Some(&serde_json::json!(-7.0))
        );
        assert_eq!(
            init::document(DocKind::Environment, 7)
                .body
                .get("metaSortKey"),
            Some(&serde_json::json!(7.0))
        );
        Ok(())
    }

    #[test]
    fn an_unmodelled_kind_gets_no_sort_key() {
        assert!(
            !document_of_kind(DocKind::MockRoute)
                .body
                .contains_key("metaSortKey")
        );
    }

    #[test]
    fn only_the_five_private_models_carry_is_private() {
        // `isPrivate` is in five `init()` functions and pruned out of every
        // other type, so a fresh document of any other kind has no such field.
        for kind in DocKind::ALL {
            let doc = document_of_kind(*kind);
            let expected = matches!(
                kind,
                DocKind::Request
                    | DocKind::GrpcRequest
                    | DocKind::Environment
                    | DocKind::CaCertificate
                    | DocKind::ClientCertificate
            );
            assert_eq!(
                doc.meta.private,
                expected.then_some(false),
                "{kind} got the wrong isPrivate"
            );
        }
    }

    #[test]
    fn an_absent_is_private_stays_absent_through_a_round_trip()
    -> Result<(), Box<dyn std::error::Error>> {
        let doc = init::document(DocKind::Workspace, 1);
        let json = serde_json::to_value(&doc)?;
        assert!(json.get("isPrivate").is_none());
        let back: Doc = serde_json::from_value(json)?;
        assert_eq!(back.meta.private, None);
        Ok(())
    }

    #[test]
    fn applying_twice_changes_nothing_the_second_time() -> Result<(), Box<dyn std::error::Error>> {
        let mut doc = init::document(DocKind::Environment, 3);
        init::apply(&mut doc)?;
        let once = doc.clone();
        init::apply(&mut doc)?;
        assert_eq!(doc, once);
        Ok(())
    }
}
