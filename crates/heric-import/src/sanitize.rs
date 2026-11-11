//! Filename sanitisation for imported titles.

/// Makes a title safe to use as a filename in a Git collection.
///
/// A collection synced to a repository writes one file per document named after
/// its title. `/`, `\` and `..` would escape the directory, a NUL truncates the
/// name on some platforms, and an empty or whitespace-only title produces a file
/// with no name. The original sanitises all of these (`IO-015`) and Heric keeps
/// the same replacements so an exported collection has identical paths.
#[must_use]
pub fn sanitize_filename(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    for character in title.chars() {
        match character {
            '/' | '\\' => out.push('-'),
            '\0' => {}
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    // A title made only of separators collapses to a run of dashes, which is
    // legal but opaque; a title made of nothing at all needs a name.
    let trimmed = out.trim().trim_matches('.').trim();
    let collapsed = collapse_dashes(trimmed);
    if collapsed.is_empty() || collapsed.chars().all(|c| c == '-' || c == '_') {
        "untitled".to_owned()
    } else {
        collapsed
    }
}

/// Normalises a title for display: keeps the characters, drops the control ones.
#[must_use]
pub fn sanitize_title(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .filter(|character| !character.is_control())
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "Untitled".to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn collapse_dashes(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_was_dash = false;
    for character in input.chars() {
        if character == '-' {
            if !last_was_dash {
                out.push('-');
            }
            last_was_dash = true;
        } else {
            out.push(character);
            last_was_dash = false;
        }
    }
    out.trim_matches('-').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separators_become_dashes() {
        assert_eq!(sanitize_filename("a/b\\c"), "a-b-c");
    }

    #[test]
    fn a_traversal_cannot_survive() {
        let name = sanitize_filename("../etc/passwd");
        assert!(!name.contains(".."));
        assert!(!name.contains('/'));
        assert!(!name.contains('\\'));
    }

    #[test]
    fn a_null_byte_is_dropped() {
        assert_eq!(sanitize_filename("a\0b"), "ab");
    }

    #[test]
    fn an_empty_title_gets_a_name() {
        assert_eq!(sanitize_filename(""), "untitled");
        assert_eq!(sanitize_filename("   "), "untitled");
        assert_eq!(sanitize_filename("---"), "untitled");
    }

    #[test]
    fn repeated_dashes_collapse() {
        assert_eq!(sanitize_filename("a//b"), "a-b");
    }

    #[test]
    fn a_normal_title_is_unchanged() {
        assert_eq!(sanitize_filename("Get users"), "Get users");
    }

    #[test]
    fn a_display_title_keeps_its_shape() {
        assert_eq!(sanitize_title("  My collection  "), "My collection");
        assert_eq!(sanitize_title("\u{0}"), "Untitled");
    }
}
