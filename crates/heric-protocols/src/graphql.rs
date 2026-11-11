//! GraphQL: the send envelope, introspection and operation selection.

use heric_model::PreparedRequest;

/// One GraphQL operation found in a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphQlOperation {
    /// The operation name, empty for an anonymous query.
    pub name: String,
    /// `query`, `mutation` or `subscription`.
    pub kind: GraphQlOperationKind,
    /// Byte span of the operation in the original document, used to dim the
    /// others while one is selected.
    pub span: (usize, usize),
}

/// The three operation kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphQlOperationKind {
    /// A read.
    Query,
    /// A write.
    Mutation,
    /// A live subscription, carried over WebSocket rather than HTTP.
    Subscription,
}

/// The JSON body a GraphQL request sends.
///
/// Built *after* rendering, because the original renders the query, then the
/// variables, then wraps them (`PRO-002`). A plugin can also rewrite the
/// envelope before it is sent, which is why this is a value the pipeline hands
/// over rather than something the HTTP engine derives.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphQlPayload {
    /// The query text.
    pub query: String,
    /// Variables, when any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<serde_json::Value>,
    /// The operation to run, when the document has more than one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_name: Option<String>,
}

impl GraphQlPayload {
    /// Sends the payload as a request's body.
    ///
    /// # Errors
    ///
    /// Returns [`crate::ProtocolError::Encode`] when the payload cannot be
    /// serialised, which for a JSON value means a bug rather than bad input.
    pub fn into_request(
        self,
        base: &PreparedRequest,
    ) -> Result<PreparedRequest, crate::ProtocolError> {
        let text = serde_json::to_string(&self)
            .map_err(|error| crate::ProtocolError::Encode(error.to_string()))?;
        let mut request = base.clone();
        request.method = "POST".to_owned();
        request.body = heric_model::Body::text(text, "application/json");
        Ok(request)
    }
}

/// The standard introspection document.
///
/// Served from a constant rather than typed out per call so the result can be
/// cached: the original caches introspection per collection and only refetches
/// when the user asks.
pub struct IntrospectionQuery;

impl IntrospectionQuery {
    /// The introspection query text.
    #[must_use]
    pub fn text() -> &'static str {
        include_str!("introspection.graphql")
    }

    /// The payload to send.
    #[must_use]
    pub fn payload() -> GraphQlPayload {
        GraphQlPayload {
            query: Self::text().to_owned(),
            variables: None,
            operation_name: Some("IntrospectionQuery".to_owned()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_payload_omits_absent_fields() -> Result<(), Box<dyn std::error::Error>> {
        let payload = GraphQlPayload {
            query: "{ me { id } }".to_owned(),
            variables: None,
            operation_name: None,
        };
        let json = serde_json::to_string(&payload)?;
        assert_eq!(json, r#"{"query":"{ me { id } }"}"#);
        Ok(())
    }

    #[test]
    fn the_payload_uses_camel_case_keys() -> Result<(), Box<dyn std::error::Error>> {
        let payload = GraphQlPayload {
            query: "query Q { a }".to_owned(),
            variables: Some(serde_json::json!({ "x": 1 })),
            operation_name: Some("Q".to_owned()),
        };
        let json = serde_json::to_string(&payload)?;
        assert!(json.contains("\"operationName\":\"Q\""));
        Ok(())
    }

    #[test]
    fn the_introspection_query_is_the_standard_one() {
        let text = IntrospectionQuery::text();
        assert!(text.contains("__schema"));
        assert!(
            IntrospectionQuery::payload().operation_name.as_deref() == Some("IntrospectionQuery")
        );
    }
}
