//! Console capture.

/// A console level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConsoleLevel {
    /// `console.log`.
    Log,
    /// `console.info`.
    Info,
    /// `console.warn`.
    Warn,
    /// `console.error`.
    Error,
    /// `console.debug`.
    Debug,
}

impl ConsoleLevel {
    /// The method name as it appears in source.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Log => "log",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
            Self::Debug => "debug",
        }
    }
}

/// One captured line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsoleEntry {
    /// Which method was called.
    pub level: ConsoleLevel,
    /// The arguments, already stringified the way the original does it: objects
    /// are JSON, strings are themselves, everything else uses `String()`.
    pub text: String,
    /// When it was logged, as an offset from the script's start.
    pub at: std::time::Duration,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_keep_their_names() {
        assert_eq!(ConsoleLevel::Warn.as_str(), "warn");
        let entry = ConsoleEntry {
            level: ConsoleLevel::Warn,
            text: "careful".to_owned(),
            at: std::time::Duration::from_millis(2),
        };
        assert_eq!(entry.level.as_str(), "warn");
    }
}
