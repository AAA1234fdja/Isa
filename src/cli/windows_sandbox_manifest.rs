//! Staged, pure lexical screening for a proposed Windows sandbox import.
//!
//! Not wired into execution or filesystem operations. This is a conservative
//! first filter, not containment: Windows aliases, filesystem identity, races,
//! reparse points, hard links and alternate streams require handle-based checks.
use std::collections::HashSet;

const MAX_ENTRIES: usize = 4096;
const MAX_PATH_BYTES: usize = 4096;
const MAX_COMPONENT_BYTES: usize = 255;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ManifestError {
    TooManyEntries,
    InvalidPath,
    DuplicatePath,
}

/// Validate untrusted relative UTF-8 paths before any attempted import. This
/// intentionally rejects some otherwise valid Windows names. No normalization
/// here can establish Windows filesystem identity or authorization.
pub(crate) fn screen_paths<'a>(paths: impl IntoIterator<Item = &'a str>) -> Result<(), ManifestError> {
    let mut seen = HashSet::new();
    for (index, path) in paths.into_iter().enumerate() {
        if index >= MAX_ENTRIES { return Err(ManifestError::TooManyEntries); }
        if path.is_empty() || path.len() > MAX_PATH_BYTES { return Err(ManifestError::InvalidPath); }
        // Accept only forward separators for a single canonical wire format.
        for part in path.split('/') {
            if part.is_empty() || part.len() > MAX_COMPONENT_BYTES || part == "." || part == ".."
                || part.ends_with('.') || part.ends_with(' ')
                || part.chars().any(|c| c.is_control() || matches!(c, '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '~'))
            { return Err(ManifestError::InvalidPath); }
            if part.eq_ignore_ascii_case(".isa") || part.eq_ignore_ascii_case(".git") {
                return Err(ManifestError::InvalidPath);
            }
            // Windows device names remain reserved even with an extension.
            // Reject spaces preceding an extension on device-like names too;
            // path normalization differs across Win32 namespaces/filesystems.
            let stem = part.split('.').next().unwrap_or("").trim_end_matches(' ');
            if ["CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$"].iter().any(|name| stem.eq_ignore_ascii_case(name))
                || is_numbered_device(stem)
            { return Err(ManifestError::InvalidPath); }
        }
        // ASCII duplicates are rejected here. Non-ASCII filesystem aliases
        // still require actual filesystem-aware identity checking at apply.
        if !seen.insert(path.to_ascii_lowercase()) { return Err(ManifestError::DuplicatePath); }
    }
    Ok(())
}

fn is_numbered_device(stem: &str) -> bool {
    let Some(prefix) = stem.get(..3) else { return false; };
    if !prefix.eq_ignore_ascii_case("COM") && !prefix.eq_ignore_ascii_case("LPT") { return false; }
    matches!(&stem[3..], "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_ordinary_relative_paths() {
        assert_eq!(screen_paths(["src/main.rs", "README.md", "data/ñ.txt"]), Ok(()));
    }

    #[test]
    fn rejects_dangerous_or_ambiguous_paths() {
        for path in ["", "/a", "a/", "a//b", ".", "../a", "a/../b", "a\\b", "C:/x", "a:stream", "a. ", "a.",
            ".isa/key", "a/.ISA/x", ".git/config", "ISA~1/x", "CON", "nul.txt", "NUL .txt", "com1.log", "LPT9", "COM¹", "lpt².txt", "COM³.log", "a/CONOUT$.txt", "a\n"] {
            assert_eq!(screen_paths([path]), Err(ManifestError::InvalidPath), "{path:?}");
        }
    }

    #[test]
    fn rejects_ascii_duplicate_paths_and_caps_manifest() {
        assert_eq!(screen_paths(["src/File", "SRC/file"]), Err(ManifestError::DuplicatePath));
        let entries: Vec<String> = (0..=MAX_ENTRIES).map(|i| format!("f{i}")).collect();
        assert_eq!(screen_paths(entries.iter().map(String::as_str)), Err(ManifestError::TooManyEntries));
    }

    #[test]
    fn rejects_excessive_lengths() {
        let component = "x".repeat(MAX_COMPONENT_BYTES + 1);
        assert_eq!(screen_paths([component.as_str()]), Err(ManifestError::InvalidPath));
        let long = vec!["x"; MAX_PATH_BYTES].join("/");
        assert_eq!(screen_paths([long.as_str()]), Err(ManifestError::InvalidPath));
    }
}
