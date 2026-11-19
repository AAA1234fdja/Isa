//! Experimental plaintext undo store; NOT wired to web actions.
//! Private ACLs do not defend against same-user/admin access or hostile path
//! replacement. This module is unvalidated and must not be used in production.
use super::windows_private_acl::{verify_handle, PrivateDescriptor};
use super::windows_undo_frame::{self, MAX_FRAME};
use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    os::windows::{
        ffi::OsStrExt,
        fs::OpenOptionsExt,
        io::{FromRawHandle, OwnedHandle, RawHandle},
    },
    path::{Path, PathBuf},
    ptr,
};
use windows_sys::Win32::{
    Foundation::{GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE},
    Storage::FileSystem::{
        CreateDirectoryW, CreateFileW, CREATE_NEW, FILE_ATTRIBUTE_NORMAL,
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
        FILE_SHARE_WRITE,
    },
};
fn valid_id(id: &str) -> bool {
    id.len() == 48
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn wide(path: &Path) -> io::Result<Vec<u16>> {
    let mut units: Vec<u16> = path.as_os_str().encode_wide().collect();
    if units.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path contains NUL",
        ));
    }
    units.push(0);
    Ok(units)
}
fn directory(path: &Path, create: bool) -> io::Result<File> {
    if create {
        let descriptor = PrivateDescriptor::for_current_user(true)?;
        let attributes = descriptor.attributes();
        let path = wide(path)?;
        // SAFETY: path and descriptor are live through the create call.
        if unsafe { CreateDirectoryW(path.as_ptr(), &attributes) } == 0 {
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::AlreadyExists {
                return Err(error);
            }
        }
    }
    // Deny delete sharing while this handle is alive, so normal Win32
    // rename/deletion cannot replace the checked directory during use.
    let dir = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    verify_handle(&dir, true)?;
    Ok(dir)
}
struct PrivateDir {
    path: PathBuf,
    _base: File,
    _store: File,
}
fn private_dir(root: &Path, create: bool) -> io::Result<PrivateDir> {
    let base = root.join(".isa");
    let store = base.join("web-undo");
    let base_handle = directory(&base, create)?;
    let store_handle = directory(&store, create)?;
    // This narrows races but does not anchor opens relative to directory handles:
    // root ancestors and the gaps before open remain vulnerable to replacement.
    Ok(PrivateDir {
        path: store,
        _base: base_handle,
        _store: store_handle,
    })
}
pub(super) fn save(root: &Path, id: &str, bytes: &[u8]) -> io::Result<()> {
    if !valid_id(id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid undo id",
        ));
    }
    let frame = windows_undo_frame::encode(bytes)?;
    let dir = private_dir(root, true)?;
    let path = dir.path.join(id);
    let wide = wide(&path)?;
    let descriptor = PrivateDescriptor::for_current_user(false)?;
    let attributes = descriptor.attributes();
    // SAFETY: wide path and explicit ACL descriptor live through the call.
    // CREATE_NEW refuses to overwrite an existing record or final reparse point.
    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            0,
            &attributes,
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
            ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful CreateFileW returned a new owned file handle.
    let handle = unsafe { OwnedHandle::from_raw_handle(handle as RawHandle) };
    let mut file = File::from(handle);
    let result = (|| {
        verify_handle(&file, false)?;
        file.write_all(&frame)?;
        file.sync_all()
    })();
    // Do not delete by pathname on failure: another process could have replaced
    // that path. A partial, private record may remain and needs manual cleanup.
    drop(file);
    result
}
pub(super) fn load(root: &Path, id: &str) -> io::Result<Vec<u8>> {
    if !valid_id(id) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid undo id",
        ));
    }
    let dir = private_dir(root, false)?;
    let path = dir.path.join(id);
    // Prevent concurrent write/delete through handles opened after ours.
    let file = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    verify_handle(&file, false)?;
    if file.metadata()?.len() > MAX_FRAME as u64 {
        return Err(io::Error::other("undo record exceeds limit"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_FRAME as u64 + 1).read_to_end(&mut bytes)?;
    Ok(windows_undo_frame::decode(&bytes)?.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };
    fn root(label: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "isa-undo-{}-{}-{}",
            label,
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&p).unwrap();
        p
    }
    #[test]
    fn windows_undo_store_checked_directories_are_held_open() {
        let root = root("held");
        let dir = private_dir(&root, true).unwrap();
        assert!(fs::rename(&dir.path, root.join("moved")).is_err());
        assert!(fs::rename(root.join(".isa"), root.join("renamed")).is_err());
        drop(dir);
        fs::rename(root.join(".isa/web-undo"), root.join(".isa/moved")).unwrap();
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn windows_undo_store_rejects_incomplete_record() {
        let root = root("incomplete");
        let id = "c".repeat(48);
        save(&root, &id, b"private original").unwrap();
        let path = root.join(".isa/web-undo").join(&id);
        let file = OpenOptions::new().write(true).open(&path).unwrap();
        file.set_len(file.metadata().unwrap().len() - 1).unwrap();
        drop(file);
        assert!(load(&root, &id).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn windows_undo_store_private_records_roundtrip_and_reject_hardlinks() {
        let root = root("private");
        let id = "a".repeat(48);
        save(&root, &id, b"private original").unwrap();
        assert_eq!(load(&root, &id).unwrap(), b"private original");
        assert!(save(&root, &id, b"overwrite").is_err());
        assert!(load(&root, "../bad").is_err());
        fs::hard_link(root.join(".isa/web-undo").join(&id), root.join("alias")).unwrap();
        assert!(load(&root, &id).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn windows_undo_store_rejects_public_parent_acl_without_writing() {
        use std::{mem::size_of, ptr};
        use windows_sys::Win32::{
            Foundation::LocalFree,
            Security::Authorization::{
                ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
            },
            Security::SECURITY_ATTRIBUTES,
        };
        let root = root("public");
        let parent = root.join(".isa");
        let path = wide(&parent).unwrap();
        let sddl: Vec<u16> = "D:P(A;OICI;FA;;;WD)"
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let mut descriptor = ptr::null_mut();
        assert_ne!(
            unsafe {
                ConvertStringSecurityDescriptorToSecurityDescriptorW(
                    sddl.as_ptr(),
                    SDDL_REVISION_1,
                    &mut descriptor,
                    ptr::null_mut(),
                )
            },
            0
        );
        let attrs = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor,
            bInheritHandle: 0,
        };
        let created = unsafe { CreateDirectoryW(path.as_ptr(), &attrs) };
        unsafe { LocalFree(descriptor) };
        assert_ne!(created, 0, "{}", io::Error::last_os_error());
        assert!(save(&root, &"b".repeat(48), b"sensitive").is_err());
        assert!(!root.join(".isa/web-undo").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
