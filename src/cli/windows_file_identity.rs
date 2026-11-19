//! Experimental Windows file identity checks, not a filesystem security boundary.
//! Parent directory replacement, aliases and concurrent writes can still race.
use std::{fs::OpenOptions, io, mem::zeroed, os::windows::{fs::OpenOptionsExt, io::AsRawHandle}, path::Path};
use windows_sys::Win32::Storage::FileSystem::{GetFileInformationByHandle,BY_HANDLE_FILE_INFORMATION,FILE_ATTRIBUTE_DIRECTORY,FILE_ATTRIBUTE_REPARSE_POINT,FILE_FLAG_OPEN_REPARSE_POINT};

/// Return volume serial and file index for a single-link regular file.
/// Opens the final component without following a reparse point. This does not
/// prevent replacement of any path component before or after the handle opens.
pub(super) fn single_link_identity(path:&Path)->io::Result<(u64,u64)> {
    let file=OpenOptions::new().read(true).custom_flags(FILE_FLAG_OPEN_REPARSE_POINT).open(path)?;
    // SAFETY: all-zero initialization is valid for this Win32 output structure.
    let mut info:BY_HANDLE_FILE_INFORMATION=unsafe { zeroed() };
    // SAFETY: file owns a live file handle and info is writable for this call.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle() as _,&mut info) }==0 {return Err(io::Error::last_os_error());}
    if info.dwFileAttributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT)!=0 || info.nNumberOfLinks!=1 || !file.metadata()?.is_file() {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied,"reparse point, directory or hard-linked target is not supported"));
    }
    let file_id=((info.nFileIndexHigh as u64)<<32) | info.nFileIndexLow as u64;
    if file_id==0 {return Err(io::Error::other("filesystem did not report a stable file identifier"));}
    Ok((info.dwVolumeSerialNumber as u64,file_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs,time::{SystemTime,UNIX_EPOCH}};
    #[test]
    fn windows_file_identity_rejects_hardlinks_and_tracks_replacement() {
        let root=std::env::temp_dir().join(format!("isa-file-identity-{}-{}",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir(&root).unwrap();let file=root.join("file.txt");let link=root.join("hardlink.txt");
        fs::write(&file,b"first").unwrap();let first=single_link_identity(&file).unwrap();
        fs::hard_link(&file,&link).unwrap();assert!(single_link_identity(&file).is_err());assert!(single_link_identity(&link).is_err());
        fs::remove_file(link).unwrap();assert_eq!(single_link_identity(&file).unwrap(),first);
        let replacement=root.join("new.txt");fs::write(&replacement,b"second").unwrap();let second=single_link_identity(&replacement).unwrap();assert_ne!(first,second);
        assert!(single_link_identity(&root).is_err());fs::remove_dir_all(root).unwrap();
    }
}
