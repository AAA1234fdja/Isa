//! Conservative path-component checks for edits, not a filesystem sandbox.
use std::fs::Metadata;

fn protected(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | ".isa"
            | ".env"
            | ".git-credentials"
            | ".netrc"
            | ".ssh"
            | ".aws"
            | ".gnupg"
            | ".kube"
            | "id_rsa"
            | "id_ed25519"
    ) || name.starts_with(".env.")
}

// `windows` is an explicit argument so the name rules can be tested on any OS.
fn unsafe_component(name: &str, windows: bool) -> bool {
    if windows {
        // Avoid Win32 aliasing, alternate data streams, ambiguous trailing
        // characters and common 8.3 short-name aliases, including internal paths.
        if name.contains([':', '~']) || name.ends_with(['.', ' ']) {
            return true;
        }
        protected(&name.to_ascii_lowercase())
    } else {
        protected(name)
    }
}

pub(super) fn rejected_name(name: &str) -> bool {
    unsafe_component(name, cfg!(windows))
}

pub(super) fn is_link_or_reparse(metadata: &Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_path_guard_rejects_ambiguous_names_and_protected_case_variants() {
        for name in [
            ".GIT",
            ".Isa",
            ".ENV",
            ".EnV.Local",
            ".SSH",
            "ID_RSA",
            "file:stream",
            "file.",
            "file ",
            "GIT~1",
        ] {
            assert!(unsafe_component(name, true), "accepted {name:?}");
        }
        for name in ["source.rs", "project-name", "café.txt"] {
            assert!(!unsafe_component(name, true), "rejected {name:?}");
        }
        assert!(!unsafe_component(".GIT", false));
        assert!(!unsafe_component("file:stream", false));
        assert!(unsafe_component(".git", false));
    }
}
