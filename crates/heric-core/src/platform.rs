//! Platform detection shared by the GUI, the CLI and the script sandbox.
//!
//! The original exposes `platform`, `isMac`, `isWindows` and `isLinux` as
//! constants read from `process.platform`, defaulting to Linux when no process
//! object is reachable (templating workers). Heric compiles per target, so the
//! same value is known statically; the default only matters for targets this
//! build does not name, which are treated as Linux (PLT-007).

/// The operating system a build targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    /// macOS, `darwin` in the original.
    Mac,
    /// Windows, `win32` in the original.
    Windows,
    /// Linux and every unnamed target, the original's fallback.
    Linux,
}

impl Platform {
    /// The name Heric uses for this platform: `"macos"`, `"windows"` or
    /// `"linux"`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mac => "macos",
            Self::Windows => "windows",
            Self::Linux => "linux",
        }
    }
}

/// The platform this build targets, defaulting to Linux.
#[must_use]
pub fn current() -> Platform {
    if cfg!(target_os = "macos") {
        Platform::Mac
    } else if cfg!(target_os = "windows") {
        Platform::Windows
    } else {
        Platform::Linux
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_platform_has_a_stable_name() {
        assert_eq!(Platform::Mac.as_str(), "macos");
        assert_eq!(Platform::Windows.as_str(), "windows");
        assert_eq!(Platform::Linux.as_str(), "linux");
    }

    #[test]
    fn the_current_platform_matches_the_target() {
        let expected = if cfg!(target_os = "macos") {
            Platform::Mac
        } else if cfg!(target_os = "windows") {
            Platform::Windows
        } else {
            Platform::Linux
        };
        assert_eq!(current(), expected);
    }
}
