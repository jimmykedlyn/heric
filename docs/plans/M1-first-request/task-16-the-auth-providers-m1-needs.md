### Task 16: The auth providers M1 needs

**Files:**
- Create: `crates/heric-auth/src/providers/{mod.rs,basic.rs,bearer.rs,apikey.rs}`
- Modify: `crates/heric-auth/src/lib.rs`
- Test: `#[cfg(test)]` in `providers/mod.rs`

**Interfaces:**
- Produces: `heric_auth::providers() -> Result<ProviderRegistry, AuthError>` registering exactly `BasicProvider`, `BearerProvider`, `ApiKeyProvider`.

**Scope, decided here:** three providers. `ProviderRegistry::apply` already answers `AuthError::Unsupported` for an unregistered method, which is the honest result for Digest, NTLM, OAuth and IAM until M3, and `heric_auth::detect` already feeds the migration report so a user learns at import time rather than at send time.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn basic_adds_an_authorization_header_with_base64_credentials() -> Result<(), AuthError> {
    // user "kyle" / pass "secret" -> "Basic a3lsZTpzZWNyZXQ="
    Ok(())
}

#[test]
fn a_secret_pending_reentry_is_an_error_not_an_empty_value() {
    // SecretState::PendingReEntry -> Err(AuthError::PendingReEntry(_)), never Ok((""))
}

#[test]
fn bearer_uses_the_configured_prefix() { /* default "Bearer", or "Token" */ }
#[test]
fn an_api_key_lands_in_a_header_a_query_parameter_or_a_cookie() { /* ApiKeyLocation x3 */ }
#[test]
fn an_unregistered_method_is_unsupported() { /* registry.get("oauth2") is None */ }
```

- [ ] **Step 2: Run to verify failure.**

- [ ] **Step 3: Implement** `AuthProvider` for each: read the method's fields from `AuthMethod`, resolve credentials through `AuthContext::secrets` so `SecretState::PendingReEntry` becomes `AuthError::PendingReEntry` (Review Focus 4's pin) and `Absent` becomes `AuthError::Missing`, and return an `AuthOutcome` carrying headers, query pairs or cookies. Base64 is a hand-written eleven-line encoder rather than a new dependency.

- [ ] **Step 4: Run to verify passing.**

- [ ] **Step 5: Commit** — `feat(auth): add the basic, bearer and API-key providers`

