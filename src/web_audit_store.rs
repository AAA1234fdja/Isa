//! Bounded, redacted audit storage. Not tamper-proof or a transaction journal.
use std::{
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};
const MAX_LOG: u64 = 8 * 1024 * 1024;
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "unsafe or unavailable audit storage",
    )
}
#[cfg(unix)]
fn directory(root: &Path, create: bool) -> io::Result<PathBuf> {
    for (path, private) in [
        (root.join(".isa"), false),
        (root.join(".isa/web-audit"), true),
    ] {
        if create {
            match fs::create_dir(&path) {
                Ok(()) => fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?,
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => (),
                Err(e) => return Err(e),
            }
        }
        let m = fs::symlink_metadata(&path)?;
        if !m.is_dir()
            || m.file_type().is_symlink()
            || m.uid() != unsafe { libc::geteuid() }
            || m.mode() & if private { 0o077 } else { 0o022 } != 0
        {
            return Err(invalid());
        }
    }
    Ok(root.join(".isa/web-audit"))
}
#[cfg(unix)]
fn safe_file(file: &fs::File) -> io::Result<()> {
    let m = file.metadata()?;
    if !m.is_file()
        || m.nlink() != 1
        || m.uid() != unsafe { libc::geteuid() }
        || m.mode() & 0o077 != 0
        || m.len() > MAX_LOG
    {
        return Err(invalid());
    }
    Ok(())
}
#[cfg(unix)]
fn append_named(root: &Path, name: &str, line: &[u8], limit: usize) -> io::Result<()> {
    if line.is_empty() || line.len() > limit || line.contains(&b'\n') || line.contains(&b'\r') {
        return Err(invalid());
    }
    let dir = directory(root, true)?;
    let path = dir.join(name);
    let file = OpenOptions::new()
        .write(true)
        .append(true)
        .create(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&path)?;
    safe_file(&file)?;
    let size = file.metadata()?.len();
    if size.saturating_add(line.len() as u64 + 1) > MAX_LOG {
        return Err(io::Error::other("audit log size limit reached"));
    }
    let mut file = file;
    let mut entry = Vec::with_capacity(line.len() + 1);
    entry.extend_from_slice(line);
    entry.push(b'\n');
    file.write_all(&entry)?;
    file.sync_all()?;
    fs::File::open(dir)?.sync_all()?;
    Ok(())
}
#[cfg(unix)]
fn read_named(root: &Path, name: &str) -> io::Result<Vec<u8>> {
    let dir = match directory(root, false) {
        Ok(p) => p,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(dir.join(name))
    {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    safe_file(&file)?;
    let mut data = Vec::new();
    file.take(MAX_LOG + 1).read_to_end(&mut data)?;
    if data.len() as u64 > MAX_LOG || (!data.is_empty() && !data.ends_with(b"\n")) {
        return Err(invalid());
    }
    Ok(data)
}
#[cfg(unix)]
pub(crate) fn append(root: &Path, line: &[u8]) -> io::Result<()> {
    append_named(root, "events.jsonl", line, 4096)
}
#[cfg(unix)]
pub(crate) fn read(root: &Path) -> io::Result<Vec<u8>> {
    read_named(root, "events.jsonl")
}
#[cfg(unix)]
pub(crate) fn append_sealed(root: &Path, line: &[u8]) -> io::Result<()> {
    append_named(root, "sealed.jsonl", line, 100 * 1024)
}
#[cfg(unix)]
pub(crate) fn read_sealed(root: &Path) -> io::Result<Vec<u8>> {
    read_named(root, "sealed.jsonl")
}
#[cfg(windows)]
pub(crate) fn append_sealed(root: &Path, line: &[u8]) -> io::Result<()> {
    if !crate::windows_supervision_gate::audit_enabled() {
        return Err(invalid());
    }
    crate::cli::windows_audit_store::append_sealed(root, line)
}
#[cfg(windows)]
pub(crate) fn read_sealed(root: &Path) -> io::Result<Vec<u8>> {
    if !crate::windows_supervision_gate::audit_enabled() {
        return Err(invalid());
    }
    crate::cli::windows_audit_store::read_sealed(root)
}
#[cfg(not(any(unix, windows)))]
pub(crate) fn append_sealed(_: &Path, _: &[u8]) -> io::Result<()> {
    Err(invalid())
}
#[cfg(not(any(unix, windows)))]
pub(crate) fn read_sealed(_: &Path) -> io::Result<Vec<u8>> {
    Err(invalid())
}
#[cfg(not(unix))]
pub(crate) fn append(_: &Path, _: &[u8]) -> io::Result<()> {
    Err(invalid())
}
#[cfg(not(unix))]
pub(crate) fn read(_: &Path) -> io::Result<Vec<u8>> {
    Err(invalid())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[test]
    fn stores_bounded_private_lines_and_rejects_symlinks_and_hardlinks() {
        use std::os::unix::fs::{symlink, MetadataExt};
        let root = std::env::temp_dir().join(format!(
            "isa-audit-store-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        fs::create_dir(&root).unwrap();
        assert!(read(&root).unwrap().is_empty());
        append(&root, br#"{"version":1}"#).unwrap();
        append(&root, br#"{"version":2}"#).unwrap();
        assert_eq!(read(&root).unwrap(), b"{\"version\":1}\n{\"version\":2}\n");
        append_sealed(&root, b"ciphertext-only").unwrap();
        assert_eq!(read_sealed(&root).unwrap(), b"ciphertext-only\n");
        assert!(!read(&root)
            .unwrap()
            .windows(15)
            .any(|w| w == b"ciphertext-only"));
        let path = root.join(".isa/web-audit/events.jsonl");
        assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
        fs::hard_link(&path, root.join("extra")).unwrap();
        assert!(read(&root).is_err());
        assert!(append(&root, b"{}").is_err());
        fs::remove_file(root.join("extra")).unwrap();
        fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(MAX_LOG)
            .unwrap();
        assert!(append(&root, b"{}").is_err());
        assert!(read(&root).is_err());
        fs::remove_file(&path).unwrap();
        let outside = root.join("outside");
        fs::write(&outside, b"do not touch").unwrap();
        symlink(&outside, &path).unwrap();
        assert!(append(&root, b"{}").is_err());
        assert!(read(&root).is_err());
        assert_eq!(fs::read(outside).unwrap(), b"do not touch");
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(all(test, windows))]
mod windows_fail_closed_tests {
    use super::*;
    #[test]
    fn windows_fail_closed_audit_storage_never_creates_files() {
        let root =
            std::env::temp_dir().join(format!("isa-audit-unsupported-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        assert!(append(&root, b"{}").is_err());
        if !crate::windows_supervision_gate::audit_enabled() {
            assert!(append_sealed(&root, b"ciphertext").is_err());
            assert!(read_sealed(&root).is_err());
        }
        assert!(read(&root).is_err());
        assert!(!root.join(".isa").exists());
        std::fs::remove_dir(root).unwrap();
    }
}
