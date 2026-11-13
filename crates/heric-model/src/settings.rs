//! The settings singleton: its defaults, its repair pass and its migration.
//!
//! The original keeps exactly one `Settings` row, looked up by a fixed id, and
//! runs a migration over it on every read (`init-model/settings.ts`). This
//! module owns the same three pieces: the `init()` defaults, the repair that
//! restores what a document is missing, and the migration that folds a retired
//! flag forward and prunes hotkeys the app no longer knows.
//!
//! One thing is deliberately not ported yet. The original's `hotKeyRegistry`
//! default is `newDefaultRegistry()`, a 35-entry table of key combinations
//! (`common-src/hotkeys.ts:205`). M1 has no keyboard handling, so
//! [`defaults`] writes an empty registry and [`migrate_hotkeys`] keeps whatever
//! bindings a user already has. An invented registry would be worse than an
//! empty one: it would claim bindings the app does not honour.

use serde_json::{Map, Value};

use crate::init::DefaultValue;
use crate::{Doc, DocKind, DocMeta, Id};

/// The singleton settings document's id.
///
/// Fixed rather than generated: the original keeps one settings row and looks it
/// up by a known id, and two settings rows would make every read ambiguous.
pub const SETTINGS_ID: &str = "set_heric";

/// The `hotKeyRegistry` body key.
const HOT_KEY_REGISTRY: &str = "hotKeyRegistry";

/// The retired flag whose value folds into `pluginSandboxEnabled`.
const LEGACY_SANDBOX_FLAG: &str = "templateTagSandboxEnabled";

/// The flag the retired one folds into.
const SANDBOX_FLAG: &str = "pluginSandboxEnabled";

/// The actions the original's default hotkey registry binds.
///
/// Transcribed from `common-src/hotkeys.ts:10-46`. A binding for an action
/// outside this list is one the app can no longer dispatch, so
/// [`migrate_hotkeys`] drops it — which is what the original's
/// `migrateEnsureHotKeys` does.
const KNOWN_ACTIONS: [&str; 35] = [
    "workspace_showSettings",
    "request_showSettings",
    "preferences_showKeyboardShortcuts",
    "preferences_showGeneral",
    "request_quickSwitch",
    "plugin_reload",
    "showAutocomplete",
    "request_send",
    "request_showOptions",
    "environment_showEditor",
    "environment_showSwitchMenu",
    "environment_showSwitchProjectMenu",
    "request_toggleHttpMethodMenu",
    "request_toggleHistory",
    "request_focusUrl",
    "request_showGenerateCodeEditor",
    "sidebar_focusFilter",
    "sidebar_showCreateDropdown",
    "sidebar_toggle",
    "sidebar_rename",
    "response_focus",
    "showCookiesEditor",
    "request_createHTTP",
    "request_showDelete",
    "request_showCreateFolder",
    "request_showDuplicate",
    "request_togglePin",
    "environment_showVariableSourceAndValue",
    "beautifyRequestBody",
    "graphql_explorer_focus_filter",
    "close_tab",
    "tab_nextTab",
    "tab_previousTab",
    "tab_reopenClosedTab",
    "request_openInNewTab",
];

/// The `init()` table, transcribed from `models/settings.ts:28-101`.
///
/// `hotKeyRegistry` is the one entry that is not the original's value; see the
/// module documentation. `forceVerticalLayout` is `false` here because the
/// original only raises it under `PLAYWRIGHT_TEST`, which Heric has no
/// equivalent of.
pub(crate) static DEFAULTS: [(&str, DefaultValue); 68] = [
    ("autoDetectColorScheme", DefaultValue::Bool(false)),
    ("autoHideMenuBar", DefaultValue::Bool(false)),
    ("autocompleteDelay", DefaultValue::Int(1200)),
    ("clearOAuth2SessionOnRestart", DefaultValue::Bool(true)),
    ("darkTheme", DefaultValue::Str("default")),
    ("debugMode", DefaultValue::Bool(false)),
    ("deviceId", DefaultValue::Null),
    ("disableHtmlPreviewJs", DefaultValue::Bool(false)),
    ("disableResponsePreviewLinks", DefaultValue::Bool(false)),
    ("disableUpdateNotification", DefaultValue::Bool(false)),
    ("editorFontSize", DefaultValue::Int(11)),
    ("editorIndentSize", DefaultValue::Int(2)),
    ("editorIndentWithTabs", DefaultValue::Bool(true)),
    ("editorKeyMap", DefaultValue::Str("default")),
    (
        "enableKeyMapForInlineTextEditors",
        DefaultValue::Bool(false),
    ),
    ("editorLineWrapping", DefaultValue::Bool(true)),
    ("enableAnalytics", DefaultValue::Bool(true)),
    ("showVariableSourceAndValue", DefaultValue::Bool(false)),
    ("filterResponsesByEnv", DefaultValue::Bool(false)),
    ("followRedirects", DefaultValue::Bool(true)),
    ("fontInterface", DefaultValue::Null),
    ("fontMonospace", DefaultValue::Null),
    ("fontSize", DefaultValue::Int(13)),
    ("fontVariantLigatures", DefaultValue::Bool(false)),
    ("forceVerticalLayout", DefaultValue::Bool(false)),
    ("hasKonnectPat", DefaultValue::Bool(false)),
    (
        "hideSecretValuesInPreviewAndConsole",
        DefaultValue::Bool(true),
    ),
    ("konnectOrganizationId", DefaultValue::Null),
    ("hotKeyRegistry", DefaultValue::EmptyObject),
    ("httpProxy", DefaultValue::Str("")),
    ("httpsProxy", DefaultValue::Str("")),
    ("lightTheme", DefaultValue::Str("studio-light")),
    ("maxHistoryResponses", DefaultValue::Int(20)),
    ("maxRedirects", DefaultValue::Int(10)),
    ("maxTimelineDataSizeKB", DefaultValue::Int(10)),
    ("pluginNodeExtraCerts", DefaultValue::Str("")),
    ("noProxy", DefaultValue::Str("")),
    ("proxyScope", DefaultValue::Str("requests")),
    ("nunjucksPowerUserMode", DefaultValue::Bool(false)),
    ("pluginConfig", DefaultValue::EmptyObject),
    ("pluginPath", DefaultValue::Str("")),
    ("preferredHttpVersion", DefaultValue::Str("default")),
    ("proxyEnabled", DefaultValue::Bool(false)),
    ("showPasswords", DefaultValue::Bool(false)),
    ("sidebarFocusForCollections", DefaultValue::Bool(true)),
    ("hasSeenSidebarFocusOnboarding", DefaultValue::Bool(false)),
    ("theme", DefaultValue::Str("default")),
    ("timeout", DefaultValue::Int(30000)),
    ("updateAutomatically", DefaultValue::Bool(true)),
    ("updateChannel", DefaultValue::Str("stable")),
    ("useBulkHeaderEditor", DefaultValue::Bool(false)),
    ("useBulkParametersEditor", DefaultValue::Bool(false)),
    ("validateAuthSSL", DefaultValue::Bool(true)),
    ("validateSSL", DefaultValue::Bool(true)),
    ("saveVaultKeyLocally", DefaultValue::Bool(true)),
    ("enableVaultInScripts", DefaultValue::Bool(false)),
    ("saveVaultKeyToOSSecretManager", DefaultValue::Bool(true)),
    ("vaultSecretCacheDuration", DefaultValue::Int(30)),
    ("dataFolders", DefaultValue::EmptyArray),
    ("scriptSandboxEnabled", DefaultValue::Bool(true)),
    ("scriptStrictModeEnabled", DefaultValue::Bool(true)),
    ("useQuickJsScriptSandbox", DefaultValue::Bool(false)),
    ("pluginSandboxEnabled", DefaultValue::Bool(false)),
    ("disabledSecurityRules", DefaultValue::EmptyArray),
    ("disabledBlockedProperties", DefaultValue::EmptyArray),
    ("disabledBlockedRoots", DefaultValue::EmptyArray),
    ("npmRegistryUrl", DefaultValue::Str("")),
    ("enableLegacyUnitTests", DefaultValue::Bool(false)),
];

/// Returns a fresh copy of the settings defaults.
#[must_use]
pub fn defaults() -> Map<String, Value> {
    DEFAULTS
        .iter()
        .map(|(key, value)| ((*key).to_owned(), value.to_value()))
        .collect()
}

/// Builds the settings singleton as it looks on a fresh installation.
///
/// The id is [`SETTINGS_ID`] rather than a generated one, so the document is
/// recognisable across runs and installations.
#[must_use]
pub fn singleton(now: i64) -> Doc {
    Doc {
        meta: DocMeta {
            id: Id::new(SETTINGS_ID),
            kind: DocKind::Settings,
            parent: None,
            modified: now,
            created: now,
            private: None,
            name: None,
        },
        body: defaults(),
    }
}

/// Restores what a settings document is missing and reports what it fixed.
///
/// Two things are repaired. A defaulted key that is absent is put back, because
/// the original's pruning loop deletes any key a model does not declare and a
/// settings document written by an older version can therefore be missing one
/// that a newer version reads. A hotkey bound to the same combination as
/// another action is *reported* and left alone: the original lets the user pick
/// which binding to keep, and silently rebinding one would take a shortcut away
/// without saying so.
///
/// The returned names are in the order they were found, and each appears once.
#[must_use]
pub fn repair(doc: &mut Doc) -> Vec<&'static str> {
    let mut fixed = Vec::new();
    for (key, value) in &DEFAULTS {
        if !doc.body.contains_key(*key) {
            doc.body.insert((*key).to_owned(), value.to_value());
            fixed.push(*key);
        }
    }
    fixed.extend(hotkey_conflicts(&doc.body));
    fixed
}

/// Names the actions whose binding collides with an earlier action's.
///
/// A collision is per platform: the same combination on macOS and on
/// Windows/Linux is not a conflict, because the two never run together. The
/// comparison is the original's `areSameKeyCombinations`
/// (`common-src/hotkeys.ts:216-226`): the key code plus the four modifiers,
/// with an absent modifier equal to `false`.
fn hotkey_conflicts(body: &Map<String, Value>) -> Vec<&'static str> {
    let Some(registry) = body.get(HOT_KEY_REGISTRY).and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut seen: Vec<(&str, String)> = Vec::new();
    let mut conflicts = Vec::new();
    for (action, bindings) in registry {
        let Some(bindings) = bindings.as_object() else {
            continue;
        };
        let mut collides = false;
        for platform in ["macKeys", "winLinuxKeys"] {
            let Some(combinations) = bindings.get(platform).and_then(Value::as_array) else {
                continue;
            };
            for combination in combinations {
                let signature = combination_signature(combination);
                let key = (platform, signature);
                if seen.contains(&key) {
                    collides = true;
                } else {
                    seen.push(key);
                }
            }
        }
        if collides {
            // Only a known action can be named as `&'static str`; an unknown one
            // is dropped by `migrate_hotkeys` and is not worth reporting.
            if let Some(name) = KNOWN_ACTIONS.iter().find(|known| **known == action) {
                conflicts.push(*name);
            }
        }
    }
    conflicts
}

/// A key combination reduced to the fields the original compares.
fn combination_signature(combination: &Value) -> String {
    let Some(map) = combination.as_object() else {
        return String::new();
    };
    let key_code = map
        .get("keyCode")
        .map_or_else(String::new, Value::to_string);
    let flag = |name: &str| {
        map.get(name)
            .is_some_and(|value| value == &Value::Bool(true))
    };
    format!(
        "{key_code}/{}/{}/{}/{}",
        flag("alt"),
        flag("shift"),
        flag("ctrl"),
        flag("meta")
    )
}

/// Applies the original's settings migration to a settings body.
///
/// Three steps, in the original's order (`init-model/settings.ts:5-100`):
/// hotkeys for actions the app no longer knows are dropped, defaults are
/// re-added for actions that lost their binding, and the retired
/// `templateTagSandboxEnabled` flag is folded into `pluginSandboxEnabled` and
/// deleted.
///
/// The second step is a no-op today because [`defaults`] carries no bindings;
/// it is written out so that porting the default registry is a change to one
/// table rather than to this function.
#[must_use]
pub fn migrate_hotkeys(mut map: Map<String, Value>) -> Map<String, Value> {
    let registry = map
        .remove(HOT_KEY_REGISTRY)
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    let mut kept = Map::new();
    for (action, bindings) in registry {
        if KNOWN_ACTIONS.contains(&action.as_str()) {
            kept.insert(action, bindings);
        }
    }
    map.insert(HOT_KEY_REGISTRY.to_owned(), Value::Object(kept));

    if let Some(legacy) = map.remove(LEGACY_SANDBOX_FLAG) {
        let opted_in = legacy == Value::Bool(true);
        let already_on = map.get(SANDBOX_FLAG) == Some(&Value::Bool(true));
        if opted_in && !already_on {
            map.insert(SANDBOX_FLAG.to_owned(), Value::Bool(true));
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// Every key the original's `init()` writes, written out so that a key
    /// dropped from [`DEFAULTS`] fails here rather than silently.
    const EXPECTED_KEYS: [&str; 68] = [
        "autoDetectColorScheme",
        "autoHideMenuBar",
        "autocompleteDelay",
        "clearOAuth2SessionOnRestart",
        "darkTheme",
        "debugMode",
        "deviceId",
        "disableHtmlPreviewJs",
        "disableResponsePreviewLinks",
        "disableUpdateNotification",
        "editorFontSize",
        "editorIndentSize",
        "editorIndentWithTabs",
        "editorKeyMap",
        "enableKeyMapForInlineTextEditors",
        "editorLineWrapping",
        "enableAnalytics",
        "showVariableSourceAndValue",
        "filterResponsesByEnv",
        "followRedirects",
        "fontInterface",
        "fontMonospace",
        "fontSize",
        "fontVariantLigatures",
        "forceVerticalLayout",
        "hasKonnectPat",
        "hideSecretValuesInPreviewAndConsole",
        "konnectOrganizationId",
        "hotKeyRegistry",
        "httpProxy",
        "httpsProxy",
        "lightTheme",
        "maxHistoryResponses",
        "maxRedirects",
        "maxTimelineDataSizeKB",
        "pluginNodeExtraCerts",
        "noProxy",
        "proxyScope",
        "nunjucksPowerUserMode",
        "pluginConfig",
        "pluginPath",
        "preferredHttpVersion",
        "proxyEnabled",
        "showPasswords",
        "sidebarFocusForCollections",
        "hasSeenSidebarFocusOnboarding",
        "theme",
        "timeout",
        "updateAutomatically",
        "updateChannel",
        "useBulkHeaderEditor",
        "useBulkParametersEditor",
        "validateAuthSSL",
        "validateSSL",
        "saveVaultKeyLocally",
        "enableVaultInScripts",
        "saveVaultKeyToOSSecretManager",
        "vaultSecretCacheDuration",
        "dataFolders",
        "scriptSandboxEnabled",
        "scriptStrictModeEnabled",
        "useQuickJsScriptSandbox",
        "pluginSandboxEnabled",
        "disabledSecurityRules",
        "disabledBlockedProperties",
        "disabledBlockedRoots",
        "npmRegistryUrl",
        "enableLegacyUnitTests",
    ];

    #[test]
    fn every_default_key_is_present() {
        let defaults = defaults();
        for key in EXPECTED_KEYS {
            assert!(defaults.contains_key(key), "`{key}` is missing");
        }
        assert_eq!(defaults.len(), EXPECTED_KEYS.len());
    }

    #[test]
    fn the_defaults_carry_the_originals_values() {
        let defaults = defaults();
        assert_eq!(defaults.get("timeout"), Some(&json!(30_000)));
        assert_eq!(defaults.get("lightTheme"), Some(&json!("studio-light")));
        assert_eq!(defaults.get("darkTheme"), Some(&json!("default")));
        assert_eq!(defaults.get("proxyScope"), Some(&json!("requests")));
        assert_eq!(defaults.get("maxHistoryResponses"), Some(&json!(20)));
        // A `null` default is present and null, not absent: the pruning rule
        // treats an absent key as missing and would restore it.
        assert_eq!(defaults.get("deviceId"), Some(&Value::Null));
        assert_eq!(defaults.get("fontInterface"), Some(&Value::Null));
        assert_eq!(defaults.get("konnectOrganizationId"), Some(&Value::Null));
        assert_eq!(defaults.get("dataFolders"), Some(&json!([])));
        assert_eq!(defaults.get("pluginConfig"), Some(&json!({})));
    }

    #[test]
    fn a_missing_key_is_restored_and_reported() {
        let mut doc = singleton(1_000);
        doc.body.remove("timeout");
        let fixed = repair(&mut doc);
        assert_eq!(doc.body.get("timeout"), Some(&json!(30_000)));
        assert_eq!(fixed.iter().filter(|name| **name == "timeout").count(), 1);
    }

    #[test]
    fn a_present_key_is_never_overwritten() {
        let mut doc = singleton(1_000);
        doc.body.insert("timeout".to_owned(), json!(5_000));
        let fixed = repair(&mut doc);
        assert_eq!(doc.body.get("timeout"), Some(&json!(5_000)));
        assert!(fixed.is_empty(), "nothing was missing: {fixed:?}");
    }

    #[test]
    fn a_hotkey_bound_twice_is_reported_and_left_to_the_user() {
        // The original lets the user choose which binding to keep, so repair
        // names the collision and rewrites neither.
        let mut doc = singleton(1_000);
        let binding = json!({"macKeys": [{"meta": true, "keyCode": 78}], "winLinuxKeys": []});
        doc.body.insert(
            "hotKeyRegistry".to_owned(),
            json!({
                "request_send": binding,
                "request_showOptions": binding,
            }),
        );
        let fixed = repair(&mut doc);
        assert_eq!(fixed, vec!["request_showOptions"]);
        let registry = doc.body.get("hotKeyRegistry").and_then(Value::as_object);
        assert_eq!(
            registry.and_then(|map| map.get("request_send")),
            Some(&binding)
        );
        assert_eq!(
            registry.and_then(|map| map.get("request_showOptions")),
            Some(&binding)
        );
    }

    #[test]
    fn the_same_combination_on_two_platforms_is_not_a_conflict() {
        let mut doc = singleton(1_000);
        doc.body.insert(
            "hotKeyRegistry".to_owned(),
            json!({
                "request_send": {"macKeys": [{"meta": true, "keyCode": 78}], "winLinuxKeys": []},
                "request_showOptions": {"macKeys": [], "winLinuxKeys": [{"meta": true, "keyCode": 78}]},
            }),
        );
        assert!(repair(&mut doc).is_empty());
    }

    #[test]
    fn the_settings_id_is_the_singletons() {
        let doc = singleton(0);
        assert_eq!(doc.meta.id.as_str(), SETTINGS_ID);
        assert!(doc.meta.id.starts_with_kind(DocKind::Settings));
        assert_eq!(doc.meta.kind, DocKind::Settings);
        assert_eq!(doc.meta.parent, None);
        assert_eq!(doc.meta.private, None);
    }

    #[test]
    fn a_settings_document_survives_the_pruning_pass() -> Result<(), Box<dyn std::error::Error>> {
        // `init::apply` deletes any key a model does not declare, so a Settings
        // document that went through it would lose everything if the model were
        // unmodelled or the table incomplete.
        let mut doc = singleton(1_000);
        crate::init::apply(&mut doc)?;
        assert_eq!(doc.body.len(), EXPECTED_KEYS.len());
        assert_eq!(doc.body.get("timeout"), Some(&json!(30_000)));
        Ok(())
    }

    #[test]
    fn a_hotkey_for_an_unknown_action_is_dropped() {
        let map = json!({
            "hotKeyRegistry": {
                "request_send": {"macKeys": [], "winLinuxKeys": []},
                "retired_action": {"macKeys": [], "winLinuxKeys": []},
            },
        });
        let migrated = migrate_hotkeys(map.as_object().cloned().unwrap_or_default());
        let registry = migrated.get("hotKeyRegistry").and_then(Value::as_object);
        assert!(registry.is_some_and(|map| map.contains_key("request_send")));
        assert!(registry.is_some_and(|map| !map.contains_key("retired_action")));
    }

    #[test]
    fn the_retired_sandbox_flag_folds_forward_and_is_deleted() {
        let map = json!({
            "templateTagSandboxEnabled": true,
            "pluginSandboxEnabled": false,
        });
        let migrated = migrate_hotkeys(map.as_object().cloned().unwrap_or_default());
        assert_eq!(migrated.get("pluginSandboxEnabled"), Some(&json!(true)));
        assert!(!migrated.contains_key("templateTagSandboxEnabled"));
    }

    #[test]
    fn a_retired_flag_that_was_off_leaves_the_new_one_alone() {
        let map = json!({
            "templateTagSandboxEnabled": false,
            "pluginSandboxEnabled": false,
        });
        let migrated = migrate_hotkeys(map.as_object().cloned().unwrap_or_default());
        assert_eq!(migrated.get("pluginSandboxEnabled"), Some(&json!(false)));
        assert!(!migrated.contains_key("templateTagSandboxEnabled"));
    }

    #[test]
    fn a_missing_registry_is_created_empty() {
        let migrated = migrate_hotkeys(Map::new());
        assert_eq!(migrated.get("hotKeyRegistry"), Some(&json!({})));
    }
}
