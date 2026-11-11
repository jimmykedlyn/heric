//! Why a render is happening.

/// What the rendered text is for.
///
/// The original passes this through every render call because it gates two
/// things: whether confidential values may be revealed, and whether tags that
/// reach outside the renderer are allowed to wait for a person.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderPurpose {
    /// Rendering a request about to be sent.
    Send,
    /// Rendering for the editor's preview.
    Preview,
    /// Rendering from a script.
    Script,
    /// Rendering for an export.
    Export,
    /// Not rendering at all, which is what a request that opts out asks for.
    NoRender,
}

impl RenderPurpose {
    /// Whether `{% prompt %}` may block for an answer.
    ///
    /// Only a send may: a preview that blocks on a dialog turns a hover into a
    /// modal, and the original enforces the same rule.
    #[must_use]
    pub fn allows_blocking_tags(self) -> bool {
        matches!(self, Self::Send)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_send_allows_a_blocking_prompt() {
        assert!(RenderPurpose::Send.allows_blocking_tags());
        for purpose in [
            RenderPurpose::Preview,
            RenderPurpose::Script,
            RenderPurpose::Export,
            RenderPurpose::NoRender,
        ] {
            assert!(!purpose.allows_blocking_tags(), "{purpose:?}");
        }
    }
}
