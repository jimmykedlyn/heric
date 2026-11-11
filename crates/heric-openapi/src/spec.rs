//! The specification model.
//!
//! A deliberately narrow model: what import and lint need, and nothing more. A
//! full OpenAPI model would be an enormous surface to keep correct, and every
//! field Heric does not read is a field it would have to keep in sync with the
//! specification's revisions.

/// Which specification dialect a document is written in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SpecVersion {
    /// Swagger 2.0.
    Swagger2,
    /// OpenAPI 3.0.x, the default because a document that names no version is
    /// read as the current 3.0 shape.
    #[default]
    OpenApi30,
    /// OpenAPI 3.1.x, which allows JSON Schema 2020-12.
    OpenApi31,
}

impl SpecVersion {
    /// The version as written.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Swagger2 => "2.0",
            Self::OpenApi30 => "3.0",
            Self::OpenApi31 => "3.1",
        }
    }

    /// Whether the dialect needs the v2-to-v3 upgrade step.
    #[must_use]
    pub fn is_swagger(self) -> bool {
        matches!(self, Self::Swagger2)
    }
}

/// A parsed specification.
#[derive(Debug, Clone, Default)]
pub struct Spec {
    /// Which dialect.
    pub version: SpecVersion,
    /// The document's title, used to name the imported collection.
    pub title: String,
    /// The document's description.
    pub description: Option<String>,
    /// API version string from `info.version`.
    pub api_version: String,
    /// The servers, or the single implicit one from Swagger's `host`/`basePath`.
    pub servers: Vec<Server>,
    /// Paths, keyed by the path template.
    pub paths: Vec<(String, PathItem)>,
    /// Components, keyed by name.
    pub schemas: Vec<(String, Schema)>,
    /// Security schemes, keyed by name.
    pub security_schemes: Vec<(String, SecurityScheme)>,
    /// The document's raw content, kept for linting rules that need to see what
    /// the model drops.
    pub raw: serde_json::Value,
}

impl Spec {
    /// Whether the document has anything to import.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }
}

/// A server the API is served from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Server {
    /// The base URL, possibly with `{variable}` placeholders.
    pub url: String,
    /// A description.
    pub description: Option<String>,
    /// Variables the URL uses.
    pub variables: Vec<ServerVariable>,
}

/// A server URL variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerVariable {
    /// The variable's name.
    pub name: String,
    /// Its default.
    pub default: String,
    /// Allowed values.
    pub enum_values: Vec<String>,
}

/// One path and its operations.
#[derive(Debug, Clone, Default)]
pub struct PathItem {
    /// The operations, keyed by method.
    pub operations: Vec<(String, Operation)>,
}

/// One operation.
#[derive(Debug, Clone, Default)]
pub struct Operation {
    /// `operationId`, used to name the request.
    pub operation_id: Option<String>,
    /// A summary.
    pub summary: Option<String>,
    /// A description, possibly markdown.
    pub description: Option<String>,
    /// The tags, which decide which folder the request lands in.
    pub tags: Vec<String>,
    /// Parameters, from the path item and the operation merged.
    pub parameters: Vec<Parameter>,
    /// The request body's media types.
    pub request_body: Vec<MediaType>,
    /// Whether the body is required.
    pub body_required: bool,
    /// Security requirement names, empty meaning the operation is public.
    pub security: Vec<String>,
    /// Whether the operation is deprecated.
    pub deprecated: bool,
}

/// One parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    /// The name.
    pub name: String,
    /// Where it goes.
    pub location: ParameterLocation,
    /// Whether it is required.
    pub required: bool,
    /// A description.
    pub description: Option<String>,
    /// Its schema.
    pub schema: Schema,
    /// A sample value, which becomes the request's default.
    pub example: Option<serde_json::Value>,
}

/// Where a parameter goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParameterLocation {
    /// In the path.
    Path,
    /// In the query string.
    Query,
    /// In a header.
    Header,
    /// In a cookie.
    Cookie,
}

impl ParameterLocation {
    /// The location as written in the specification.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Path => "path",
            Self::Query => "query",
            Self::Header => "header",
            Self::Cookie => "cookie",
        }
    }
}

/// A media type and its schema.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaType {
    /// The content type.
    pub content_type: String,
    /// The schema.
    pub schema: Schema,
    /// A sample payload, used to seed the request body.
    pub example: Option<serde_json::Value>,
}

/// A schema, as far as Heric needs to read it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Schema {
    /// The JSON type.
    pub kind: Option<String>,
    /// The format, `date-time` and `binary` being the two that change import.
    pub format: Option<String>,
    /// A `$ref`, unresolved.
    pub reference: Option<String>,
    /// Properties, for an object.
    pub properties: Vec<(String, Schema)>,
    /// The item schema, for an array.
    pub items: Option<Box<Schema>>,
    /// Whether the value may be null.
    pub nullable: bool,
    /// Enumerated values.
    pub enum_values: Vec<String>,
}

impl Schema {
    /// Whether the schema describes an uploaded file, which becomes a multipart
    /// part rather than a JSON field.
    #[must_use]
    pub fn is_binary(&self) -> bool {
        matches!(self.format.as_deref(), Some("binary") | Some("byte"))
    }

    /// Whether the schema describes a multipart form, which decides whether the
    /// imported request is urlencoded or multipart.
    #[must_use]
    pub fn is_object(&self) -> bool {
        matches!(self.kind.as_deref(), Some("object")) || !self.properties.is_empty()
    }
}

/// A security scheme.
#[derive(Debug, Clone, PartialEq)]
pub enum SecurityScheme {
    /// HTTP Basic.
    Basic,
    /// Bearer.
    Bearer {
        /// The bearer format hint.
        format: Option<String>,
    },
    /// An API key.
    ApiKey {
        /// The header, query parameter or cookie name.
        name: String,
        /// Where it goes.
        location: ParameterLocation,
    },
    /// OAuth 2.
    OAuth2 {
        /// The flows.
        flows: Vec<OAuth2Flow>,
    },
    /// OpenID Connect.
    OpenIdConnect {
        /// The discovery URL.
        url: String,
    },
    /// Swagger 2.0's combined basic/API-key scheme in a non-standard place.
    Other,
}

/// One OAuth 2 flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OAuth2Flow {
    /// Which flow.
    pub kind: OAuth2FlowKind,
    /// The authorisation URL.
    pub authorization_url: Option<String>,
    /// The token URL.
    pub token_url: Option<String>,
    /// Available scopes.
    pub scopes: Vec<(String, String)>,
}

/// The OAuth 2 flow names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OAuth2FlowKind {
    /// `implicit`.
    Implicit,
    /// `password`.
    Password,
    /// `clientCredentials`.
    ClientCredentials,
    /// `authorizationCode`.
    AuthorizationCode,
}

impl OAuth2FlowKind {
    /// The flow as written.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Implicit => "implicit",
            Self::Password => "password",
            Self::ClientCredentials => "clientCredentials",
            Self::AuthorizationCode => "authorizationCode",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_binary_schema_is_a_file_part() {
        let schema = Schema {
            format: Some("binary".to_owned()),
            ..Schema::default()
        };
        assert!(schema.is_binary());
    }

    #[test]
    fn an_object_is_recognised_from_its_properties() {
        let schema = Schema {
            properties: vec![("a".to_owned(), Schema::default())],
            ..Schema::default()
        };
        assert!(schema.is_object());
    }

    #[test]
    fn swagger_versions_are_flagged() {
        assert!(SpecVersion::Swagger2.is_swagger());
        assert!(!SpecVersion::OpenApi31.is_swagger());
    }
}
