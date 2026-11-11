//! The two sandboxes: a static AST walk and a runtime interceptor set.
//!
//! Both are best-effort. The original's documentation is explicit that these
//! stop accidents, not attackers, and Heric repeats that rather than implying a
//! guarantee it cannot make. A script that reaches the network through
//! `insomnia.sendRequest` is allowed by design; a script that reaches the
//! process table is not, but a crafted script could still find a way, which is
//! why the script's own mutations are reviewed before they are persisted.

/// A root object a script may not touch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BlockedRoot {
    /// `require`.
    Require,
    /// `process`.
    Process,
    /// `global`.
    Global,
    /// `globalThis`.
    GlobalThis,
    /// `window`.
    Window,
    /// `document`.
    Document,
    /// `eval`.
    Eval,
    /// `Function`, the constructor form of `eval`.
    Function,
    /// `import`.
    Import,
    /// `module`.
    Module,
    /// `exports`.
    Exports,
    /// `__dirname`.
    Dirname,
}

impl BlockedRoot {
    /// The identifier as it appears in source.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Require => "require",
            Self::Process => "process",
            Self::Global => "global",
            Self::GlobalThis => "globalThis",
            Self::Window => "window",
            Self::Document => "document",
            Self::Eval => "eval",
            Self::Function => "Function",
            Self::Import => "import",
            Self::Module => "module",
            Self::Exports => "exports",
            Self::Dirname => "__dirname",
        }
    }

    /// Every blocked root, in the order the settings UI lists them.
    pub const ALL: [Self; 12] = [
        Self::Require,
        Self::Process,
        Self::Global,
        Self::GlobalThis,
        Self::Window,
        Self::Document,
        Self::Eval,
        Self::Function,
        Self::Import,
        Self::Module,
        Self::Exports,
        Self::Dirname,
    ];
}

/// A module a script may `require`.
///
/// The original allows twelve collection modules and eight Node built-ins. The
/// built-ins are read-only views, never the real thing: `path` is a
/// re-implementation and `fs` would defeat the point, so it is absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RequireModule {
    /// `path`.
    Path,
    /// `util`.
    Util,
    /// `crypto`.
    Crypto,
    /// `url`.
    Url,
    /// `querystring`.
    Querystring,
    /// `os`.
    Os,
    /// `buffer`.
    Buffer,
    /// `stream`.
    Stream,
    /// `@kong/insomnia-collection`.
    InsomniaCollection,
    /// `@kong/insomnia-sdk`.
    InsomniaSdk,
    /// `moment`.
    Moment,
    /// `lodash`.
    Lodash,
}

impl RequireModule {
    /// The specifier as written in a script.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Path => "path",
            Self::Util => "util",
            Self::Crypto => "crypto",
            Self::Url => "url",
            Self::Querystring => "querystring",
            Self::Os => "os",
            Self::Buffer => "buffer",
            Self::Stream => "stream",
            Self::InsomniaCollection => "@kong/insomnia-collection",
            Self::InsomniaSdk => "@kong/insomnia-sdk",
            Self::Moment => "moment",
            Self::Lodash => "lodash",
        }
    }

    /// Looks a specifier up.
    #[must_use]
    pub fn parse(specifier: &str) -> Option<Self> {
        [
            Self::Path,
            Self::Util,
            Self::Crypto,
            Self::Url,
            Self::Querystring,
            Self::Os,
            Self::Buffer,
            Self::Stream,
            Self::InsomniaCollection,
            Self::InsomniaSdk,
            Self::Moment,
            Self::Lodash,
        ]
        .into_iter()
        .find(|module| module.as_str() == specifier)
    }
}

/// Something the static sandbox refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// What kind of thing was blocked.
    pub kind: ViolationKind,
    /// Its name, as written.
    pub name: String,
    /// 1-based line, when the walk could place it.
    pub line: Option<u32>,
}

/// The kinds of thing the static sandbox blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViolationKind {
    /// A blocked root identifier.
    Root,
    /// A blocked property on an allowed object.
    Property,
    /// A dynamic `require` whose argument is not a literal.
    DynamicRequire,
    /// An `import` statement.
    Import,
}

impl ViolationKind {
    /// A short name, for the error message.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Root => "identifier",
            Self::Property => "property",
            Self::DynamicRequire => "dynamic require",
            Self::Import => "import",
        }
    }
}

/// The sandbox's configuration.
///
/// Every rule can be turned off individually, because the original exposes a
/// settings screen for exactly that (`SCR-015`). A disabled rule is recorded, not
/// silently ignored: the settings screen shows the reduced protection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecurityPolicy {
    /// Which roots are blocked.
    pub blocked_roots: Vec<BlockedRoot>,
    /// Which modules `require` may load.
    pub allowed_modules: Vec<RequireModule>,
    /// Whether `eval` is intercepted at runtime as well as statically.
    pub intercept_eval: bool,
    /// Whether a dynamic `require` argument is rejected.
    pub require_literal_only: bool,
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self::strict()
    }
}

impl SecurityPolicy {
    /// Every rule on.
    #[must_use]
    pub fn strict() -> Self {
        Self {
            blocked_roots: BlockedRoot::ALL.to_vec(),
            allowed_modules: [
                RequireModule::Path,
                RequireModule::Util,
                RequireModule::Crypto,
                RequireModule::Url,
                RequireModule::Querystring,
                RequireModule::Os,
                RequireModule::Buffer,
                RequireModule::Stream,
                RequireModule::InsomniaCollection,
                RequireModule::InsomniaSdk,
                RequireModule::Moment,
                RequireModule::Lodash,
            ]
            .to_vec(),
            intercept_eval: true,
            require_literal_only: true,
        }
    }

    /// Every rule off.
    ///
    /// The original offers this for a user who trusts their own scripts. The
    /// settings screen warns when it is on.
    #[must_use]
    pub fn permissive() -> Self {
        Self {
            blocked_roots: Vec::new(),
            allowed_modules: RequireModule::parse("path").into_iter().collect(),
            intercept_eval: false,
            require_literal_only: false,
        }
    }

    /// Whether a root is blocked.
    #[must_use]
    pub fn blocks_root(&self, root: BlockedRoot) -> bool {
        self.blocked_roots.contains(&root)
    }

    /// Whether a module may be loaded.
    #[must_use]
    pub fn allows_module(&self, module: RequireModule) -> bool {
        self.allowed_modules.contains(&module)
    }

    /// Walks source text and reports the first violation.
    ///
    /// Deliberately a linear scan over the text rather than a real AST: this
    /// phase only fixes the contract, and a scan that is honest about being one
    /// is better than a partial parser that looks authoritative. The
    /// implementation replaces it with a `swc`-based walk in M2.
    ///
    /// # Errors
    ///
    /// Never fails; a violation is a value, not an error, because the caller
    /// decides whether to show it to the user or to the timeline.
    #[must_use]
    pub fn check_source(&self, source: &str) -> Option<Violation> {
        for (index, line) in source.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") || trimmed.starts_with('*') {
                continue;
            }
            for root in &self.blocked_roots {
                let needle = root.as_str();
                if contains_identifier(line, needle) {
                    return Some(Violation {
                        kind: ViolationKind::Root,
                        name: needle.to_owned(),
                        line: Some(index as u32 + 1),
                    });
                }
            }
        }
        None
    }

    /// Which of the settings a user turned off.
    ///
    /// The settings screen shows this so reduced protection is never invisible.
    #[must_use]
    pub fn disabled_rules(&self) -> Vec<&'static str> {
        let mut disabled = Vec::new();
        for root in BlockedRoot::ALL {
            if !self.blocks_root(root) {
                disabled.push(root.as_str());
            }
        }
        if !self.intercept_eval {
            disabled.push("eval-interception");
        }
        if !self.require_literal_only {
            disabled.push("require-literal-only");
        }
        disabled
    }
}

/// Whether `needle` appears in `haystack` as a whole identifier.
///
/// `process` must not match `preprocess`, and `eval` must not match
/// `evaluate`.
fn contains_identifier(haystack: &str, needle: &str) -> bool {
    let bytes = haystack.as_bytes();
    let mut from = 0;
    while let Some(offset) = haystack[from..].find(needle) {
        let start = from + offset;
        let end = start + needle.len();
        let before_ok = start == 0 || !is_ident_byte(bytes[start - 1]);
        let after_ok = end == bytes.len() || !is_ident_byte(bytes[end]);
        if before_ok && after_ok {
            return true;
        }
        from = end;
    }
    false
}

fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_strict_policy_blocks_every_root() {
        let policy = SecurityPolicy::strict();
        assert_eq!(policy.blocked_roots.len(), 12);
        assert!(policy.disabled_rules().is_empty());
    }

    #[test]
    fn a_whole_identifier_match_is_required() {
        assert!(contains_identifier("process.env.X", "process"));
        assert!(!contains_identifier("preprocess(data)", "process"));
        assert!(!contains_identifier("evaluate(x)", "eval"));
        assert!(contains_identifier("eval(x)", "eval"));
        assert!(!contains_identifier("my_process", "process"));
    }

    #[test]
    fn a_blocked_root_is_reported_with_its_line() -> Result<(), Box<dyn std::error::Error>> {
        let policy = SecurityPolicy::strict();
        let source = "const a = 1;\nconst b = process.env.HOME;\n";
        let violation = policy.check_source(source).ok_or("expected a violation")?;
        assert_eq!(violation.name, "process");
        assert_eq!(violation.line, Some(2));
        Ok(())
    }

    #[test]
    fn a_comment_does_not_trip_the_scan() {
        let policy = SecurityPolicy::strict();
        assert!(
            policy
                .check_source("// process.env is blocked\nconst a = 1;")
                .is_none()
        );
    }

    #[test]
    fn a_permissive_policy_disables_rules_visibly() {
        let policy = SecurityPolicy::permissive();
        assert!(!policy.disabled_rules().is_empty());
        assert!(!policy.blocks_root(BlockedRoot::Process));
        assert!(policy.check_source("process.exit(1)").is_none());
    }

    #[test]
    fn the_module_allowlist_round_trips() -> Result<(), Box<dyn std::error::Error>> {
        for name in ["path", "lodash", "@kong/insomnia-sdk"] {
            let module = RequireModule::parse(name).ok_or("known module")?;
            assert_eq!(module.as_str(), name);
        }
        assert!(RequireModule::parse("fs").is_none());
        Ok(())
    }
}
