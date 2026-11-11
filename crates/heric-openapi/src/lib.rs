//! OpenAPI and Swagger support.
//!
//! Two jobs: parse a specification, and turn it into a collection. The split
//! matters because linting (Design mode) needs only the first and must run
//! without building documents, while import needs both and runs once.
//!
//! Only OpenAPI 3 and Swagger 2.0 are parsed. WSDL is not: the original routes it
//! through a patched JavaScript package and then through its Postman importer,
//! which is a different pipeline with different failure modes (`IO-010`).

#![deny(unsafe_code)]

mod convert;
mod error;
mod lint;
mod spec;

pub use convert::{CollectionBuilder, ConvertOptions, ConvertedRequest, SecurityMapping, build};
pub use error::{OpenApiDiagnostic, OpenApiError};
pub use lint::{LintReport, LintViolation, Ruleset, Severity};
pub use spec::{
    MediaType, OAuth2Flow, Operation, Parameter, ParameterLocation, PathItem, Schema,
    SecurityScheme, Server, ServerVariable, Spec, SpecVersion,
};

/// Returns the version of `heric-openapi`.
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Parses a specification from YAML or JSON.
///
/// # Errors
///
/// Returns [`OpenApiError::Syntax`] when the document does not parse and
/// [`OpenApiError::UnsupportedVersion`] when it is neither OpenAPI 3 nor Swagger
/// 2.0. A document with a `swagger: "2.0"` key is upgraded into the v3 shape
/// before conversion, so the rest of the crate sees one model.
pub fn parse(source: &str) -> Result<Spec, OpenApiError> {
    let _ = source;
    todo!("M5: parse an OpenAPI or Swagger document")
}

/// Parses and converts in one step.
///
/// # Errors
///
/// Returns [`OpenApiError`] from either stage.
pub fn into_collection(
    source: &str,
    options: &ConvertOptions,
) -> Result<CollectionBuilder, OpenApiError> {
    let spec = parse(source)?;
    let _ = (&spec, options);
    todo!("M5: convert a parsed spec into a collection")
}

#[cfg(test)]
mod tests {
    #[test]
    fn version_is_semver_like() {
        assert_eq!(super::version().split('.').count(), 3);
    }
}
