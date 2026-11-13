//! Workspace, project and scope reads.
//!
//! The discriminator here is deliberately *not* called `Scope`: `heric-template`
//! exports a `Scope` that means a variable layer, and `heric-core` depends on
//! both crates. Two types with one name in one crate's view is a reading
//! mistake waiting to happen, so this crate's workspace scope is
//! [`WorkspaceScope`] and its discriminator is [`CollectionScope`].

use serde_json::Value;

use crate::{Doc, DocKind, Id, ModelError};

/// What kind of thing a workspace holds.
///
/// The five values the original's schema allows, plus the absent case, which
/// means a collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CollectionScope {
    /// A document collection: requests and folders.
    #[default]
    Collection,
    /// A design document.
    Design,
    /// A mock server.
    MockServer,
    /// An environment workspace.
    Environment,
    /// An MCP client workspace.
    Mcp,
}

impl CollectionScope {
    /// The wire name the original stores.
    #[must_use]
    pub fn wire_name(self) -> &'static str {
        match self {
            Self::Collection => "collection",
            Self::Design => "design",
            Self::MockServer => "mock-server",
            Self::Environment => "environment",
            Self::Mcp => "mcp",
        }
    }

    /// Parses a stored scope name.
    ///
    /// An unrecognised value is not an error: the original's schema defaults
    /// the field, and a workspace written by a future release must still open.
    /// The caller that needs to warn about it can compare against
    /// [`Self::Collection`] and the raw string.
    #[must_use]
    pub fn parse(raw: &str) -> Self {
        match raw {
            "design" => Self::Design,
            "mock-server" => Self::MockServer,
            "environment" => Self::Environment,
            "mcp" => Self::Mcp,
            _ => Self::Collection,
        }
    }
}

/// A workspace's scope: which kind of thing it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WorkspaceScope {
    /// The discriminator.
    pub scope: CollectionScope,
    /// The raw value as stored, when it was present.
    ///
    /// Kept so a caller can tell "absent" from "unrecognised" without
    /// re-reading the document.
    pub raw: Option<&'static str>,
}

/// Reads a workspace's scope.
///
/// # Errors
///
/// Returns [`ModelError::WrongKind`] when the document is not a workspace.
pub fn scope_of(doc: &Doc) -> Result<CollectionScope, ModelError> {
    let workspace = doc.as_workspace()?;
    let raw = workspace
        .doc()
        .body
        .get("scope")
        .and_then(Value::as_str)
        .unwrap_or_default();
    Ok(CollectionScope::parse(raw))
}

/// Checks that a workspace's parent is a project.
///
/// The scratchpad workspace is the one workspace with no owning project, so an
/// absent parent is valid. Any other absent or unresolvable parent is not
/// treated as an error here either: `lookup` returning `None` means the caller
/// has not loaded the parent, which is a question this function cannot answer.
///
/// # Errors
///
/// Returns [`ModelError::WrongKind`] when the parent resolves to something that
/// is not a project.
pub fn assert_parent_is_project(
    doc: &Doc,
    lookup: &dyn Fn(&Id) -> Option<Doc>,
) -> Result<(), ModelError> {
    let Some(parent_id) = doc.meta.parent.as_ref() else {
        return Ok(());
    };
    let Some(parent) = lookup(parent_id) else {
        return Ok(());
    };
    if parent.meta.kind == DocKind::Project {
        return Ok(());
    }
    Err(ModelError::WrongKind {
        expected: DocKind::Project,
        found: parent.meta.kind,
    })
}

/// The project list, in the order the sidebar shows it.
///
/// The original sorts in two passes, not one comparator: projects that belong
/// to the default organization come first, and each group is then sorted by
/// name. A single comparator mixing both keys gives a different order whenever
/// a non-default project's name would sort between two default ones, so the
/// two passes are reproduced literally.
pub struct Projects;

impl Projects {
    /// Sorts `projects` into display order.
    #[must_use]
    pub fn sorted(projects: &[Doc]) -> Vec<Doc> {
        let mut default_org: Vec<Doc> = Vec::new();
        let mut other: Vec<Doc> = Vec::new();
        for project in projects {
            if is_default_organization(project) {
                default_org.push(project.clone());
            } else {
                other.push(project.clone());
            }
        }
        default_org.sort_by_key(project_name);
        other.sort_by_key(project_name);
        default_org.extend(other);
        default_org
    }
}

/// The project's name, or an empty string when it has none.
fn project_name(doc: &Doc) -> String {
    doc.meta.name.clone().unwrap_or_default()
}

/// Whether a project belongs to the default organization.
///
/// The original derives this from the remote id's prefix: `proj_team` and
/// `proj_org` both mean the default organization.
fn is_default_organization(doc: &Doc) -> bool {
    doc.body
        .get("remoteId")
        .and_then(Value::as_str)
        .is_some_and(|id| id.starts_with("proj_team") || id.starts_with("proj_org"))
}

/// Where a project lives, as a label a person reads.
///
/// Checked Git first, deliberately: a git project has `remoteId: null` and
/// would otherwise be labelled Local. A document that is not a project gets
/// `"Local"`, because the return type has no third answer and the original's
/// type system makes the case impossible.
#[must_use]
pub fn storage_label(doc: &Doc) -> &'static str {
    let is_git = doc
        .body
        .get("gitRepositoryId")
        .and_then(Value::as_str)
        .is_some_and(|id| !id.is_empty() || id == "empty");
    if is_git {
        return "Git";
    }
    // `remoteId` present and non-null means the project is cloud-backed.
    let remote = doc.body.get("remoteId");
    match remote {
        Some(Value::String(id)) if !id.is_empty() => "Remote",
        _ => "Local",
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

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
    fn the_scope_discriminator_reads_collection_design_and_mock_server() -> TestResult {
        for (raw, expected) in [
            ("collection", CollectionScope::Collection),
            ("design", CollectionScope::Design),
            ("mock-server", CollectionScope::MockServer),
            ("environment", CollectionScope::Environment),
            ("mcp", CollectionScope::Mcp),
        ] {
            let workspace = doc(DocKind::Workspace, "wrk_1", json!({"scope": raw}))?;
            assert_eq!(scope_of(&workspace)?, expected, "for `{raw}`");
        }
        Ok(())
    }

    #[test]
    fn an_absent_or_unknown_scope_is_a_collection() -> TestResult {
        let absent = doc(DocKind::Workspace, "wrk_1", json!({}))?;
        assert_eq!(scope_of(&absent)?, CollectionScope::Collection);

        let unknown = doc(DocKind::Workspace, "wrk_2", json!({"scope": "future"}))?;
        assert_eq!(scope_of(&unknown)?, CollectionScope::Collection);
        Ok(())
    }

    #[test]
    fn a_non_workspace_has_no_scope() -> TestResult {
        let request = doc(DocKind::Request, "req_1", json!({}))?;
        assert!(matches!(
            scope_of(&request),
            Err(ModelError::WrongKind {
                expected: DocKind::Workspace,
                ..
            })
        ));
        Ok(())
    }

    #[test]
    fn a_workspace_parent_must_be_a_project() -> TestResult {
        let workspace = doc(DocKind::Workspace, "wrk_1", json!({"parentId": "proj_1"}))?;

        let project = doc(DocKind::Project, "proj_1", json!({}))?;
        assert!(assert_parent_is_project(&workspace, &|_| Some(project.clone())).is_ok());

        let wrong = doc(DocKind::Request, "proj_1", json!({}))?;
        assert!(matches!(
            assert_parent_is_project(&workspace, &|_| Some(wrong.clone())),
            Err(ModelError::WrongKind {
                expected: DocKind::Project,
                found: DocKind::Request
            })
        ));

        // An absent parent is the scratchpad, which is valid.
        assert!(assert_parent_is_project(&workspace, &|_| None).is_ok());

        // A workspace with no parentId at all is valid too.
        let orphan = doc(DocKind::Workspace, "wrk_scratchpad", json!({}))?;
        assert!(assert_parent_is_project(&orphan, &|_| None).is_ok());
        Ok(())
    }

    #[test]
    fn projects_sort_by_organization_then_name() -> TestResult {
        let a = doc(
            DocKind::Project,
            "proj_a",
            json!({"name": "zzz", "remoteId": "proj_team_1"}),
        )?;
        let b = doc(
            DocKind::Project,
            "proj_b",
            json!({"name": "aaa", "remoteId": null}),
        )?;
        let c = doc(
            DocKind::Project,
            "proj_c",
            json!({"name": "bbb", "remoteId": "proj_org_1"}),
        )?;
        let sorted = Projects::sorted(&[a, b, c]);
        let names: Vec<&str> = sorted
            .iter()
            .filter_map(|d| d.meta.name.as_deref())
            .collect();
        assert_eq!(names, ["bbb", "zzz", "aaa"]);
        Ok(())
    }

    #[test]
    fn a_non_default_project_sorts_last_even_when_its_name_comes_first() -> TestResult {
        // This is the case a single comparator gets wrong.
        let default_org = doc(
            DocKind::Project,
            "proj_a",
            json!({"name": "zulu", "remoteId": "proj_org_1"}),
        )?;
        let other = doc(
            DocKind::Project,
            "proj_b",
            json!({"name": "alpha", "remoteId": "proj_team_9"}),
        )?;
        let local = doc(
            DocKind::Project,
            "proj_c",
            json!({"name": "mike", "remoteId": null}),
        )?;
        let names: Vec<String> = Projects::sorted(&[other, local, default_org])
            .iter()
            .map(project_name)
            .collect();
        // `proj_team_9` *is* a default-org prefix, so it sorts with the first
        // group; the local project sorts in the second.
        assert_eq!(names, ["alpha", "zulu", "mike"]);
        Ok(())
    }

    #[test]
    fn the_storage_label_is_human_readable() -> TestResult {
        let local = doc(DocKind::Project, "proj_1", json!({"remoteId": null}))?;
        assert_eq!(storage_label(&local), "Local");

        let remote = doc(DocKind::Project, "proj_2", json!({"remoteId": "proj_abc"}))?;
        assert_eq!(storage_label(&remote), "Remote");

        let git = doc(
            DocKind::Project,
            "proj_3",
            json!({"remoteId": null, "gitRepositoryId": "git_abc"}),
        )?;
        assert_eq!(storage_label(&git), "Git");

        // A git project with no remote must not be labelled Local.
        assert_ne!(storage_label(&git), "Local");
        Ok(())
    }

    #[test]
    fn a_non_project_is_labelled_local() -> TestResult {
        let request = doc(DocKind::Request, "req_1", json!({}))?;
        assert_eq!(storage_label(&request), "Local");
        Ok(())
    }
}
