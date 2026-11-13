//! Domain types for Heric.
//!
//! This crate is the shared vocabulary of the workspace: identifiers, the
//! document model, the mutation and query types, and the wire types the
//! request pipeline passes between crates. It performs no I/O and has no
//! Heric dependencies, which is what lets every other library crate depend on
//! it without a cycle.
//!
//! Layout follows the Insomnia data model as recorded in
//! `docs/inventory/models.md`.

#![deny(unsafe_code)]

mod diagnostic;
mod doc;
mod error;
mod id;
pub mod init;
mod kind;
mod wire;

pub use diagnostic::{Diagnostic, MessageKey, MessageValue};
pub use doc::{Change, Doc, DocMeta, Order, Patch, PatchOp, Query, Request, Workspace};
pub use error::ModelError;
pub use id::Id;
pub use init::{SCRATCHPAD_ORGANIZATION, SCRATCHPAD_PROJECT, SCRATCHPAD_WORKSPACE};
pub use kind::DocKind;
pub use wire::{Body, Execution, Header, PreparedRequest, TimelineEntry, TimelineKind, Timings};

/// Returns the version of `heric-model`, which tracks the workspace version.
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Generates a fresh random identifier with no type prefix.
///
/// Prefer [`Id::generate`], which prefixes the identifier with the kind it
/// belongs to. This function exists for the few places that need an opaque
/// identifier, such as cookie ids, which the original also generates without a
/// prefix.
#[must_use]
pub fn generate_id() -> Id {
    Id::opaque(uuid::Uuid::new_v4().simple().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_semver_like() {
        assert_eq!(version().split('.').count(), 3);
    }

    #[test]
    fn opaque_ids_are_unique() {
        assert_ne!(generate_id(), generate_id());
    }
}
