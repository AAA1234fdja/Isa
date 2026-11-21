//! Staged in-memory change proposal builder. It does not access the filesystem,
//! authorize an import or apply changes. Guest content and inventory are untrusted;
//! the host must obtain baseline snapshots through verified handles and obtain a
//! separate approval for the exact proposal before any future import.
use super::windows_sandbox_import_plan::{self, Change, Entry, PlanError, Snapshot};
use super::windows_sandbox_manifest::screen_paths;
use super::windows_sandbox_transfer::ReceivedWorkspace;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug)]
pub(crate) struct Baseline {
    pub(crate) path: String,
    pub(crate) snapshot: Snapshot,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ProposalError {
    InvalidBaseline,
    InvalidGuestPaths,
    CaseMismatch,
    Plan(PlanError),
}

pub(crate) struct Proposal {
    pub(crate) entries: Vec<Entry>,
    pub(crate) review_digest: [u8; 32],
}

/// Compare a completed guest inventory with a complete host-supplied baseline.
/// The caller must establish that the baseline was exported accurately; an
/// omitted guest file becomes a proposed deletion, never an automatic one.
/// Only file writes/deletes are represented; directory changes are not applied.
pub(crate) fn propose(
    baseline: &[Baseline],
    workspace: ReceivedWorkspace,
) -> Result<Option<Proposal>, ProposalError> {
    screen_paths(baseline.iter().map(|b| b.path.as_str()))
        .map_err(|_| ProposalError::InvalidBaseline)?;
    let mut old = BTreeMap::new();
    for entry in baseline {
        if !matches!(entry.snapshot, Snapshot::File { file_id: 1.., .. }) {
            return Err(ProposalError::InvalidBaseline);
        }
        old.insert(
            entry.path.to_ascii_lowercase(),
            (entry.path.clone(), entry.snapshot),
        );
    }
    let files = workspace.into_files();
    screen_paths(files.iter().map(|f| f.path.as_str()))
        .map_err(|_| ProposalError::InvalidGuestPaths)?;
    let mut entries = Vec::new();
    for file in files {
        let key = file.path.to_ascii_lowercase();
        let previous = match old.remove(&key) {
            Some((path, snapshot)) => {
                if path != file.path {
                    return Err(ProposalError::CaseMismatch);
                }
                snapshot
            }
            None => Snapshot::Missing,
        };
        let unchanged = match previous {
            Snapshot::File { digest, size, .. } => {
                size == file.contents.len() as u64
                    && digest == <[u8; 32]>::from(Sha256::digest(&file.contents))
            }
            Snapshot::Missing => false,
        };
        if !unchanged {
            entries.push(Entry {
                path: file.path,
                baseline: previous,
                change: Change::Write {
                    contents: file.contents,
                },
            });
        }
    }
    for (_, (path, snapshot)) in old {
        entries.push(Entry {
            path,
            baseline: snapshot,
            change: Change::Delete,
        });
    }
    if entries.is_empty() {
        return Ok(None);
    }
    // Canonical order makes the review digest independent of inventory order.
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    windows_sandbox_import_plan::validate(&entries).map_err(ProposalError::Plan)?;
    let review_digest = fingerprint(&entries);
    Ok(Some(Proposal {
        entries,
        review_digest,
    }))
}

/// Bind the complete reviewed paths, operation, baseline identity and resulting
/// content digest to one proposal. This digest is not an approval capability,
/// authentication, or protection against filesystem races.
fn fingerprint(entries: &[Entry]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"ISA Windows sandbox import proposal v1\0");
    hash.update((entries.len() as u64).to_be_bytes());
    for entry in entries {
        hash.update((entry.path.len() as u64).to_be_bytes());
        hash.update(entry.path.as_bytes());
        match entry.baseline {
            Snapshot::Missing => hash.update([0]),
            Snapshot::File {
                volume,
                file_id,
                digest,
                size,
            } => {
                hash.update([1]);
                hash.update(volume.to_be_bytes());
                hash.update(file_id.to_be_bytes());
                hash.update(size.to_be_bytes());
                hash.update(digest);
            }
        }
        match &entry.change {
            Change::Delete => hash.update([0]),
            Change::Write { contents } => {
                hash.update([1]);
                hash.update((contents.len() as u64).to_be_bytes());
                hash.update(Sha256::digest(contents));
            }
        }
    }
    hash.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::super::windows_sandbox_transfer::{Item, Receiver};
    use super::*;
    fn snapshot(bytes: &[u8], id: u64) -> Snapshot {
        Snapshot::File {
            volume: 1,
            file_id: id,
            digest: Sha256::digest(bytes).into(),
            size: bytes.len() as u64,
        }
    }
    fn baseline(path: &str, bytes: &[u8], id: u64) -> Baseline {
        Baseline {
            path: path.into(),
            snapshot: snapshot(bytes, id),
        }
    }
    fn workspace(files: &[(&str, &[u8])]) -> ReceivedWorkspace {
        let items: Vec<_> = files
            .iter()
            .map(|(path, bytes)| Item::File {
                path: (*path).into(),
                size: bytes.len() as u64,
                digest: Sha256::digest(bytes).into(),
            })
            .collect();
        let mut receiver = Receiver::new(&items).unwrap();
        for (_, bytes) in files {
            for chunk in bytes.chunks(32 * 1024) {
                receiver.push(chunk).unwrap();
            }
            receiver.end_file().unwrap();
        }
        receiver.finish().unwrap()
    }

    #[test]
    fn proposes_changes_and_deletions_only_for_review() {
        let old = [
            baseline("same", b"a", 1),
            baseline("edit", b"b", 2),
            baseline("delete", b"c", 3),
        ];
        let p = propose(
            &old,
            workspace(&[("same", b"a"), ("edit", b"changed"), ("new", b"d")]),
        )
        .unwrap()
        .unwrap();
        assert_eq!(p.entries.len(), 3);
        assert!(p
            .entries
            .iter()
            .any(|e| e.path == "delete" && e.change == Change::Delete));
        assert!(p
            .entries
            .iter()
            .any(|e| e.path == "new" && e.baseline == Snapshot::Missing));
        assert!(p.entries.iter().any(|e| e.path == "edit" && matches!(&e.change, Change::Write { contents } if contents.as_slice() == b"changed")));
        let current: Vec<_> = p.entries.iter().map(|e| e.baseline).collect();
        assert_eq!(
            windows_sandbox_import_plan::check_conflicts(&p.entries, &current),
            Ok(())
        );
        let mut conflicting = current;
        conflicting[0] = Snapshot::Missing;
        assert_eq!(
            windows_sandbox_import_plan::check_conflicts(&p.entries, &conflicting),
            Err(PlanError::Conflict)
        );
    }

    #[test]
    fn rejects_case_aliases_invalid_baselines_and_limits() {
        assert!(matches!(
            propose(&[baseline("File", b"a", 1)], workspace(&[("file", b"a")])),
            Err(ProposalError::CaseMismatch)
        ));
        assert!(matches!(
            propose(&[baseline("bad", b"a", 0)], workspace(&[])),
            Err(ProposalError::InvalidBaseline)
        ));
        assert!(matches!(
            propose(&[baseline(".git/key", b"a", 1)], workspace(&[])),
            Err(ProposalError::InvalidBaseline)
        ));
        let oversized = vec![0u8; 1024 * 1024 + 1];
        assert!(matches!(
            propose(&[], workspace(&[("large", oversized.as_slice())])),
            Err(ProposalError::Plan(PlanError::FileTooLarge))
        ));
    }

    #[test]
    fn review_digest_changes_with_content_and_baseline_identity() {
        let first = propose(&[baseline("file", b"a", 1)], workspace(&[("file", b"b")]))
            .unwrap()
            .unwrap();
        let reordered = propose(&[baseline("file", b"a", 1)], workspace(&[("file", b"b")]))
            .unwrap()
            .unwrap();
        let other_content = propose(&[baseline("file", b"a", 1)], workspace(&[("file", b"c")]))
            .unwrap()
            .unwrap();
        let other_identity = propose(&[baseline("file", b"a", 2)], workspace(&[("file", b"b")]))
            .unwrap()
            .unwrap();
        assert_eq!(first.review_digest, reordered.review_digest);
        assert_ne!(first.review_digest, other_content.review_digest);
        assert_ne!(first.review_digest, other_identity.review_digest);
        assert!(
            propose(&[baseline("file", b"a", 1)], workspace(&[("file", b"a")]))
                .unwrap()
                .is_none()
        );
    }
}
