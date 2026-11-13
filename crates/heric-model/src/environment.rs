//! Environment values and the base environment's identity.
//!
//! An environment's variables live in two shapes: the original stores them as a
//! JSON object of `{name: value}`, while its editor and several of its helpers
//! work with a list of `{name, value}` rows. Both conversions live here so the
//! migration reader and the editor agree on what a conversion means.

use serde_json::{Map, Value};

use crate::{Doc, DocKind, Id, init};

/// The name every workspace's base environment has.
pub const BASE_NAME: &str = "Base Environment";

/// Turns an environment's `data` object into editor rows.
///
/// A non-object input yields an empty list rather than an error: a document
/// whose `data` is malformed should still open, and the caller can compare the
/// input against `[]` to notice.
///
/// The output order is the object's iteration order, which with `serde_json`
/// built without `preserve_order` is alphabetical. Claims about insertion order
/// would be false, so none are made.
#[must_use]
pub fn kv_from_json(data: &Value) -> Vec<Value> {
    let Value::Object(map) = data else {
        return Vec::new();
    };
    map.iter()
        .map(|(name, value)| {
            let mut row = Map::new();
            row.insert("name".to_owned(), Value::String(name.clone()));
            // The original stores a KV value as a string, so a number or a bool
            // becomes its JSON spelling rather than keeping its type.
            let value = match value {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            row.insert("value".to_owned(), Value::String(value));
            Value::Object(row)
        })
        .collect()
}

/// Turns editor rows back into an environment's `data` object.
///
/// A row with no `name` is skipped: it names nothing and cannot become a key.
#[must_use]
pub fn json_from_kv(rows: &[Value]) -> Value {
    let mut map = Map::new();
    for row in rows {
        let Some(name) = row.get("name").and_then(Value::as_str) else {
            continue;
        };
        let value = row.get("value").and_then(Value::as_str).unwrap_or_default();
        map.insert(name.to_owned(), Value::String(value.to_owned()));
    }
    Value::Object(map)
}

/// The base environment belonging to `parent`.
///
/// Pure: it builds the document and returns it, and the caller inserts it. The
/// id is `env_` plus a stable hash of the parent id, so two calls for one
/// workspace produce one environment and two workspaces never collide.
///
/// The original hashes with SHA-1 to stay idempotent against its cloud ids.
/// Heric needs only "deterministic per parent, distinct across parents", which
/// FNV-1a gives without a digest dependency. The consequence is that this id
/// will not match the original's for the same parent, which matters only if
/// Heric ever syncs against Insomnia's cloud — recorded as a known deviation.
#[must_use]
pub fn base_for(parent: Id) -> Doc {
    let id = Id::new(format!("env_{:016x}", fnv1a_64(parent.as_str().as_bytes())));
    let mut doc = init::document(DocKind::Environment, 0);
    doc.meta.id = id;
    doc.meta.name = Some(BASE_NAME.to_owned());
    doc.meta.parent = Some(parent);
    doc.body
        .insert("name".to_owned(), Value::String(BASE_NAME.to_owned()));
    doc.body
        .insert("environmentType".to_owned(), Value::String("kv".to_owned()));
    doc
}

/// FNV-1a, 64-bit.
///
/// Chosen because the id is persisted and compared across upgrades: a hash whose
/// output Rust does not promise to keep stable would create a second base
/// environment for every workspace on a compiler bump. FNV-1a is defined by its
/// arithmetic, so it cannot drift, and it needs no crate.
fn fnv1a_64(bytes: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET_BASIS;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn the_base_environment_id_is_deterministic_for_a_parent() {
        assert_eq!(
            base_for(Id::new("wrk_1")).meta.id,
            base_for(Id::new("wrk_1")).meta.id
        );
        assert_ne!(
            base_for(Id::new("wrk_1")).meta.id,
            base_for(Id::new("wrk_2")).meta.id
        );
    }

    #[test]
    fn the_base_environment_id_is_prefixed_and_hex() {
        let id = base_for(Id::new("wrk_1")).meta.id;
        let raw = id.as_str();
        let hex = raw.strip_prefix("env_").unwrap_or("");
        assert_eq!(hex.len(), 16, "id was `{raw}`");
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()), "id was `{raw}`");
    }

    #[test]
    fn the_base_environment_is_named_and_parented() {
        let doc = base_for(Id::new("wrk_1"));
        assert_eq!(doc.meta.name.as_deref(), Some(BASE_NAME));
        assert_eq!(doc.meta.parent, Some(Id::new("wrk_1")));
        assert_eq!(doc.meta.kind, DocKind::Environment);
        assert_eq!(doc.body.get("environmentType"), Some(&json!("kv")));
    }

    #[test]
    fn kv_rows_round_trip() {
        let data = json!({"host": "example.com", "port": "8080"});
        let rows = kv_from_json(&data);
        assert_eq!(rows.len(), 2);
        assert_eq!(json_from_kv(&rows), data);
    }

    #[test]
    fn a_non_object_data_yields_no_rows() {
        assert!(kv_from_json(&json!([])).is_empty());
        assert!(kv_from_json(&json!("text")).is_empty());
        assert!(kv_from_json(&Value::Null).is_empty());
    }

    #[test]
    fn a_nameless_row_is_skipped() {
        let rows = vec![json!({"value": "x"}), json!({"name": "a", "value": "1"})];
        assert_eq!(json_from_kv(&rows), json!({"a": "1"}));
    }

    #[test]
    fn a_non_string_value_stringifies() {
        let rows = kv_from_json(&json!({"n": 5, "b": true}));
        let values: Vec<&str> = rows
            .iter()
            .filter_map(|row| row.get("value").and_then(Value::as_str))
            .collect();
        assert_eq!(values, ["true", "5"]);
    }
}
