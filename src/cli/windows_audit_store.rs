//! Experimental Windows storage for encrypted audit envelopes. This does not
//! prevent ancestor/path races and has not been tested on Windows.
//! Callers must encrypt and redact before calling; no plaintext fallback exists.
use super::windows_private_acl::{PrivateDescriptor,verify_handle};
use std::{fs::{File,OpenOptions},io::{self,Read,Seek,SeekFrom,Write},os::windows::{ffi::OsStrExt,fs::OpenOptionsExt,io::{FromRawHandle,OwnedHandle,RawHandle}},path::{Path,PathBuf},ptr};
use windows_sys::Win32::{Foundation::{GENERIC_READ,INVALID_HANDLE_VALUE},Storage::FileSystem::{CreateDirectoryW,CreateFileW,FILE_APPEND_DATA,FILE_ATTRIBUTE_NORMAL,FILE_FLAG_BACKUP_SEMANTICS,FILE_FLAG_OPEN_REPARSE_POINT,FILE_SHARE_READ,FILE_SHARE_WRITE,OPEN_ALWAYS}};
const MAX_LOG:u64=8*1024*1024;
const MAX_LINE:usize=100*1024;
fn invalid()->io::Error {io::Error::new(io::ErrorKind::PermissionDenied,"unsafe or unavailable audit storage")}
fn wide(path:&Path)->io::Result<Vec<u16>> {let mut name:Vec<u16>=path.as_os_str().encode_wide().collect();if name.contains(&0){return Err(invalid());}name.push(0);Ok(name)}
fn directory(path:&Path,create:bool)->io::Result<File> {
    if create {
        let descriptor=PrivateDescriptor::for_current_user(true)?;let attrs=descriptor.attributes();let name=wide(path)?;
        // SAFETY: path and descriptor remain live for this call.
        if unsafe {CreateDirectoryW(name.as_ptr(),&attrs)}==0 {let e=io::Error::last_os_error();if e.kind()!=io::ErrorKind::AlreadyExists {return Err(e);}}
    }
    let handle=OpenOptions::new().read(true).share_mode(FILE_SHARE_READ|FILE_SHARE_WRITE).custom_flags(FILE_FLAG_BACKUP_SEMANTICS|FILE_FLAG_OPEN_REPARSE_POINT).open(path)?;
    verify_handle(&handle,true)?;Ok(handle)
}
struct Directories {path:PathBuf,_base:File,_audit:File}
fn directories(root:&Path,create:bool)->io::Result<Directories> {
    let base=root.join(".isa");let audit=base.join("web-audit");let b=directory(&base,create)?;let a=directory(&audit,create)?;
    Ok(Directories {path:audit,_base:b,_audit:a})
}
// Structural validation prevents accidental plain JSON records, but is not
// proof of encryption or authenticity. Only pass output from seal().
fn valid_envelope(line:&[u8])->bool {
    use base64::{engine::general_purpose::STANDARD,Engine as _};
    if line.is_empty()||line.len()>MAX_LINE||line.contains(&b'\n')||line.contains(&b'\r'){return false;}
    let Ok(value)=serde_json::from_slice::<serde_json::Value>(line) else {return false;};
    let Some(object)=value.as_object() else {return false;};
    if object.len()!=3 || object.get("version").and_then(|v|v.as_u64())!=Some(1) {return false;}
    let Some(nonce)=object.get("nonce").and_then(|v|v.as_str()) else {return false;};
    let Some(ciphertext)=object.get("ciphertext").and_then(|v|v.as_str()) else {return false;};
    if nonce.len()!=32 || ciphertext.len()>MAX_LINE {return false;}
    STANDARD.decode(nonce).is_ok_and(|v|v.len()==24) && STANDARD.decode(ciphertext).is_ok_and(|v|v.len()>=16 && v.len()<=64*1024+16)
}
pub(crate) fn append_sealed(root:&Path,line:&[u8])->io::Result<()> {
    if !valid_envelope(line) {return Err(invalid());}
    let dirs=directories(root,true)?;let path=wide(&dirs.path.join("sealed.jsonl"))?;
    let descriptor=PrivateDescriptor::for_current_user(false)?;let attrs=descriptor.attributes();
    // No share mode: cross-process append/read contention fails closed rather
    // than allowing interleaved records. OPEN_ALWAYS only applies ACL on create;
    // existing files are verified through the returned handle before use.
    let raw=unsafe {CreateFileW(path.as_ptr(),GENERIC_READ|FILE_APPEND_DATA,0,&attrs,OPEN_ALWAYS,FILE_ATTRIBUTE_NORMAL|FILE_FLAG_OPEN_REPARSE_POINT,ptr::null_mut())};
    if raw==INVALID_HANDLE_VALUE {return Err(io::Error::last_os_error());}
    let handle=unsafe {OwnedHandle::from_raw_handle(raw as RawHandle)};let mut file=File::from(handle);
    verify_handle(&file,false)?;let len=file.metadata()?.len();
    if len.saturating_add(line.len() as u64+1)>MAX_LOG {return Err(io::Error::other("audit log size limit reached"));}
    if len>0 {file.seek(SeekFrom::End(-1))?;let mut last=[0u8;1];file.read_exact(&mut last)?;if last[0]!=b'\n' {return Err(invalid());}}
    let mut record=Vec::with_capacity(line.len()+1);record.extend_from_slice(line);record.push(b'\n');file.write_all(&record)?;file.sync_all()
}
pub(crate) fn read_sealed(root:&Path)->io::Result<Vec<u8>> {
    let dirs=match directories(root,false) {Ok(d)=>d,Err(e) if e.kind()==io::ErrorKind::NotFound=>return Ok(Vec::new()),Err(e)=>return Err(e)};
    let file=match OpenOptions::new().read(true).share_mode(0).custom_flags(FILE_FLAG_OPEN_REPARSE_POINT).open(dirs.path.join("sealed.jsonl")) {Ok(f)=>f,Err(e) if e.kind()==io::ErrorKind::NotFound=>return Ok(Vec::new()),Err(e)=>return Err(e)};
    verify_handle(&file,false)?;if file.metadata()?.len()>MAX_LOG {return Err(invalid());}
    let mut data=Vec::new();file.take(MAX_LOG+1).read_to_end(&mut data)?;
    if data.len() as u64>MAX_LOG || (!data.is_empty()&&!data.ends_with(b"\n")) || data.split(|b|*b==b'\n').filter(|l|!l.is_empty()).any(|l|!valid_envelope(l)) {return Err(invalid());}
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs,time::{SystemTime,UNIX_EPOCH}};
    fn root(label:&str)->PathBuf {let p=std::env::temp_dir().join(format!("isa-audit-win-{}-{}-{}",label,std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));fs::create_dir(&p).unwrap();p}
    // Structurally valid fixture only; this is not authenticated ciphertext.
    const ENVELOPE:&[u8]=br#"{"version":1,"nonce":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","ciphertext":"AAAAAAAAAAAAAAAAAAAAAA=="}"#;
    #[test]
    fn windows_audit_store_rejects_plaintext_truncation_and_hardlinks() {
        let root=root("records");assert!(read_sealed(&root).unwrap().is_empty());
        assert!(append_sealed(&root,b"private credentials").is_err());assert!(!root.join(".isa").exists());
        append_sealed(&root,ENVELOPE).unwrap();append_sealed(&root,ENVELOPE).unwrap();
        let contents=read_sealed(&root).unwrap();assert_eq!(contents.split(|b|*b==b'\n').filter(|l|!l.is_empty()).count(),2);
        let path=root.join(".isa/web-audit/sealed.jsonl");fs::hard_link(&path,root.join("alias")).unwrap();assert!(read_sealed(&root).is_err());assert!(append_sealed(&root,ENVELOPE).is_err());fs::remove_file(root.join("alias")).unwrap();
        let file=OpenOptions::new().write(true).open(&path).unwrap();file.set_len(contents.len() as u64-1).unwrap();drop(file);
        assert!(read_sealed(&root).is_err());assert!(append_sealed(&root,ENVELOPE).is_err());fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn windows_audit_store_roundtrips_real_credential_key_and_encryption() {
        use super::super::web_audit_crypto as crypto;
        let root=root("encrypted");
        struct Cleanup(PathBuf);impl Drop for Cleanup {fn drop(&mut self){let _=crypto::delete_test_key(&self.0);let _=fs::remove_dir_all(&self.0);}}
        let _cleanup=Cleanup(root.clone());crypto::initialize(&root).unwrap();
        let plaintext=b"sensitive audit argument and result";let sealed=crypto::seal(&root,plaintext).unwrap();
        append_sealed(&root,&sealed).unwrap();let stored=read_sealed(&root).unwrap();
        assert!(!stored.windows(plaintext.len()).any(|window|window==plaintext));
        assert_eq!(crypto::open(&root,stored.strip_suffix(b"\n").unwrap()).unwrap().as_slice(),plaintext);
        crypto::delete_test_key(&root).unwrap();assert!(crypto::open(&root,&sealed).is_err());
    }
    #[test]
    fn windows_audit_store_refuses_concurrent_file_open() {
        let root=root("sharing");append_sealed(&root,ENVELOPE).unwrap();
        let path=root.join(".isa/web-audit/sealed.jsonl");let file=OpenOptions::new().read(true).share_mode(0).open(path).unwrap();
        assert!(append_sealed(&root,ENVELOPE).is_err());assert!(read_sealed(&root).is_err());drop(file);assert!(read_sealed(&root).is_ok());fs::remove_dir_all(root).unwrap();
    }
}
