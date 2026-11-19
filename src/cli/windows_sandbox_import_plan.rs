//! Staged import-plan validation; not connected to the supervisor or filesystem.
//!
//! These checks are not authorization and do not safely open or apply paths.
//! A future importer must obtain trustworthy snapshots through verified handles,
//! recheck them at apply time, and separately obtain approval for the exact plan.
use super::windows_sandbox_manifest::{screen_paths, ManifestError};

const MAX_FILE_BYTES: usize = 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 4 * 1024 * 1024;

/// Identity and content of a regular file, obtained by a future trusted host
/// snapshotter using a verified handle and collision-resistant digest. Neither
/// is obtained here. The volume and file ID must be stable and meaningful for
/// the supported filesystem; reject unsupported identities rather than guessing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Snapshot {
    Missing,
    File { volume: u64, file_id: u64, digest: [u8; 32], size: u64 },
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Change {
    Write { contents: Vec<u8> },
    Delete,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Entry {
    pub(crate) path: String,
    pub(crate) baseline: Snapshot,
    pub(crate) change: Change,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PlanError {
    Empty,
    Paths(ManifestError),
    OverlappingPaths,
    InvalidDelete,
    InvalidIdentity,
    FileTooLarge,
    TotalTooLarge,
    SnapshotCountMismatch,
    Conflict,
}

/// Structural checks only; the caller still needs object-type, alias and
/// filesystem-boundary checks. Directory and special-file operations are not
/// represented and must not be accepted by a future importer implicitly.
pub(crate) fn validate(entries: &[Entry]) -> Result<(), PlanError> {
    if entries.is_empty() { return Err(PlanError::Empty); }
    screen_paths(entries.iter().map(|entry| entry.path.as_str())).map_err(PlanError::Paths)?;
    // Disallow an entry that is a parent of another entry. This is a lexical
    // check only; actual Windows case and alias rules still need verification.
    let paths: std::collections::HashSet<String> = entries.iter().map(|entry| entry.path.to_ascii_lowercase()).collect();
    for path in &paths {
        for (index, _) in path.match_indices('/') {
            if paths.contains(&path[..index]) { return Err(PlanError::OverlappingPaths); }
        }
    }
    let mut total = 0usize;
    for entry in entries {
        if matches!(entry.baseline, Snapshot::File { file_id: 0, .. }) {
            return Err(PlanError::InvalidIdentity);
        }
        match &entry.change {
            Change::Delete if entry.baseline == Snapshot::Missing => return Err(PlanError::InvalidDelete),
            Change::Delete => {},
            Change::Write { contents } => {
                if contents.len() > MAX_FILE_BYTES { return Err(PlanError::FileTooLarge); }
                total = total.checked_add(contents.len()).ok_or(PlanError::TotalTooLarge)?;
                if total > MAX_TOTAL_BYTES { return Err(PlanError::TotalTooLarge); }
            }
        }
    }
    Ok(())
}

/// Compare a trusted, freshly obtained snapshot for each entry with the
/// baseline. An absent or untrustworthy snapshot must not be passed as Missing.
/// This does not prevent changes after comparison (TOCTOU).
pub(crate) fn check_conflicts(entries: &[Entry], current: &[Snapshot]) -> Result<(), PlanError> {
    validate(entries)?;
    if entries.len() != current.len() { return Err(PlanError::SnapshotCountMismatch); }
    if entries.iter().zip(current).any(|(entry, now)| entry.baseline != *now) {
        return Err(PlanError::Conflict);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn file(n: u8) -> Snapshot { Snapshot::File { volume: 7, file_id: 42, digest: [n; 32], size: 3 } }
    fn write(path: &str, baseline: Snapshot, bytes: usize) -> Entry {
        Entry { path: path.into(), baseline, change: Change::Write { contents: vec![0; bytes] } }
    }
    #[test]
    fn detects_conflicts_for_creations_modifications_and_deletions() {
        let entries = vec![write("new", Snapshot::Missing, 1), write("old", file(1), 1),
            Entry { path: "gone".into(), baseline: file(2), change: Change::Delete }];
        assert_eq!(check_conflicts(&entries, &[Snapshot::Missing, file(1), file(2)]), Ok(()));
        assert_eq!(check_conflicts(&entries, &[file(3), file(1), file(2)]), Err(PlanError::Conflict));
        assert_eq!(check_conflicts(&entries, &[Snapshot::Missing, file(3), file(2)]), Err(PlanError::Conflict));
        assert_eq!(check_conflicts(&entries, &[Snapshot::Missing, file(1), Snapshot::Missing]), Err(PlanError::Conflict));
        assert_eq!(check_conflicts(&entries, &[]), Err(PlanError::SnapshotCountMismatch));
    }
    #[test]
    fn detects_same_content_replacement_and_volume_change() {
        let entries = vec![write("file", file(1), 1)];
        let replaced = Snapshot::File { volume: 7, file_id: 43, digest: [1; 32], size: 3 };
        let other_volume = Snapshot::File { volume: 8, file_id: 42, digest: [1; 32], size: 3 };
        assert_eq!(check_conflicts(&entries, &[replaced]), Err(PlanError::Conflict));
        assert_eq!(check_conflicts(&entries, &[other_volume]), Err(PlanError::Conflict));
        let invalid = Snapshot::File { volume: 7, file_id: 0, digest: [1; 32], size: 3 };
        assert_eq!(validate(&[write("file", invalid, 1)]), Err(PlanError::InvalidIdentity));
    }
    #[test]
    fn rejects_invalid_paths_and_overlapping_entries() {
        assert!(matches!(validate(&[write(".isa/secret", Snapshot::Missing, 0)]), Err(PlanError::Paths(_))));
        assert_eq!(validate(&[write("a", Snapshot::Missing, 0), write("A/b", Snapshot::Missing, 0)]), Err(PlanError::OverlappingPaths));
        assert!(validate(&[write("a", Snapshot::Missing, 0), write("ab/c", Snapshot::Missing, 0)]).is_ok());
        assert_eq!(validate(&[write("a", Snapshot::Missing, 0), write("a-b", Snapshot::Missing, 0), write("a/b", Snapshot::Missing, 0)]), Err(PlanError::OverlappingPaths));
    }
    #[test]
    fn rejects_empty_delete_of_missing_and_excess_size() {
        assert_eq!(validate(&[]), Err(PlanError::Empty));
        assert_eq!(validate(&[Entry { path: "x".into(), baseline: Snapshot::Missing, change: Change::Delete }]), Err(PlanError::InvalidDelete));
        assert_eq!(validate(&[write("x", Snapshot::Missing, MAX_FILE_BYTES + 1)]), Err(PlanError::FileTooLarge));
        let entries: Vec<_> = (0..5).map(|i| write(&format!("f{i}"), Snapshot::Missing, MAX_FILE_BYTES)).collect();
        assert_eq!(validate(&entries), Err(PlanError::TotalTooLarge));
        assert!(validate(&entries[..4]).is_ok());
    }
}
