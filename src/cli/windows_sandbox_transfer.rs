//! Bounded in-memory receiver for a staged guest workspace transfer.
//!
//! This component does not authenticate a peer, read or write a filesystem,
//! establish trustworthy host snapshots, authorize import, or provide Windows
//! alias/reparse/hard-link protection. Guest manifests and digests are untrusted.
//! It is compiled only in tests and must not be used as an import boundary.
use super::windows_sandbox_export_plan::{self, Object};
use sha2::{Digest, Sha256};

const MAX_CHUNK: usize = 32 * 1024;

#[derive(Clone, Debug)]
pub(crate) enum Item {
    Directory {
        path: String,
    },
    File {
        path: String,
        size: u64,
        digest: [u8; 32],
    },
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ReceivedFile {
    pub(crate) path: String,
    pub(crate) contents: Vec<u8>,
}

/// Produced only after the caller signals end-of-transfer and every declared
/// file has passed checks. Completeness of a guest's claimed inventory is not
/// trustworthy and must be part of the separately reviewed import proposal.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ReceivedWorkspace {
    files: Vec<ReceivedFile>,
}
impl ReceivedWorkspace {
    pub(super) fn into_files(self) -> Vec<ReceivedFile> {
        self.files
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum TransferError {
    InvalidManifest,
    InvalidState,
    ChunkTooLarge,
    SizeMismatch,
    DigestMismatch,
    Incomplete,
    Failed,
}

pub(crate) struct Receiver {
    files: Vec<(String, usize, [u8; 32])>,
    index: usize,
    current: Vec<u8>,
    completed: Vec<ReceivedFile>,
    failed: bool,
}

impl Receiver {
    /// Entries must be an inventory in deterministic transfer order. Paths and
    /// limits are screened before receiving bytes, but this is not filesystem
    /// identity validation and file digests do not make guest data trustworthy.
    pub(crate) fn new(items: &[Item]) -> Result<Self, TransferError> {
        let entries: Vec<_> = items
            .iter()
            .map(|item| match item {
                Item::Directory { path } => windows_sandbox_export_plan::Entry {
                    path: path.clone(),
                    object: Object::Directory,
                },
                Item::File { path, size, .. } => windows_sandbox_export_plan::Entry {
                    path: path.clone(),
                    object: Object::File { size: *size },
                },
            })
            .collect();
        windows_sandbox_export_plan::validate(&entries)
            .map_err(|_| TransferError::InvalidManifest)?;
        let files = items
            .iter()
            .filter_map(|item| match item {
                Item::File { path, size, digest } => Some((path.clone(), *size as usize, *digest)),
                Item::Directory { .. } => None,
            })
            .collect();
        Ok(Self {
            files,
            index: 0,
            current: Vec::new(),
            completed: Vec::new(),
            failed: false,
        })
    }

    fn fail<T>(&mut self, err: TransferError) -> Result<T, TransferError> {
        self.failed = true;
        self.current.clear();
        self.completed.clear();
        Err(err)
    }

    /// Accept bytes for the next file only; an error poisons the receiver.
    pub(crate) fn push(&mut self, bytes: &[u8]) -> Result<(), TransferError> {
        if self.failed {
            return Err(TransferError::Failed);
        }
        if bytes.is_empty() || bytes.len() > MAX_CHUNK {
            return self.fail(TransferError::ChunkTooLarge);
        }
        let Some((_, expected, _)) = self.files.get(self.index) else {
            return self.fail(TransferError::InvalidState);
        };
        if self
            .current
            .len()
            .checked_add(bytes.len())
            .is_none_or(|n| n > *expected)
        {
            return self.fail(TransferError::SizeMismatch);
        }
        self.current.extend_from_slice(bytes);
        Ok(())
    }

    /// A completed file is accepted only after exact length and digest checks.
    pub(crate) fn end_file(&mut self) -> Result<(), TransferError> {
        if self.failed {
            return Err(TransferError::Failed);
        }
        let Some((path, expected, digest)) = self.files.get(self.index) else {
            return self.fail(TransferError::InvalidState);
        };
        if self.current.len() != *expected {
            return self.fail(TransferError::SizeMismatch);
        }
        let actual: [u8; 32] = Sha256::digest(&self.current).into();
        if actual != *digest {
            return self.fail(TransferError::DigestMismatch);
        }
        self.completed.push(ReceivedFile {
            path: path.clone(),
            contents: std::mem::take(&mut self.current),
        });
        self.index += 1;
        Ok(())
    }

    /// Partial transfers never expose a successful result.
    pub(crate) fn finish(mut self) -> Result<ReceivedWorkspace, TransferError> {
        if self.failed {
            return Err(TransferError::Failed);
        }
        if self.index != self.files.len() || !self.current.is_empty() {
            return self.fail(TransferError::Incomplete);
        }
        Ok(ReceivedWorkspace {
            files: self.completed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn file(path: &str, contents: &[u8]) -> Item {
        Item::File {
            path: path.into(),
            size: contents.len() as u64,
            digest: Sha256::digest(contents).into(),
        }
    }

    #[test]
    fn accepts_ordered_bounded_chunks_and_empty_files() {
        let mut receiver = Receiver::new(&[
            Item::Directory { path: "src".into() },
            file("src/a", b"hello"),
            file("empty", b""),
        ])
        .unwrap();
        receiver.push(b"he").unwrap();
        receiver.push(b"llo").unwrap();
        receiver.end_file().unwrap();
        receiver.end_file().unwrap();
        assert_eq!(
            receiver.finish().unwrap().into_files(),
            vec![
                ReceivedFile {
                    path: "src/a".into(),
                    contents: b"hello".to_vec()
                },
                ReceivedFile {
                    path: "empty".into(),
                    contents: vec![]
                }
            ]
        );
    }

    #[test]
    fn rejects_unsafe_manifests_before_accepting_data() {
        for items in [
            vec![file(".git/config", b"x")],
            vec![file("../x", b"x")],
            vec![file("dir/x", b"x")],
            vec![file("A", b"x"), file("a", b"y")],
        ] {
            assert!(matches!(
                Receiver::new(&items),
                Err(TransferError::InvalidManifest)
            ));
        }
    }

    #[test]
    fn rejects_truncation_overflow_tampering_and_further_input() {
        let items = [file("a", b"abc")];
        let mut receiver = Receiver::new(&items).unwrap();
        receiver.push(b"ab").unwrap();
        assert_eq!(receiver.end_file(), Err(TransferError::SizeMismatch));
        assert_eq!(receiver.push(b"c"), Err(TransferError::Failed));
        let mut receiver = Receiver::new(&items).unwrap();
        assert_eq!(receiver.push(b"abcd"), Err(TransferError::SizeMismatch));
        let mut receiver = Receiver::new(&items).unwrap();
        receiver.push(b"bad").unwrap();
        assert_eq!(receiver.end_file(), Err(TransferError::DigestMismatch));
        let receiver = Receiver::new(&items).unwrap();
        assert_eq!(receiver.finish(), Err(TransferError::Incomplete));
    }
}
