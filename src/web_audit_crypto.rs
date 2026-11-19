//! Experimental authenticated encryption for audit records.
//! Never automatically generate a replacement key when a key is unavailable.
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chacha20poly1305::{
    aead::{Aead, Payload},
    KeyInit, XChaCha20Poly1305, XNonce,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{io, path::Path};
use zeroize::Zeroizing;

const SERVICE: &str = "dev.isa.web-audit";
const AAD: &[u8] = b"ISA web audit record v1";
const MAX_PLAINTEXT: usize = 64 * 1024;
const MAX_ENCODED: usize = 100 * 1024;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    version: u8,
    nonce: String,
    ciphertext: String,
}
fn failure() -> io::Error {
    io::Error::other("Audit encryption or key storage is unavailable")
}

// The keychain account is workspace-specific without storing a path in it.
fn account(root: &Path) -> io::Result<String> {
    let path = root.canonicalize()?;
    #[cfg(unix)]
    let raw = {
        use std::os::unix::ffi::OsStrExt;
        path.as_os_str().as_bytes()
    };
    #[cfg(not(unix))]
    let raw = path.to_str().ok_or_else(failure)?.as_bytes();
    Ok(format!("workspace-{:x}", Sha256::digest(raw)))
}
fn entry(root: &Path) -> io::Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, &account(root)?).map_err(|_| failure())
}
fn random<const N: usize>() -> io::Result<[u8; N]> {
    let mut bytes = [0; N];
    getrandom::getrandom(&mut bytes).map_err(|_| failure())?;
    Ok(bytes)
}
/// Explicit one-time initialization. Never invoke automatically at server startup.
/// On Windows cooperating initializers for the same account share a mutex;
/// this cannot protect against non-cooperating credential-store writers or path
/// aliases. On other platforms, the operator must serialize initialization.
pub(crate) fn initialize(root: &Path) -> io::Result<()> {
    #[cfg(windows)]
    let _lock = windows_init_lock::acquire(&account(root)?)?;
    let key = entry(root)?;
    match key.get_password() {
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Audit key already exists",
            ))
        }
        Err(keyring::Error::NoEntry) => (),
        Err(_) => return Err(failure()),
    }
    // Refuse a new key if any previous encrypted log already exists.
    match std::fs::symlink_metadata(root.join(".isa/web-audit/sealed.jsonl")) {
        Ok(_) => {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Encrypted audit data already exists; restore its key",
            ))
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => (),
        Err(e) => return Err(e),
    }
    let bytes = Zeroizing::new(random::<32>()?);
    let encoded = Zeroizing::new(STANDARD.encode(&*bytes));
    key.set_password(&encoded).map_err(|_| failure())?;
    Ok(())
}
fn load(root: &Path) -> io::Result<Zeroizing<[u8; 32]>> {
    let encoded = Zeroizing::new(entry(root)?.get_password().map_err(|_| failure())?);
    let decoded = Zeroizing::new(STANDARD.decode(encoded.as_bytes()).map_err(|_| failure())?);
    let key: [u8; 32] = decoded.as_slice().try_into().map_err(|_| failure())?;
    Ok(Zeroizing::new(key))
}
fn seal_with(key: &[u8; 32], plain: &[u8], nonce: &[u8; 24]) -> io::Result<Vec<u8>> {
    if plain.is_empty() || plain.len() > MAX_PLAINTEXT {
        return Err(failure());
    }
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|_| failure())?;
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: plain,
                aad: AAD,
            },
        )
        .map_err(|_| failure())?;
    serde_json::to_vec(&Envelope {
        version: 1,
        nonce: STANDARD.encode(nonce),
        ciphertext: STANDARD.encode(ciphertext),
    })
    .map_err(|_| failure())
}
fn open_with(key: &[u8; 32], envelope: &[u8]) -> io::Result<Zeroizing<Vec<u8>>> {
    if envelope.len() > MAX_ENCODED {
        return Err(failure());
    }
    let envelope: Envelope = serde_json::from_slice(envelope).map_err(|_| failure())?;
    if envelope.version != 1 {
        return Err(failure());
    }
    let nonce = STANDARD.decode(envelope.nonce).map_err(|_| failure())?;
    if nonce.len() != 24 {
        return Err(failure());
    }
    let ciphertext = STANDARD
        .decode(envelope.ciphertext)
        .map_err(|_| failure())?;
    if ciphertext.len() < 16 || ciphertext.len() > MAX_PLAINTEXT + 16 {
        return Err(failure());
    }
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|_| failure())?;
    Ok(Zeroizing::new(
        cipher
            .decrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &ciphertext,
                    aad: AAD,
                },
            )
            .map_err(|_| failure())?,
    ))
}
pub(crate) fn check_key(root: &Path) -> io::Result<()> {
    let _key = load(root)?;
    Ok(())
}
#[cfg(all(test, windows))]
pub(crate) fn delete_test_key(root: &Path) -> io::Result<()> {
    entry(root)?.delete_credential().map_err(|_| failure())
}
pub(crate) fn seal(root: &Path, plain: &[u8]) -> io::Result<Vec<u8>> {
    let key = load(root)?;
    let nonce = random::<24>()?;
    seal_with(&key, plain, &nonce)
}
pub(crate) fn open(root: &Path, data: &[u8]) -> io::Result<Zeroizing<Vec<u8>>> {
    let key = load(root)?;
    open_with(&key, data)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn os_random_nonce_encrypts_and_decrypts() {
        let nonce = random::<24>().expect("OS random source must be available");
        let key = [7u8; 32];
        let sealed = seal_with(&key, b"audit test", &nonce).unwrap();
        assert_eq!(open_with(&key, &sealed).unwrap().as_slice(), b"audit test");
    }
    #[test]
    fn authenticated_envelope_detects_tampering_and_wrong_key() {
        let key = [7u8; 32];
        let plain = b"private argument and result";
        let sealed = seal_with(&key, plain, &[3; 24]).unwrap();
        assert!(!sealed.windows(plain.len()).any(|w| w == plain));
        assert_eq!(open_with(&key, &sealed).unwrap().as_slice(), plain);
        assert!(open_with(&[8; 32], &sealed).is_err());
        let mut envelope: Envelope = serde_json::from_slice(&sealed).unwrap();
        let mut ciphertext = STANDARD.decode(&envelope.ciphertext).unwrap();
        ciphertext[0] ^= 1;
        envelope.ciphertext = STANDARD.encode(ciphertext);
        assert!(open_with(&key, &serde_json::to_vec(&envelope).unwrap()).is_err());
    }
    #[cfg(windows)]
    #[test]
    fn windows_audit_credential_store_recovers_key_and_refuses_replacement() {
        use std::time::{SystemTime, UNIX_EPOCH};
        let root = std::env::temp_dir().join(format!(
            "isa-audit-key-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        // Clean up only this unique test account, even if an assertion fails.
        struct Cleanup(std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                if let Ok(e) = entry(&self.0) {
                    let _ = e.delete_credential();
                }
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(root.clone());
        let barrier = std::sync::Barrier::new(3);
        let (first, second) = std::thread::scope(|scope| {
            let first = scope.spawn(|| {
                barrier.wait();
                initialize(&root)
            });
            let second = scope.spawn(|| {
                barrier.wait();
                initialize(&root)
            });
            barrier.wait();
            (first.join().unwrap(), second.join().unwrap())
        });
        assert_ne!(
            first.is_ok(),
            second.is_ok(),
            "exactly one cooperating initializer must succeed: {first:?}, {second:?}"
        );
        let failure = if first.is_err() {
            first.unwrap_err()
        } else {
            second.unwrap_err()
        };
        assert_eq!(failure.kind(), io::ErrorKind::AlreadyExists);
        let sealed = seal(&root, b"private test payload").unwrap();
        assert_eq!(
            open(&root, &sealed).unwrap().as_slice(),
            b"private test payload"
        );
        assert_eq!(
            initialize(&root).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(
            open(&root, &sealed).unwrap().as_slice(),
            b"private test payload"
        );
        entry(&root).unwrap().delete_credential().unwrap();
        assert!(check_key(&root).is_err());
        assert!(open(&root, &sealed).is_err());
        std::fs::create_dir_all(root.join(".isa/web-audit")).unwrap();
        std::fs::write(root.join(".isa/web-audit/sealed.jsonl"), &sealed).unwrap();
        assert_eq!(
            initialize(&root).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert!(check_key(&root).is_err());
    }
    #[test]
    fn rejects_bad_format_and_invalid_bounds() {
        let key = [1u8; 32];
        assert!(seal_with(&key, b"", &[0; 24]).is_err());
        assert!(seal_with(&key, &vec![0; MAX_PLAINTEXT + 1], &[0; 24]).is_err());
        assert!(open_with(&key, b"{}").is_err());
        assert!(open_with(&key, &vec![b'x'; MAX_ENCODED + 1]).is_err());
        let mut e: Envelope =
            serde_json::from_slice(&seal_with(&key, b"x", &[4; 24]).unwrap()).unwrap();
        e.version = 2;
        assert!(open_with(&key, &serde_json::to_vec(&e).unwrap()).is_err());
        e.version = 1;
        e.nonce = STANDARD.encode([3; 23]);
        assert!(open_with(&key, &serde_json::to_vec(&e).unwrap()).is_err());
    }
}

#[cfg(windows)]
mod windows_init_lock {
    use std::{
        io,
        os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle},
        ptr,
    };
    use windows_sys::Win32::{
        Foundation::{WAIT_ABANDONED, WAIT_OBJECT_0, WAIT_TIMEOUT},
        System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject},
    };
    pub(super) struct Guard(OwnedHandle);
    impl Drop for Guard {
        fn drop(&mut self) {
            unsafe {
                ReleaseMutex(self.0.as_raw_handle() as _);
            }
        }
    }
    pub(super) fn acquire(account: &str) -> io::Result<Guard> {
        // The account is generated as a fixed ASCII prefix plus SHA-256 hex.
        if account.len() != 74
            || !account.starts_with("workspace-")
            || !account[10..]
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid audit key account",
            ));
        }
        let name: Vec<u16> = format!("Global\\IsaWebAuditKey-{account}")
            .encode_utf16()
            .chain(Some(0))
            .collect();
        // SAFETY: the NUL-terminated name is valid, and null attributes create
        // a non-inheritable mutex using the creating process's default DACL.
        let handle = unsafe { CreateMutexW(ptr::null(), 0, name.as_ptr()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let handle = unsafe { OwnedHandle::from_raw_handle(handle as RawHandle) };
        let result = unsafe { WaitForSingleObject(handle.as_raw_handle() as _, 5000) };
        match result {
            WAIT_OBJECT_0 => Ok(Guard(handle)),
            WAIT_ABANDONED => {
                unsafe {
                    ReleaseMutex(handle.as_raw_handle() as _);
                };
                Err(io::Error::other(
                    "audit key initializer was interrupted; inspect the credential store",
                ))
            }
            WAIT_TIMEOUT => Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "another audit key initializer is active",
            )),
            _ => Err(io::Error::last_os_error()),
        }
    }
}
