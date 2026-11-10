//! Nunjucks-compatible templating for Heric.
//!
//! Reimplements the subset of Nunjucks that Insomnia uses (variables,
//! filters, template tags such as `{% response %}`, `{% uuid %}`,
//! `{% timestamp %}` and custom tag registration) with golden tests built
//! from the original TypeScript test table. See `docs/inventory/templating.md`
//! once the inventory lands.

/// Error type for template rendering.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// A variable or tag referenced something unavailable in the context.
    #[error("unknown variable: {0}")]
    UnknownVariable(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_message_is_readable() {
        let err = RenderError::UnknownVariable("base_url".to_string());
        assert_eq!(err.to_string(), "unknown variable: base_url");
    }
}
