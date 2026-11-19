//! Staged, purely in-memory export inventory checks for a Windows guest copy.
//!
//! Not connected to any filesystem, guest, or supervisor. Guest copy safety
//! additionally depends on handle-based traversal and identity verification;
//! a path and its reported type/size are not trustworthy evidence by themselves.
use super::windows_sandbox_manifest::{screen_paths, ManifestError};
use std::collections::HashMap;

// Provisional defensive limits, not claims about a supported workspace size.
const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 128 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Object {
    Directory,
    File {
        size: u64,
    },
    /// Any unsupported object, including links, junctions or reparse points.
    Unsupported,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Entry {
    pub(crate) path: String,
    pub(crate) object: Object,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ExportError {
    Paths(ManifestError),
    UnsupportedObject,
    MissingOrNonDirectoryParent,
    FileTooLarge,
    TotalTooLarge,
}

/// Validate a complete inventory. Every parent directory must appear in the
/// inventory. The root itself is implicit. `.isa` and `.git` must be excluded
/// by the future traversal; encountering them here rejects the whole inventory.
/// This does not copy files or validate that supplied metadata matches reality.
pub(crate) fn validate(entries: &[Entry]) -> Result<(), ExportError> {
    screen_paths(entries.iter().map(|entry| entry.path.as_str())).map_err(ExportError::Paths)?;
    let objects: HashMap<String, Object> = entries
        .iter()
        .map(|entry| (entry.path.to_ascii_lowercase(), entry.object))
        .collect();
    let mut total = 0u64;
    for entry in entries {
        let path = entry.path.to_ascii_lowercase();
        for (index, _) in path.match_indices('/') {
            if objects.get(&path[..index]) != Some(&Object::Directory) {
                return Err(ExportError::MissingOrNonDirectoryParent);
            }
        }
        match entry.object {
            Object::Directory => {}
            Object::Unsupported => return Err(ExportError::UnsupportedObject),
            Object::File { size } => {
                if size > MAX_FILE_BYTES {
                    return Err(ExportError::FileTooLarge);
                }
                total = total.checked_add(size).ok_or(ExportError::TotalTooLarge)?;
                if total > MAX_TOTAL_BYTES {
                    return Err(ExportError::TotalTooLarge);
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry(path: &str, object: Object) -> Entry {
        Entry {
            path: path.into(),
            object,
        }
    }
    #[test]
    fn accepts_empty_or_complete_inventory_in_any_order() {
        assert_eq!(validate(&[]), Ok(()));
        assert_eq!(
            validate(&[
                entry("a/b/file", Object::File { size: 3 }),
                entry("a", Object::Directory),
                entry("a/b", Object::Directory)
            ]),
            Ok(())
        );
    }
    #[test]
    fn requires_every_parent_to_be_a_directory() {
        assert_eq!(
            validate(&[entry("a/b", Object::File { size: 0 })]),
            Err(ExportError::MissingOrNonDirectoryParent)
        );
        assert_eq!(
            validate(&[
                entry("a", Object::File { size: 0 }),
                entry("a/b", Object::File { size: 0 })
            ]),
            Err(ExportError::MissingOrNonDirectoryParent)
        );
    }
    #[test]
    fn refuses_special_objects_protected_paths_and_duplicates() {
        assert_eq!(
            validate(&[entry("link", Object::Unsupported)]),
            Err(ExportError::UnsupportedObject)
        );
        assert!(matches!(
            validate(&[entry(".isa", Object::Directory)]),
            Err(ExportError::Paths(_))
        ));
        assert!(matches!(
            validate(&[entry(".git", Object::Directory)]),
            Err(ExportError::Paths(_))
        ));
        assert!(matches!(
            validate(&[entry("A", Object::Directory), entry("a", Object::Directory)]),
            Err(ExportError::Paths(_))
        ));
    }
    #[test]
    fn caps_files_and_total_even_for_small_inventories() {
        assert_eq!(
            validate(&[entry(
                "a",
                Object::File {
                    size: MAX_FILE_BYTES + 1
                }
            )]),
            Err(ExportError::FileTooLarge)
        );
        let mut entries: Vec<_> = (0..8)
            .map(|n| {
                entry(
                    &format!("f{n}"),
                    Object::File {
                        size: MAX_FILE_BYTES,
                    },
                )
            })
            .collect();
        assert_eq!(validate(&entries), Ok(()));
        entries.push(entry("extra", Object::File { size: 1 }));
        assert_eq!(validate(&entries), Err(ExportError::TotalTooLarge));
    }
}
