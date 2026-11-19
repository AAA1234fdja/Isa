//! Name filtering for workspace enumeration; not a secret scanner.
fn base_excluded(name: &str) -> bool {
    matches!(name, ".git" | ".isa" | "node_modules" | "target" | "dist" | "build" | ".venv" | "vendor" | ".env" | ".git-credentials" | ".netrc" | ".ssh" | ".aws" | "id_rsa" | "id_ed25519")
        || name.starts_with(".env.")
}

// The argument makes Windows name rules testable on other platforms. It is
// intentionally conservative; these rules cannot identify every filesystem alias.
fn excluded_for(name: &str, windows: bool) -> bool {
    if windows {
        if name.contains([':', '~']) || name.ends_with(['.', ' ']) { return true; }
        let lower = name.to_ascii_lowercase();
        base_excluded(&lower) || matches!(lower.as_str(), ".gnupg" | ".kube")
    } else {
        base_excluded(name)
    }
}

pub(super) fn excluded(name: &str) -> bool { excluded_for(name, cfg!(windows)) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_repository_filter_rejects_sensitive_case_variants_and_ambiguous_names() {
        for name in [".GIT", ".IsA", ".ENV", ".Env.Local", ".SSH", ".GnuPG", ".KUBE", "ID_RSA", "file:stream", "file.", "file ", "GIT~1"] {
            assert!(excluded_for(name, true), "accepted {name:?}");
        }
        for name in ["main.rs", "café.txt", "source-dir"] { assert!(!excluded_for(name, true), "rejected {name:?}"); }
        assert!(!excluded_for(".ENV", false));
        assert!(excluded_for(".env", false));
    }
}
