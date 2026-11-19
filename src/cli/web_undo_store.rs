//! Durable, unencrypted local undo records. Not a defense against hostile workspace races.
#[cfg(unix)]
use std::os::unix::{
    fs::{MetadataExt, OpenOptionsExt, PermissionsExt},
    io::AsRawFd,
};
use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};
const MAX_RECORD: u64 = 8 * 1024 * 1024;
fn valid_id(id: &str) -> bool {
    id.len() == 48
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
#[cfg(unix)]
fn private_dir(root: &Path, create: bool) -> io::Result<PathBuf> {
    fn dir(path: &Path, create: bool, private: bool) -> io::Result<()> {
        if create {
            match fs::create_dir(path) {
                Ok(()) => fs::set_permissions(path, fs::Permissions::from_mode(0o700))?,
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => (),
                Err(e) => return Err(e),
            }
        }
        let m = fs::symlink_metadata(path)?;
        if !m.is_dir()
            || m.file_type().is_symlink()
            || m.uid() != unsafe { libc_geteuid() }
            || m.mode() & if private { 0o077 } else { 0o022 } != 0
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "unsafe undo directory",
            ));
        }
        Ok(())
    }
    // Avoid depending on a crate in this isolated module's tests.
    unsafe extern "C" {
        fn geteuid() -> u32;
    }
    unsafe fn libc_geteuid() -> u32 {
        unsafe { geteuid() }
    }
    let isa = root.join(".isa");
    dir(&isa, create, false)?;
    let store = isa.join("web-undo");
    dir(&store, create, true)?;
    Ok(store)
}
#[cfg(not(unix))]
fn private_dir(_root: &Path, _create: bool) -> io::Result<PathBuf> {
    Err(io::Error::other("durable undo is supported only on Unix"))
}
#[cfg(unix)]
pub(crate) fn save(root: &Path, id: &str, bytes: &[u8]) -> io::Result<()> {
    if !valid_id(id) || bytes.len() as u64 > MAX_RECORD {
        return Err(io::Error::other("invalid undo record"));
    }
    let dir = private_dir(root, true)?;
    let path = dir.join(id);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true).mode(0o600);
    // create_new rejects an existing final component; parent path races remain possible.
    let mut file = options.open(&path)?;
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        let directory = fs::File::open(&dir)?;
        directory.sync_all()
    })();
    if result.is_err() {
        let _ = fs::remove_file(path);
    }
    result
}
#[cfg(not(unix))]
pub(crate) fn save(_root: &Path, _id: &str, _bytes: &[u8]) -> io::Result<()> {
    Err(io::Error::other("unsupported"))
}
#[cfg(unix)]
pub(crate) fn load(root: &Path, id: &str) -> io::Result<Vec<u8>> {
    if !valid_id(id) {
        return Err(io::Error::other("invalid undo id"));
    }
    let path = private_dir(root, false)?.join(id);
    let meta = fs::symlink_metadata(&path)?;
    if !meta.is_file()
        || meta.file_type().is_symlink()
        || meta.nlink() != 1
        || meta.uid() != unsafe { geteuid() }
        || meta.mode() & 0o077 != 0
        || meta.len() > MAX_RECORD
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "unsafe undo record",
        ));
    }
    unsafe extern "C" {
        fn geteuid() -> u32;
    }
    let file = OpenOptions::new().read(true).open(&path)?;
    let opened = file.metadata()?;
    if opened.dev() != meta.dev()
        || opened.ino() != meta.ino()
        || opened.nlink() != 1
        || opened.mode() & 0o077 != 0
    {
        return Err(io::Error::other("undo record changed"));
    }
    let _ = file.as_raw_fd();
    let mut bytes = Vec::new();
    file.take(MAX_RECORD + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_RECORD {
        return Err(io::Error::other("undo record too large"));
    }
    Ok(bytes)
}
#[cfg(not(unix))]
pub(crate) fn load(_root: &Path, _id: &str) -> io::Result<Vec<u8>> {
    Err(io::Error::other("unsupported"))
}
#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[test]
    fn persists_and_rejects_links_and_unsafe_permissions() {
        let root = std::env::temp_dir().join(format!("isa-durable-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir(&root).unwrap();
        let id = "a".repeat(48);
        save(&root, &id, b"private").unwrap();
        assert_eq!(load(&root, &id).unwrap(), b"private");
        assert!(save(&root, &id, b"overwrite").is_err());
        assert!(load(&root, "../escape").is_err());
        let path = root.join(".isa/web-undo").join(&id);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(load(&root, &id).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        fs::hard_link(&path, root.join("link")).unwrap();
        assert!(load(&root, &id).is_err());
        let _ = fs::remove_dir_all(root);
    }
}

#[cfg(all(test, windows))]
mod windows_fail_closed_tests {
    use super::*;
    #[test]
    fn windows_fail_closed_undo_storage_never_creates_files() {
        let root =
            std::env::temp_dir().join(format!("isa-undo-unsupported-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let id = "a".repeat(48);
        assert!(save(&root, &id, b"original contents").is_err());
        assert!(load(&root, &id).is_err());
        assert!(!root.join(".isa").exists());
        std::fs::remove_dir(root).unwrap();
    }
}
