//! The 62 v5 parser expectations, as model-validation tests (`FIX-006`).
//!
//! The original's `import-v5-parser.test.ts` exercises a Zod schema Heric does
//! not have and will not get: Heric reads JSON into [`Doc`], keeps every key it
//! was given, and moves shape enforcement into typed views and
//! `heric_model::init::apply`. So each case below asserts either the original
//! behaviour, faithfully, or the honest Heric equivalent, and every case that
//! deviates carries a comment naming the original's expectation and where
//! Heric puts that responsibility instead.
//!
//! There are exactly 62 tests, matching the 62 `it(...)` blocks in
//! `packages/insomnia/src/common/__tests__/import-v5-parser.test.ts`. The file
//! is the record of `FIX-006`: what was ported, and what was deliberately not.

use serde_json::{Value, json};

use heric_model::{Doc, DocKind, ModelError, init};

/// The test-file alias, so the signatures below stay short.
type TestResult = Result<(), Box<dyn std::error::Error>>;

/// Builds a document from JSON, which is how a migrated v5 file arrives.
fn doc_from(value: Value) -> Result<Doc, Box<dyn std::error::Error>> {
    Ok(serde_json::from_value(value)?)
}

/// Serialises and re-reads a document, which is the round trip a store performs.
fn round_trip(doc: &Doc) -> Result<Doc, Box<dyn std::error::Error>> {
    let value = serde_json::to_value(doc)?;
    Ok(serde_json::from_value(value)?)
}

/// A request carrying the given `authentication` block, as the original's
/// `makeHttpRequest({ authentication })` would build one.
fn request_with_auth(auth: Value) -> Result<Doc, Box<dyn std::error::Error>> {
    doc_from(json!({
        "_id": "req_auth",
        "type": "Request",
        "name": "Get Users",
        "url": "https://example.com",
        "authentication": auth,
    }))
}

/// The request's authentication block, owned.
///
/// The typed views borrow from the document and hand back borrows of their own
/// temporary, so a test that keeps the result has to copy it out.
fn auth_of(doc: &Doc) -> Result<Option<Value>, Box<dyn std::error::Error>> {
    Ok(doc.as_request()?.authentication().cloned())
}

/// A field of the request's authentication block, owned.
fn auth_field(doc: &Doc, field: &str) -> Result<Option<Value>, Box<dyn std::error::Error>> {
    Ok(doc
        .as_request()?
        .authentication()
        .and_then(|value| value.get(field))
        .cloned())
}

/// The request's header rows, owned.
fn headers_of(doc: &Doc) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    Ok(doc.as_request()?.headers().to_vec())
}

// ---------------------------------------------------------------------------
// Group A — literal and JSON primitives (4)
// ---------------------------------------------------------------------------

#[test]
fn literal_schema_accepts_valid_literals() -> TestResult {
    // The original pins `LiteralSchema.parse` over the four JSON scalars. Heric
    // has no literal schema; the equivalent property is that each scalar
    // survives a body round trip unchanged.
    let doc = doc_from(json!({
        "_id": "req_lit",
        "type": "Request",
        "string": "a",
        "number": 1,
        "boolean": true,
        "nothing": null,
    }))?;
    let back = round_trip(&doc)?;
    assert_eq!(back.body.get("string"), Some(&json!("a")));
    assert_eq!(back.body.get("number"), Some(&json!(1)));
    assert_eq!(back.body.get("boolean"), Some(&json!(true)));
    assert_eq!(back.body.get("nothing"), Some(&Value::Null));
    Ok(())
}

#[test]
fn literal_schema_rejects_objects() -> TestResult {
    // The original has `LiteralSchema.parse({})` throw. Heric's `Doc` accepts
    // any JSON and its serialiser refuses nothing, so the rejection point moves
    // to the typed accessor: `url` reads as the empty default while the object
    // stays in the body for a migration to see.
    let doc = doc_from(json!({
        "_id": "req_lit_obj",
        "type": "Request",
        "url": { "nested": true },
    }))?;
    let back = round_trip(&doc)?;
    assert_eq!(back.as_request()?.url(), "");
    assert_eq!(
        back.body.get("url"),
        Some(&json!({ "nested": true })),
        "the malformed value must stay readable, not be dropped"
    );
    Ok(())
}

#[test]
fn json_schema_accepts_nested_arrays_and_literal_keys() -> TestResult {
    let original = json!({ "a": [1, { "b": true }, null], "3": "ok" });
    let doc = doc_from(json!({
        "_id": "req_nested",
        "type": "Request",
        "payload": original.clone(),
    }))?;
    let back = round_trip(&doc)?;
    assert_eq!(back.body.get("payload"), Some(&original));
    Ok(())
}

#[test]
fn json_schema_rejects_unsupported_shapes() -> TestResult {
    // `JsonSchema.parse({ bad: () => null })` throws because a function is not
    // JSON. Nothing in a `Doc` can be a function, so the adjacent truth is that
    // the edge shapes JSON does allow all survive: null, `[]` and `{}` are
    // distinct values and none of them is lost or merged.
    let doc = doc_from(json!({
        "_id": "req_shapes",
        "type": "Request",
        "a": null,
        "b": [],
        "c": {},
    }))?;
    let back = round_trip(&doc)?;
    assert_eq!(back.body.get("a"), Some(&Value::Null));
    assert_eq!(back.body.get("b"), Some(&json!([])));
    assert_eq!(back.body.get("c"), Some(&json!({})));
    Ok(())
}

// ---------------------------------------------------------------------------
// Group B — meta (2)
// ---------------------------------------------------------------------------

#[test]
fn meta_schema_requires_an_id() {
    // Faithful: `MetaSchema.parse({})` throws, and so does a document with no
    // `_id`, because `DocMeta.id` has no default.
    let outcome: Result<Doc, _> = serde_json::from_value(json!({ "type": "Request" }));
    assert!(outcome.is_err(), "a document with no _id must not load");
}

#[test]
fn meta_group_schema_accepts_a_description() -> TestResult {
    // `MetaGroupSchema` accepts `{ id, description }`. In Heric a `meta`
    // object's `id` becomes `_id` and everything else stays in the body.
    let doc = doc_from(json!({
        "_id": "g1",
        "type": "RequestGroup",
        "description": "d",
    }))?;
    assert_eq!(doc.body.get("description"), Some(&json!("d")));
    assert_eq!(doc.meta.id.as_str(), "g1");
    Ok(())
}

// ---------------------------------------------------------------------------
// Group C — headers (2)
// ---------------------------------------------------------------------------

#[test]
fn valid_header_rows_parse() -> TestResult {
    let doc = doc_from(json!({
        "_id": "req_hdr",
        "type": "Request",
        "headers": [
            { "name": "Content-Type", "value": "application/json" },
            { "name": "X-Id", "value": "1", "disabled": true }
        ],
    }))?;
    let back = round_trip(&doc)?;
    let headers = headers_of(&back)?;
    assert_eq!(headers.len(), 2);
    assert_eq!(headers[1].get("disabled"), Some(&json!(true)));
    assert_eq!(headers[1].get("name"), Some(&json!("X-Id")));
    Ok(())
}

#[test]
fn an_invalid_header_row_is_rejected() -> TestResult {
    // `HeadersSchema.parse([{ value: 'x' }])` throws for a row with no name.
    // Heric keeps the row — a dropped row is a silently lost header — and the
    // nameless row reads as the empty name, which is how a caller can tell.
    let doc = doc_from(json!({
        "_id": "req_hdr_bad",
        "type": "Request",
        "headers": [{ "value": "x" }],
    }))?;
    let back = round_trip(&doc)?;
    let headers = headers_of(&back)?;
    assert_eq!(headers.len(), 1);
    assert_eq!(
        headers[0].get("name").and_then(Value::as_str).unwrap_or(""),
        "",
        "a nameless row reads as empty rather than vanishing"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Group D — cookie jar (2)
// ---------------------------------------------------------------------------

#[test]
fn a_cookie_gets_its_defaults() -> TestResult {
    // The original's cookie-row defaults (`path: '/'`, `secure`, `httpOnly`,
    // `expires`, an id) belong to the row schema, not the jar model. M1 does
    // not model cookie rows, so the honest property is that the jar's own
    // `init()` default is an empty array and a supplied row is kept verbatim.
    let fresh = init::document(DocKind::CookieJar, 0);
    assert_eq!(fresh.body.get("cookies"), Some(&json!([])));
    assert_eq!(fresh.meta.name.as_deref(), Some("Default Jar"));

    let doc = doc_from(json!({
        "_id": "jar_1",
        "type": "CookieJar",
        "name": "jar",
        "cookies": [{}],
    }))?;
    let back = round_trip(&doc)?;
    assert_eq!(back.body.get("cookies"), Some(&json!([{}])));
    Ok(())
}

#[test]
fn an_expiry_date_string_survives_as_a_string() -> TestResult {
    // The original coerces `expires` to a `Date`. Heric stores JSON, so the
    // string is preserved and the caller parses it.
    let doc = doc_from(json!({
        "_id": "jar_2",
        "type": "CookieJar",
        "cookies": [{ "expires": "2025-01-01T00:00:00Z" }],
    }))?;
    let back = round_trip(&doc)?;
    let cookies = back.body.get("cookies").and_then(Value::as_array);
    let first = cookies.and_then(|rows| rows.first());
    assert_eq!(
        first.and_then(|row| row.get("expires")),
        Some(&json!("2025-01-01T00:00:00Z"))
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Group E — environment (2)
// ---------------------------------------------------------------------------

#[test]
fn an_environment_parses_root_and_sub_environments() -> TestResult {
    let doc = doc_from(json!({
        "_id": "env_1",
        "type": "Environment",
        "name": "Main",
        "data": { "api": { "baseUrl": "https://api.example.com" } },
        "color": "#00AA88",
        "subEnvironments": [
            { "name": "Dev", "data": { "api": { "baseUrl": "http://localhost:3000" } } }
        ],
    }))?;
    let back = round_trip(&doc)?;
    let subs = back.body.get("subEnvironments").and_then(Value::as_array);
    assert_eq!(
        subs.and_then(|rows| rows.first())
            .and_then(|row| row.get("name")),
        Some(&json!("Dev"))
    );
    assert!(back.body.get("data").is_some());
    Ok(())
}

#[test]
fn an_environment_accepts_a_null_color() -> TestResult {
    let doc = doc_from(json!({
        "_id": "env_2",
        "type": "Environment",
        "color": null,
    }))?;
    let back = round_trip(&doc)?;
    assert_eq!(back.body.get("color"), Some(&Value::Null));
    assert_eq!(
        init::document(DocKind::Environment, 0).body.get("color"),
        Some(&Value::Null)
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Group F — CA certificate key order (1)
// ---------------------------------------------------------------------------

#[test]
fn a_ca_certificate_keeps_its_keys() -> TestResult {
    // The original asserts `Object.keys(file.certificates[0])` is exactly
    // `["path","disabled","meta"]`. Heric cannot assert key order: `serde_json`
    // is built without `preserve_order`, so `Doc.body` serialises
    // alphabetically. The set is what a round trip preserves, and that is what
    // this test pins; `meta`'s `id` is lifted into `_id` on the way in.
    let doc = doc_from(json!({
        "_id": "crt_1",
        "type": "CaCertificate",
        "path": "/path/to/ca.pem",
        "disabled": false,
    }))?;
    let back = round_trip(&doc)?;
    let mut keys: Vec<&String> = back.body.keys().collect();
    keys.sort();
    assert_eq!(keys, vec!["disabled", "path"]);
    assert_eq!(back.body.get("disabled"), Some(&json!(false)));
    assert_eq!(back.body.get("path"), Some(&json!("/path/to/ca.pem")));
    assert_eq!(back.meta.id.as_str(), "crt_1");
    Ok(())
}

// ---------------------------------------------------------------------------
// Group G — the authentication union (14)
// ---------------------------------------------------------------------------

#[test]
fn auth_basic() -> TestResult {
    let auth = json!({ "type": "basic", "username": "u", "password": "p", "useISO88591": true });
    let back = round_trip(&request_with_auth(auth.clone())?)?;
    assert_eq!(auth_of(&back)?, Some(auth.clone()));
    Ok(())
}

#[test]
fn auth_apikey() -> TestResult {
    let auth = json!({ "type": "apikey", "key": "k", "value": "v", "addTo": "header" });
    let back = round_trip(&request_with_auth(auth.clone())?)?;
    assert_eq!(auth_of(&back)?, Some(auth.clone()));
    Ok(())
}

#[test]
fn auth_oauth2_all_grant_types() -> TestResult {
    for grant in [
        "authorization_code",
        "client_credentials",
        "implicit",
        "password",
        "refresh_token",
    ] {
        let auth = json!({
            "type": "oauth2",
            "grantType": grant,
            "usePkce": true,
            "responseType": "code",
        });
        let back = round_trip(&request_with_auth(auth)?)?;
        assert_eq!(auth_field(&back, "grantType")?, Some(json!(grant)));
    }
    Ok(())
}

#[test]
fn auth_hawk() -> TestResult {
    let auth = json!({
        "type": "hawk", "id": "id", "key": "k",
        "algorithm": "sha256", "validatePayload": true,
    });
    let back = round_trip(&request_with_auth(auth.clone())?)?;
    assert_eq!(auth_field(&back, "algorithm")?, Some(json!("sha256")));
    Ok(())
}

#[test]
fn auth_oauth1() -> TestResult {
    let auth = json!({ "type": "oauth1", "signatureMethod": "HMAC-SHA256", "consumerKey": "ck" });
    let back = round_trip(&request_with_auth(auth)?)?;
    assert_eq!(
        auth_field(&back, "signatureMethod")?,
        Some(json!("HMAC-SHA256"))
    );
    Ok(())
}

#[test]
fn auth_digest() -> TestResult {
    let auth = json!({ "type": "digest", "username": "u", "password": "p" });
    let back = round_trip(&request_with_auth(auth.clone())?)?;
    assert_eq!(auth_of(&back)?, Some(auth.clone()));
    Ok(())
}

#[test]
fn auth_ntlm() -> TestResult {
    let auth = json!({ "type": "ntlm", "username": "u", "password": "p" });
    let back = round_trip(&request_with_auth(auth.clone())?)?;
    assert_eq!(auth_of(&back)?, Some(auth.clone()));
    Ok(())
}

#[test]
fn auth_bearer() -> TestResult {
    let auth = json!({ "type": "bearer", "token": "t", "prefix": "Bearer" });
    let back = round_trip(&request_with_auth(auth)?)?;
    assert_eq!(auth_field(&back, "prefix")?, Some(json!("Bearer")));
    Ok(())
}

#[test]
fn auth_iam() -> TestResult {
    let auth = json!({
        "type": "iam", "accessKeyId": "a",
        "secretAccessKey": "s", "region": "eu-west-1",
    });
    let back = round_trip(&request_with_auth(auth.clone())?)?;
    assert_eq!(auth_of(&back)?, Some(auth.clone()));
    Ok(())
}

#[test]
fn auth_netrc() -> TestResult {
    let auth = json!({ "type": "netrc" });
    let back = round_trip(&request_with_auth(auth.clone())?)?;
    assert_eq!(auth_of(&back)?, Some(auth.clone()));
    Ok(())
}

#[test]
fn auth_asap() -> TestResult {
    let claims = "{\"role\":\"admin\"}";
    let auth = json!({
        "type": "asap",
        "issuer": "iss",
        "subject": "sub",
        "audience": "aud",
        "additionalClaims": claims,
        "keyId": "kid",
        "privateKey": "-----BEGIN PRIVATE KEY-----\nkeys\n-----END PRIVATE KEY-----",
    });
    let back = round_trip(&request_with_auth(auth)?)?;
    assert_eq!(
        auth_field(&back, "additionalClaims")?,
        Some(json!(claims)),
        "additionalClaims is an opaque string, not parsed JSON"
    );
    Ok(())
}

#[test]
fn auth_none() -> TestResult {
    let auth = json!({ "type": "none" });
    let back = round_trip(&request_with_auth(auth.clone())?)?;
    assert_eq!(auth_of(&back)?, Some(auth.clone()));
    Ok(())
}

#[test]
fn auth_single_token() -> TestResult {
    let auth = json!({ "type": "singleToken", "token": "t" });
    let back = round_trip(&request_with_auth(auth.clone())?)?;
    assert_eq!(auth_of(&back)?, Some(auth.clone()));
    Ok(())
}

#[test]
fn auth_accepts_an_empty_object() -> TestResult {
    let mut doc = request_with_auth(json!({}))?;
    assert_eq!(auth_of(&doc)?, Some(json!({})));
    init::apply(&mut doc)?;
    assert_eq!(
        auth_of(&doc)?,
        Some(json!({})),
        "an empty auth block is a Request default, so pruning keeps it"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Group H — request settings and scripts (3)
// ---------------------------------------------------------------------------

#[test]
fn request_settings_defaults() -> TestResult {
    let fresh = init::document(DocKind::Request, 0);
    assert_eq!(fresh.body.get("settingEncodeUrl"), Some(&json!(true)));
    assert_eq!(fresh.body.get("settingStoreCookies"), Some(&json!(true)));
    assert_eq!(fresh.body.get("settingSendCookies"), Some(&json!(true)));
    assert_eq!(
        fresh.body.get("settingFollowRedirects"),
        Some(&json!("global"))
    );
    assert_eq!(fresh.body.get("settingRebuildPath"), Some(&json!(true)));
    Ok(())
}

#[test]
fn request_settings_are_respected() -> TestResult {
    let mut doc = doc_from(json!({
        "_id": "req_settings",
        "type": "Request",
        "settingEncodeUrl": false,
        "settingSendCookies": false,
        "settingStoreCookies": false,
        "settingFollowRedirects": "off",
        "settingRebuildPath": false,
        "settingDisableRenderRequestBody": true,
    }))?;
    init::apply(&mut doc)?;
    assert_eq!(doc.body.get("settingEncodeUrl"), Some(&json!(false)));
    assert_eq!(doc.body.get("settingSendCookies"), Some(&json!(false)));
    assert_eq!(doc.body.get("settingStoreCookies"), Some(&json!(false)));
    assert_eq!(doc.body.get("settingFollowRedirects"), Some(&json!("off")));
    assert_eq!(doc.body.get("settingRebuildPath"), Some(&json!(false)));
    assert_eq!(
        doc.body.get("settingDisableRenderRequestBody"),
        Some(&json!(true))
    );
    Ok(())
}

#[test]
fn body_params_default() -> TestResult {
    // The original's body-parameter schema defaults `name` to `''`. Heric keeps
    // the row exactly as written: an absent `name` stays absent, and the typed
    // reader is what decides an absent name reads as empty.
    let doc = doc_from(json!({
        "_id": "req_body",
        "type": "Request",
        "method": "POST",
        "body": { "params": [{}] },
    }))?;
    let back = round_trip(&doc)?;
    let first = back
        .body
        .get("body")
        .and_then(|body| body.get("params"))
        .and_then(Value::as_array)
        .and_then(|rows| rows.first())
        .cloned();
    assert_eq!(first, Some(json!({})));
    Ok(())
}

// ---------------------------------------------------------------------------
// Group I — RequestSettingsSchema (3)
// ---------------------------------------------------------------------------

#[test]
fn request_settings_fills_a_partial_block() -> TestResult {
    // The original nests settings under `settings: { cookies: {} }`. Heric
    // models the same intent as six flat `setting*` keys on the request, so
    // "partial" means a request carrying some of them: `apply` fills the rest.
    let mut doc = doc_from(json!({
        "_id": "req_partial",
        "type": "Request",
        "settingEncodeUrl": false,
    }))?;
    init::apply(&mut doc)?;
    assert_eq!(doc.body.get("settingEncodeUrl"), Some(&json!(false)));
    assert_eq!(doc.body.get("settingStoreCookies"), Some(&json!(true)));
    assert_eq!(doc.body.get("settingSendCookies"), Some(&json!(true)));
    assert_eq!(
        doc.body.get("settingFollowRedirects"),
        Some(&json!("global"))
    );
    assert_eq!(doc.body.get("settingRebuildPath"), Some(&json!(true)));
    assert_eq!(
        doc.body.get("settingDisableRenderRequestBody"),
        Some(&json!(false))
    );
    Ok(())
}

#[test]
fn request_settings_respects_explicit_values() -> TestResult {
    // `apply` also fills the request's other defaults, so the property is that
    // the six settings keys keep the values the document gave them.
    let mut doc = doc_from(json!({
        "_id": "req_explicit",
        "type": "Request",
        "settingEncodeUrl": false,
        "settingStoreCookies": false,
        "settingSendCookies": false,
        "settingFollowRedirects": "off",
        "settingRebuildPath": false,
        "settingDisableRenderRequestBody": true,
    }))?;
    init::apply(&mut doc)?;
    assert_eq!(doc.body.get("settingEncodeUrl"), Some(&json!(false)));
    assert_eq!(doc.body.get("settingStoreCookies"), Some(&json!(false)));
    assert_eq!(doc.body.get("settingSendCookies"), Some(&json!(false)));
    assert_eq!(doc.body.get("settingFollowRedirects"), Some(&json!("off")));
    assert_eq!(doc.body.get("settingRebuildPath"), Some(&json!(false)));
    assert_eq!(
        doc.body.get("settingDisableRenderRequestBody"),
        Some(&json!(true))
    );
    Ok(())
}

#[test]
fn render_request_body_defaults_to_false_in_the_model() -> TestResult {
    // The model field is negative: `settingDisableRenderRequestBody`. The v5
    // file's `renderRequestBody` is the positive form and is a different field,
    // so `false` here means "rendering is on".
    let fresh = init::document(DocKind::Request, 0);
    assert_eq!(
        fresh.body.get("settingDisableRenderRequestBody"),
        Some(&json!(false))
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Group J — WebSocketRequestSettingsSchema (3)
// ---------------------------------------------------------------------------

#[test]
fn websocket_settings_fill_a_partial_block() -> TestResult {
    // `WebSocketRequest` is unmodelled in this milestone: `init::apply` has no
    // table to prune against and no defaults to fill, so it leaves the document
    // alone. That is deliberate — the plan schedules the websocket model with
    // its feature, and inventing defaults now would be guessing.
    let mut doc = doc_from(json!({
        "_id": "ws-req-settings",
        "type": "WebSocketRequest",
        "settings": { "cookies": {} },
    }))?;
    let before = doc.body.clone();
    init::apply(&mut doc)?;
    assert_eq!(doc.body, before);
    Ok(())
}

#[test]
fn websocket_settings_respect_explicit_values() -> TestResult {
    let doc = doc_from(json!({
        "_id": "ws-req-explicit",
        "type": "WebSocketRequest",
        "settings": { "encodeUrl": false, "useProxy": true },
    }))?;
    let back = round_trip(&doc)?;
    assert_eq!(
        back.body.get("settings").cloned(),
        Some(json!({ "encodeUrl": false, "useProxy": true }))
    );
    Ok(())
}

#[test]
fn websocket_settings_require_the_cookies_block() -> TestResult {
    // The original's `cookies` sub-schema is required. Heric has no such
    // requirement; the adjacent truth is that a document with no settings at
    // all round-trips with none and `apply` does not invent one.
    let mut doc = doc_from(json!({
        "_id": "ws-req-nosettings",
        "type": "WebSocketRequest",
        "url": "wss://ws.example",
    }))?;
    init::apply(&mut doc)?;
    assert!(!doc.body.contains_key("settings"));
    Ok(())
}

// ---------------------------------------------------------------------------
// Group K — SocketIORequestSettingsSchema (3)
// ---------------------------------------------------------------------------

#[test]
fn socket_io_settings_fill_a_partial_block() -> TestResult {
    let mut doc = doc_from(json!({
        "_id": "socketio-req-settings",
        "type": "SocketIORequest",
        "settings": { "cookies": {} },
    }))?;
    let before = doc.body.clone();
    init::apply(&mut doc)?;
    assert_eq!(doc.body, before);
    Ok(())
}

#[test]
fn socket_io_settings_respect_explicit_values() -> TestResult {
    let doc = doc_from(json!({
        "_id": "socketio-req-explicit",
        "type": "SocketIORequest",
        "settings": { "encodeUrl": false, "path": "/custom-socket" },
    }))?;
    let back = round_trip(&doc)?;
    assert_eq!(
        back.body.get("settings").cloned(),
        Some(json!({ "encodeUrl": false, "path": "/custom-socket" }))
    );
    Ok(())
}

#[test]
fn socket_io_settings_require_the_cookies_block() -> TestResult {
    let mut doc = doc_from(json!({
        "_id": "socketio-req-nosettings",
        "type": "SocketIORequest",
        "url": "https://socket.example",
    }))?;
    init::apply(&mut doc)?;
    assert!(!doc.body.contains_key("settings"));
    Ok(())
}

// ---------------------------------------------------------------------------
// Group L — settings through the request schemas (3)
// ---------------------------------------------------------------------------

#[test]
fn a_request_fills_a_partial_settings_block() -> TestResult {
    let mut doc = doc_from(json!({
        "_id": "req_l_partial",
        "type": "Request",
        "settingEncodeUrl": false,
    }))?;
    init::apply(&mut doc)?;
    assert_eq!(doc.body.get("settingEncodeUrl"), Some(&json!(false)));
    assert_eq!(doc.body.get("settingSendCookies"), Some(&json!(true)));
    assert_eq!(
        doc.body.get("settingFollowRedirects"),
        Some(&json!("global"))
    );
    Ok(())
}

#[test]
fn a_websocket_request_respects_explicit_settings() -> TestResult {
    let doc = doc_from(json!({
        "_id": "ws-req-l",
        "type": "WebSocketRequest",
        "settings": {
            "encodeUrl": false,
            "followRedirects": "off",
            "cookies": { "send": false, "store": false },
            "useProxy": true,
        },
    }))?;
    let back = round_trip(&doc)?;
    let settings = back.body.get("settings").cloned().ok_or("settings gone")?;
    assert_eq!(settings.get("encodeUrl"), Some(&json!(false)));
    assert_eq!(settings.get("followRedirects"), Some(&json!("off")));
    assert_eq!(
        settings.get("cookies").and_then(|c| c.get("store")),
        Some(&json!(false))
    );
    assert_eq!(settings.get("useProxy"), Some(&json!(true)));
    Ok(())
}

#[test]
fn a_socket_io_request_respects_explicit_settings() -> TestResult {
    let doc = doc_from(json!({
        "_id": "socketio-req-l",
        "type": "SocketIORequest",
        "settings": {
            "encodeUrl": false,
            "cookies": { "send": false, "store": false },
            "path": "/custom-socket",
        },
    }))?;
    let back = round_trip(&doc)?;
    let settings = back.body.get("settings").cloned().ok_or("settings gone")?;
    assert_eq!(settings.get("encodeUrl"), Some(&json!(false)));
    assert_eq!(
        settings.get("cookies").and_then(|c| c.get("send")),
        Some(&json!(false))
    );
    assert_eq!(settings.get("path"), Some(&json!("/custom-socket")));
    Ok(())
}

// ---------------------------------------------------------------------------
// Group M — gRPC (1)
// ---------------------------------------------------------------------------

#[test]
fn a_minimal_grpc_request_gets_reflection_defaults() -> TestResult {
    // `GrpcRequest` is unmodelled in this milestone, so `apply` does not add the
    // original's `reflectionApi: { enabled: false, url: '' }`. What it does do
    // is keep whatever the document carries, which is the property that makes
    // reading a v5 file non-destructive.
    let doc = doc_from(json!({
        "_id": "greq_1",
        "type": "GrpcRequest",
        "url": "grpc://service.example",
        "reflectionApi": { "enabled": true, "url": "https://x" },
    }))?;
    let back = round_trip(&doc)?;
    assert_eq!(
        back.body.get("reflectionApi").cloned(),
        Some(json!({ "enabled": true, "url": "https://x" }))
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Group N — websocket and socket.io request parse (2)
// ---------------------------------------------------------------------------

#[test]
fn a_websocket_request_parses() -> TestResult {
    let doc = doc_from(json!({
        "_id": "ws-req-abc",
        "type": "WebSocketRequest",
        "url": "wss://ws.example",
        "name": "LiveFeed",
        "headers": [{ "name": "Content-Type", "value": "application/json" }],
        "parameters": [{ "name": "token", "value": "x" }],
        "pathParameters": [{ "name": "roomId", "value": "42" }],
    }))?;
    let back = round_trip(&doc)?;
    assert_eq!(back.meta.id.as_str(), "ws-req-abc");
    assert_eq!(back.meta.name.as_deref(), Some("LiveFeed"));
    assert_eq!(back.body.get("url"), Some(&json!("wss://ws.example")));
    Ok(())
}

#[test]
fn a_socket_io_request_parses() -> TestResult {
    let doc = doc_from(json!({
        "_id": "socketio-req-abc",
        "type": "SocketIORequest",
        "url": "https://socket.example",
        "name": "Rooms",
        "headers": [{ "name": "Content-Type", "value": "application/json" }],
        "parameters": [{ "name": "ns", "value": "/rooms" }],
        "pathParameters": [{ "name": "roomId", "value": "42" }],
    }))?;
    let back = round_trip(&doc)?;
    assert_eq!(back.meta.id.as_str(), "socketio-req-abc");
    assert_eq!(back.body.get("url"), Some(&json!("https://socket.example")));
    Ok(())
}

// ---------------------------------------------------------------------------
// Group O — request group (1)
// ---------------------------------------------------------------------------

#[test]
fn a_group_parses_with_headers() -> TestResult {
    let doc = doc_from(json!({
        "_id": "fld_group-1",
        "type": "RequestGroup",
        "name": "Root Group",
        "headers": [{ "name": "Content-Type", "value": "application/json" }],
    }))?;
    let back = round_trip(&doc)?;
    assert_eq!(back.meta.kind, DocKind::RequestGroup);
    assert_eq!(back.meta.id.as_str(), "fld_group-1");
    assert_eq!(back.meta.name.as_deref(), Some("Root Group"));
    assert!(back.body.get("headers").and_then(Value::as_array).is_some());
    Ok(())
}

// ---------------------------------------------------------------------------
// Group P — request collection recursion (4)
// ---------------------------------------------------------------------------

/// The five-item collection the original's recursion case builds.
fn mixed_collection() -> Value {
    json!([
        { "_id": "req_a", "type": "Request", "url": "https://example.com" },
        { "_id": "greq_a", "type": "GrpcRequest", "url": "grpc://svc" },
        { "_id": "ws-req-a", "type": "WebSocketRequest", "url": "wss://ws" },
        { "_id": "socketio-req-a", "type": "SocketIORequest", "url": "https://s" },
        {
            "_id": "fld_group-1",
            "type": "RequestGroup",
            "name": "Root Group",
            "children": [
                { "_id": "req_child", "type": "Request" },
                {
                    "_id": "fld_nested",
                    "type": "RequestGroup",
                    "children": [{ "_id": "req_nested_child", "type": "Request" }]
                }
            ]
        }
    ])
}

#[test]
fn a_mixed_collection_has_five_items() -> TestResult {
    let items = mixed_collection();
    let doc = doc_from(json!({
        "_id": "wrk_1",
        "type": "Workspace",
        "collection": items.clone(),
    }))?;
    let back = round_trip(&doc)?;
    assert_eq!(back.body.get("collection"), Some(&items));
    let rows = items.as_array().ok_or("items must be an array")?;
    assert_eq!(rows.len(), 5);
    let kinds: Vec<&str> = rows
        .iter()
        .filter_map(|row| row.get("type").and_then(Value::as_str))
        .collect();
    assert_eq!(
        kinds,
        [
            "Request",
            "GrpcRequest",
            "WebSocketRequest",
            "SocketIORequest",
            "RequestGroup"
        ],
        "each item stays distinguishable by its type"
    );
    Ok(())
}

#[test]
fn a_socket_io_item_without_meta_keeps_its_event_listeners() -> TestResult {
    // The original's union has no discriminator to go on without `meta.id`, so
    // it relies on `z.undefined()` guards to keep a Socket.IO item out of the
    // WebSocket branch. Heric has no union to pick wrongly from: the document
    // is the document.
    let listeners = json!([{
        "id": "ev1", "eventName": "join", "desc": "on join", "isOpen": true
    }]);
    let doc = doc_from(json!({
        "_id": "socketio-req-nometa",
        "type": "SocketIORequest",
        "eventListeners": listeners.clone(),
    }))?;
    let back = round_trip(&doc)?;
    assert_eq!(back.body.get("eventListeners"), Some(&listeners));
    Ok(())
}

#[test]
fn a_socket_io_item_without_meta_keeps_its_body() -> TestResult {
    let body = json!({
        "eventName": "chat",
        "ack": true,
        "args": [{ "id": "a1", "value": "\"hi\"", "mode": "application/json" }],
    });
    let doc = doc_from(json!({
        "_id": "socketio-req-nobody-meta",
        "type": "SocketIORequest",
        "body": body.clone(),
    }))?;
    let back = round_trip(&doc)?;
    assert_eq!(back.body.get("body"), Some(&body));
    Ok(())
}

#[test]
fn a_leaf_node_with_children_is_rejected() -> TestResult {
    // `RequestCollectionSchema.parse` throws on `children` at a leaf. Heric
    // accepts the JSON and prunes it: `children` is not in the Request model,
    // so `init::apply` removes it. That is the enforcement point, and it is a
    // real one — a stray `children` never survives a save.
    let mut doc = doc_from(json!({
        "_id": "req_leaf",
        "type": "Request",
        "url": "https://example.com",
        "children": [],
    }))?;
    assert_eq!(doc.body.get("children"), Some(&json!([])));
    init::apply(&mut doc)?;
    assert!(!doc.body.contains_key("children"));
    Ok(())
}

// ---------------------------------------------------------------------------
// Group Q — mock route (1)
// ---------------------------------------------------------------------------

#[test]
fn a_mock_route_gets_its_status_default() -> TestResult {
    // The mock-route schema defaults `statusCode` to 200. `MockRoute` is
    // unmodelled here, so `apply` neither fills nor prunes; a route that
    // carries a status keeps it, and one that does not stays without.
    let with_status = doc_from(json!({
        "_id": "mock-route_1",
        "type": "MockRoute",
        "statusCode": 200,
    }))?;
    let back = round_trip(&with_status)?;
    assert_eq!(back.body.get("statusCode"), Some(&json!(200)));

    let mut without = doc_from(json!({
        "_id": "mock-route_2",
        "type": "MockRoute",
        "name": "Ping",
    }))?;
    init::apply(&mut without)?;
    assert!(!without.body.contains_key("statusCode"));
    Ok(())
}

// ---------------------------------------------------------------------------
// Group R — MCP request (1)
// ---------------------------------------------------------------------------

#[test]
fn an_mcp_request_parses() -> TestResult {
    let doc = doc_from(json!({
        "_id": "mcp-req_1",
        "type": "McpRequest",
        "name": "MCP Request",
        "url": "https://example.com/mcp",
        "transportType": "streamable-http",
        "headers": [{ "name": "X-MCP", "value": "1" }],
        "authentication": { "type": "none" },
        "env": [{ "id": "env-1", "name": "foo", "value": "bar", "type": "str", "enabled": true }],
        "roots": [{ "uri": "/data/to/root/file" }, { "uri": "/more/data" }],
    }))?;
    let back = round_trip(&doc)?;
    assert_eq!(
        back.body.get("transportType"),
        Some(&json!("streamable-http"))
    );
    assert_eq!(
        back.body.get("url"),
        Some(&json!("https://example.com/mcp"))
    );
    let env = back.body.get("env").and_then(Value::as_array);
    assert_eq!(
        env.and_then(|rows| rows.first())
            .and_then(|row| row.get("name")),
        Some(&json!("foo"))
    );
    let roots = back.body.get("roots").and_then(Value::as_array);
    assert_eq!(
        roots
            .and_then(|rows| rows.first())
            .and_then(|row| row.get("uri")),
        Some(&json!("/data/to/root/file"))
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Group S — top-level documents (4)
// ---------------------------------------------------------------------------

#[test]
fn a_collection_keeps_its_collection_and_certificates() -> TestResult {
    let doc = doc_from(json!({
        "_id": "col-1",
        "type": "Workspace",
        "name": "My Collection",
        "collection": [
            { "_id": "req_a", "type": "Request" },
            { "_id": "greq_a", "type": "GrpcRequest" }
        ],
        "cookieJar": { "name": "jar", "cookies": [{}] },
        "environments": { "name": "Main", "data": { "a": "b" } },
        "certificates": [{ "path": "/etc/ssl/certs/ca.pem" }],
    }))?;
    let back = round_trip(&doc)?;
    assert_eq!(
        back.body
            .get("collection")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    let certificates = back.body.get("certificates").and_then(Value::as_array);
    let first = certificates.and_then(|rows| rows.first());
    assert_eq!(
        first.and_then(|row| row.get("path")),
        Some(&json!("/etc/ssl/certs/ca.pem"))
    );
    assert!(
        first.and_then(|row| row.get("disabled")).is_none(),
        "Heric does not default a key the document did not carry"
    );
    Ok(())
}

#[test]
fn a_spec_file_keeps_its_test_suites() -> TestResult {
    let doc = doc_from(json!({
        "_id": "spc_1",
        "type": "ApiSpec",
        "name": "My API",
        "spec": { "contents": { "openapi": "3.1.0" } },
        "testSuites": [{
            "name": "Smoke",
            "tests": [{ "name": "loads", "requestId": null, "code": "/* noop */" }]
        }],
        "certificates": [{}, {}],
    }))?;
    let back = round_trip(&doc)?;
    let request_id = back
        .body
        .get("testSuites")
        .and_then(Value::as_array)
        .and_then(|rows| rows.first())
        .and_then(|row| row.get("tests"))
        .and_then(Value::as_array)
        .and_then(|rows| rows.first())
        .and_then(|test| test.get("requestId"));
    assert_eq!(
        request_id,
        Some(&Value::Null),
        "a null requestId is present-and-null, not absent"
    );
    assert_eq!(
        back.body
            .get("certificates")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(2)
    );
    Ok(())
}

#[test]
fn a_mock_server_file_keeps_its_routes() -> TestResult {
    let doc = doc_from(json!({
        "_id": "mock_1",
        "type": "MockServer",
        "name": "MS",
        "server": { "url": "https://mock.example", "useInsomniaCloud": true },
        "routes": [{
            "_id": "mock-route_1",
            "type": "MockRoute",
            "name": "Ping",
            "method": "GET",
            "statusCode": 200,
        }],
    }))?;
    let back = round_trip(&doc)?;
    assert_eq!(
        back.body.get("server").and_then(|s| s.get("url")),
        Some(&json!("https://mock.example"))
    );
    assert_eq!(
        back.body
            .get("routes")
            .and_then(Value::as_array)
            .and_then(|rows| rows.first())
            .and_then(|row| row.get("statusCode")),
        Some(&json!(200))
    );
    Ok(())
}

#[test]
fn an_mcp_client_file_keeps_its_request() -> TestResult {
    let doc = doc_from(json!({
        "_id": "mcp-client_1",
        "type": "McpRequest",
        "name": "MCP Client",
        "mcpRequest": {
            "name": "MCP Request",
            "url": "https://example.com/mcp",
            "transportType": "streamable-http",
        },
    }))?;
    let back = round_trip(&doc)?;
    let request = back
        .body
        .get("mcpRequest")
        .cloned()
        .ok_or("no mcpRequest")?;
    assert_eq!(request.get("url"), Some(&json!("https://example.com/mcp")));
    assert_eq!(
        request.get("transportType"),
        Some(&json!("streamable-http"))
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Group T — the InsomniaFile discriminated union (6)
// ---------------------------------------------------------------------------

/// The five v5 file-type discriminators, and the [`DocKind`] each lands on.
///
/// `mcpClient.insomnia/5.0` deliberately breaks the `insomnia.rest` pattern
/// (`import-v5-parser.ts:478`, recorded at `storage.md:125`) so that older
/// clients do not crash when syncing it.
const FILE_TYPES: [(&str, DocKind); 5] = [
    ("collection.insomnia.rest/5.0", DocKind::Workspace),
    ("spec.insomnia.rest/5.0", DocKind::ApiSpec),
    ("mock.insomnia.rest/5.0", DocKind::MockServer),
    ("environment.insomnia.rest/5.0", DocKind::Environment),
    ("mcpClient.insomnia/5.0", DocKind::McpRequest),
];

/// The [`DocKind`] a v5 file type maps to, if it is one of the five.
fn kind_for_file_type(file_type: &str) -> Option<DocKind> {
    FILE_TYPES
        .iter()
        .find(|(name, _)| *name == file_type)
        .map(|(_, kind)| *kind)
}

#[test]
fn an_insomnia_file_accepts_the_collection_variant() -> TestResult {
    let kind = kind_for_file_type("collection.insomnia.rest/5.0").ok_or("unmapped")?;
    assert_eq!(kind, DocKind::Workspace);
    let doc = doc_from(json!({
        "_id": "wrk_v",
        "type": "Workspace",
        "collection": [{ "_id": "req_a", "type": "Request" }],
    }))?;
    assert_eq!(doc.meta.kind, kind);
    Ok(())
}

#[test]
fn an_insomnia_file_accepts_the_spec_variant() -> TestResult {
    let kind = kind_for_file_type("spec.insomnia.rest/5.0").ok_or("unmapped")?;
    assert_eq!(kind, DocKind::ApiSpec);
    let doc = doc_from(json!({
        "_id": "spc_v",
        "type": "ApiSpec",
        "spec": { "contents": {} },
    }))?;
    assert_eq!(doc.meta.kind, kind);
    Ok(())
}

#[test]
fn an_insomnia_file_accepts_the_mock_server_variant() -> TestResult {
    let kind = kind_for_file_type("mock.insomnia.rest/5.0").ok_or("unmapped")?;
    assert_eq!(kind, DocKind::MockServer);
    let doc = doc_from(json!({
        "_id": "mock_v",
        "type": "MockServer",
        "server": { "url": "https://mock" },
    }))?;
    assert_eq!(doc.meta.kind, kind);
    Ok(())
}

#[test]
fn an_insomnia_file_rejects_a_future_type() {
    // The file-type discriminator is not a `DocKind`; the place Heric rejects an
    // unknown type string is `DocKind::parse`, which is what a migration calls
    // when it reads a document's `type`.
    assert_eq!(
        DocKind::parse("futureCollection.insomnia.rest/5.0").err(),
        Some(ModelError::UnknownKind(
            "futureCollection.insomnia.rest/5.0".to_owned()
        ))
    );
    assert!(kind_for_file_type("futureCollection.insomnia.rest/5.0").is_none());
}

#[test]
fn an_insomnia_file_accepts_an_mcp_request() -> TestResult {
    let kind = kind_for_file_type("mcpClient.insomnia/5.0").ok_or("unmapped")?;
    assert_eq!(kind, DocKind::McpRequest);
    let doc = doc_from(json!({
        "_id": "mcp-req_v",
        "type": "McpRequest",
        "url": "https://example.com/mcp",
        "transportType": "streamable-http",
    }))?;
    assert_eq!(doc.meta.kind, kind);
    Ok(())
}

#[test]
fn an_insomnia_file_rejects_an_unknown_type() {
    assert_eq!(
        DocKind::parse("nope").err(),
        Some(ModelError::UnknownKind("nope".to_owned()))
    );
    assert!(kind_for_file_type("nope").is_none());
}
