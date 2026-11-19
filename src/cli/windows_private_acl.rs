//! Staged user-only Windows security descriptors. Not connected to undo.
//! Creating a private object alone does not validate existing parents, defend
//! against path races, or guarantee protection from administrators/the same user.
use std::{
    ffi::c_void,
    io,
    mem::size_of,
    os::windows::io::{FromRawHandle, OwnedHandle, RawHandle},
    ptr,
};
use windows_sys::Win32::{
    Foundation::{LocalFree, HANDLE},
    Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
        SDDL_REVISION_1,
    },
    Security::{
        GetTokenInformation, IsValidSid, TokenUser, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
    },
    System::Threading::{GetCurrentProcess, OpenProcessToken},
};

struct LocalMemory(*mut c_void);
impl Drop for LocalMemory {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                LocalFree(self.0);
            }
        }
    }
}

fn current_user_sid() -> io::Result<String> {
    let mut raw: HANDLE = ptr::null_mut();
    // SAFETY: GetCurrentProcess returns a valid pseudo-handle, and raw is writable.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: a successful OpenProcessToken returns an owned handle.
    let token = unsafe { OwnedHandle::from_raw_handle(raw as RawHandle) };
    use std::os::windows::io::AsRawHandle;
    let mut needed = 0u32;
    // SAFETY: size query uses a null buffer and valid return-length pointer.
    unsafe {
        GetTokenInformation(
            token.as_raw_handle() as _,
            TokenUser,
            ptr::null_mut(),
            0,
            &mut needed,
        );
    }
    if needed < (size_of::<TOKEN_USER>() as u32) || needed > 65536 {
        return Err(io::Error::other("invalid token user information size"));
    }
    let words = (needed as usize)
        .checked_add(size_of::<usize>() - 1)
        .ok_or_else(|| io::Error::other("token size overflow"))?
        / size_of::<usize>();
    let mut data = vec![0usize; words];
    let mut returned = needed;
    // SAFETY: the aligned allocation has at least needed bytes and remains alive.
    if unsafe {
        GetTokenInformation(
            token.as_raw_handle() as _,
            TokenUser,
            data.as_mut_ptr().cast(),
            needed,
            &mut returned,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    if returned > needed || returned < (size_of::<TOKEN_USER>() as u32) {
        return Err(io::Error::other("invalid token user information"));
    }
    // SAFETY: successful TokenUser query initialized the aligned TOKEN_USER.
    let sid = unsafe { (*(data.as_ptr() as *const TOKEN_USER)).User.Sid };
    if sid.is_null() || unsafe { IsValidSid(sid) } == 0 {
        return Err(io::Error::other("invalid token SID"));
    }
    let mut sid_string: *mut u16 = ptr::null_mut();
    // SAFETY: sid belongs to the live token information buffer; output is writable.
    if unsafe { ConvertSidToStringSidW(sid, &mut sid_string) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if sid_string.is_null() {
        return Err(io::Error::other("missing SID string"));
    }
    let _guard = LocalMemory(sid_string.cast());
    let mut len = 0usize;
    // Windows returns a NUL-terminated SID string. A bounded scan fails closed.
    while len <= 184 && unsafe { *sid_string.add(len) } != 0 {
        len += 1;
    }
    if len > 184 {
        return Err(io::Error::other("SID string is too long"));
    }
    // SAFETY: the bounded scan established a readable initialized string slice.
    String::from_utf16(unsafe { std::slice::from_raw_parts(sid_string, len) })
        .map_err(|_| io::Error::other("invalid SID string"))
}

pub(super) struct PrivateDescriptor {
    memory: LocalMemory,
}
impl PrivateDescriptor {
    pub(super) fn for_current_user(directory: bool) -> io::Result<Self> {
        let text = super::windows_acl_policy::private_sddl(&current_user_sid()?, directory)?;
        let wide: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        let mut descriptor = ptr::null_mut();
        // SAFETY: wide is NUL-terminated and descriptor is a writable out-pointer.
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                wide.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor,
                ptr::null_mut(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        if descriptor.is_null() {
            return Err(io::Error::other("missing security descriptor"));
        }
        Ok(Self {
            memory: LocalMemory(descriptor),
        })
    }
    /// Attributes are non-inheritable as process handles. The descriptor must
    /// stay alive throughout the Windows create call receiving these attributes.
    pub(super) fn attributes(&self) -> SECURITY_ATTRIBUTES {
        SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: self.memory.0,
            bInheritHandle: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        os::windows::ffi::OsStrExt,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };
    use windows_sys::Win32::{
        Security::Authorization::{GetNamedSecurityInfoW, SE_FILE_OBJECT},
        Security::{
            EqualSid, GetAce, GetSecurityDescriptorControl, ACCESS_ALLOWED_ACE, ACL,
            DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION, SE_DACL_PROTECTED,
        },
        Storage::FileSystem::{CreateDirectoryW, FILE_ALL_ACCESS},
    };
    #[test]
    fn windows_private_acl_rejects_directory_accessible_to_everyone() {
        use std::os::windows::fs::OpenOptionsExt;
        let root = std::env::temp_dir().join(format!(
            "isa-public-acl-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path: Vec<u16> = root.as_os_str().encode_wide().chain(Some(0)).collect();
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
        let _guard = LocalMemory(descriptor);
        let attrs = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor,
            bInheritHandle: 0,
        };
        assert_ne!(unsafe { CreateDirectoryW(path.as_ptr(), &attrs) }, 0);
        let handle = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(
                windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS
                    | windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT,
            )
            .open(&root)
            .unwrap();
        assert!(verify_handle(&handle, true).is_err());
        drop(handle);
        std::fs::remove_dir(root).unwrap();
    }
    #[test]
    fn windows_private_acl_created_directory_has_only_owner_allow_ace() {
        let root: PathBuf = std::env::temp_dir().join(format!(
            "isa-private-acl-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path: Vec<u16> = root.as_os_str().encode_wide().chain(Some(0)).collect();
        let sd = PrivateDescriptor::for_current_user(true).unwrap();
        let attrs = sd.attributes();
        // SAFETY: path and descriptor are valid through this create call.
        assert_ne!(
            unsafe { CreateDirectoryW(path.as_ptr(), &attrs) },
            0,
            "{}",
            io::Error::last_os_error()
        );
        let mut owner = ptr::null_mut();
        let mut dacl: *mut ACL = ptr::null_mut();
        let mut descriptor = ptr::null_mut();
        // SAFETY: path is NUL-terminated and all requested output pointers are valid.
        let status = unsafe {
            GetNamedSecurityInfoW(
                path.as_ptr(),
                SE_FILE_OBJECT,
                OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
                &mut owner,
                ptr::null_mut(),
                &mut dacl,
                ptr::null_mut(),
                &mut descriptor,
            )
        };
        assert_eq!(status, 0);
        let _guard = LocalMemory(descriptor);
        assert!(!owner.is_null() && !dacl.is_null());
        let mut control = 0u16;
        let mut revision = 0u32;
        assert_ne!(
            unsafe { GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) },
            0
        );
        assert_ne!(control & SE_DACL_PROTECTED, 0);
        assert_eq!(unsafe { (*dacl).AceCount }, 1);
        let mut raw_ace = ptr::null_mut();
        assert_ne!(unsafe { GetAce(dacl, 0, &mut raw_ace) }, 0);
        let ace = raw_ace as *const ACCESS_ALLOWED_ACE;
        assert_eq!(unsafe { (*ace).Header.AceType }, 0); // ACCESS_ALLOWED_ACE_TYPE
        assert_eq!(unsafe { (*ace).Mask } & FILE_ALL_ACCESS, FILE_ALL_ACCESS);
        let ace_sid = unsafe { ptr::addr_of!((*ace).SidStart).cast_mut().cast() };
        assert_ne!(unsafe { EqualSid(owner, ace_sid) }, 0);
        use std::os::windows::fs::OpenOptionsExt;
        let handle = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(
                windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS
                    | windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT,
            )
            .open(&root)
            .unwrap();
        verify_handle(&handle, true).unwrap();
        drop(handle);
        std::fs::remove_dir(root).unwrap();
    }
}

/// Conservatively verify an already-open file or directory handle. Existing
/// directories with inherited or additional entries are rejected, not repaired.
/// This alone does not make later path-based operations race-free.
pub(super) fn verify_handle(file: &std::fs::File, directory: bool) -> io::Result<()> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::{
        Security::Authorization::{ConvertStringSidToSidW, GetSecurityInfo, SE_FILE_OBJECT},
        Security::{
            EqualSid, GetAce, GetSecurityDescriptorControl, ACCESS_ALLOWED_ACE, ACL,
            DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION, SE_DACL_PROTECTED,
        },
        Storage::FileSystem::{
            GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_ALL_ACCESS,
            FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
        },
    };
    let denied = || {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            "unsafe Windows storage permissions or file identity",
        )
    };
    // SAFETY: zero initialization is valid for this Win32 output structure.
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    // SAFETY: file owns a valid handle and info is writable.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle() as _, &mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || (info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0) != directory
        || (!directory && info.nNumberOfLinks != 1)
    {
        return Err(denied());
    }
    let sid_text = current_user_sid()?;
    let sid_wide: Vec<u16> = sid_text.encode_utf16().chain(Some(0)).collect();
    let mut current = ptr::null_mut();
    // SAFETY: NUL-terminated SID string and writable out-pointer.
    if unsafe { ConvertStringSidToSidW(sid_wide.as_ptr(), &mut current) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if current.is_null() {
        return Err(denied());
    }
    let _sid_guard = LocalMemory(current);
    let mut owner = ptr::null_mut();
    let mut acl: *mut ACL = ptr::null_mut();
    let mut descriptor = ptr::null_mut();
    // SAFETY: the file handle and all requested out-pointers are valid.
    let status = unsafe {
        GetSecurityInfo(
            file.as_raw_handle() as _,
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            ptr::null_mut(),
            &mut acl,
            ptr::null_mut(),
            &mut descriptor,
        )
    };
    if status != 0 {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    if descriptor.is_null() {
        return Err(denied());
    }
    let _descriptor_guard = LocalMemory(descriptor);
    if owner.is_null() || acl.is_null() || unsafe { EqualSid(owner, current) } == 0 {
        return Err(denied());
    }
    let mut control = 0u16;
    let mut revision = 0u32;
    // SAFETY: descriptor was returned by GetSecurityInfo and outputs are valid.
    if unsafe { GetSecurityDescriptorControl(descriptor, &mut control, &mut revision) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if control & SE_DACL_PROTECTED == 0 || unsafe { (*acl).AceCount } != 1 {
        return Err(denied());
    }
    let mut raw_ace = ptr::null_mut();
    // SAFETY: acl is returned by GetSecurityInfo and contains exactly one ACE.
    if unsafe { GetAce(acl, 0, &mut raw_ace) } == 0 || raw_ace.is_null() {
        return Err(denied());
    }
    let header = raw_ace as *const windows_sys::Win32::Security::ACE_HEADER;
    // Check the header before reading the larger access-allowed ACE fields.
    let (kind, flags, size) = unsafe {
        (
            (*header).AceType,
            (*header).AceFlags,
            (*header).AceSize as usize,
        )
    };
    if kind != 0 || flags & (0x10 | 0x08) != 0 || size < 16 {
        return Err(denied());
    }
    let ace = raw_ace as *const ACCESS_ALLOWED_ACE;
    if unsafe { (*ace).Mask } & FILE_ALL_ACCESS != FILE_ALL_ACCESS {
        return Err(denied());
    }
    // SAFETY: the validated ACE has room for the minimum eight-byte SID header.
    let ace_sid = unsafe { ptr::addr_of!((*ace).SidStart).cast_mut().cast::<u8>() };
    let subauthorities = unsafe { *ace_sid.add(1) } as usize;
    if subauthorities > 15 || 8 + 4 * subauthorities > size - 8 {
        return Err(denied());
    }
    if unsafe { IsValidSid(ace_sid.cast()) } == 0
        || unsafe { EqualSid(ace_sid.cast(), current) } == 0
    {
        return Err(denied());
    }
    Ok(())
}
