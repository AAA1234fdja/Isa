//! Experimental multi-file commit engine, used by the supervised action API.
//! Per-file rename is atomic, but the batch is NOT atomic to observers or crashes.
//! Callers must hold the same edit lock and validate paths, permissions and proposals.
use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_FILES: usize = 12;
const MAX_FILE_BYTES: usize = 1024 * 1024;
static SERIAL: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub(crate) struct Change {
    pub path: PathBuf,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    Applied,
    // No target was replaced by this engine.
    NotStarted,
    // All replaced targets were restored, but external writers may have observed them.
    RolledBack,
    // Some targets may have changed. Never automatically retry or overwrite them.
    Partial { unresolved: Vec<PathBuf> },
}

struct Stage {
    path: PathBuf,
    temp: Option<PathBuf>,
    backup: Option<PathBuf>,
}
impl Drop for Stage {
    fn drop(&mut self) {
        if let Some(p) = self.temp.take() {
            let _ = fs::remove_file(p);
        }
        if let Some(p) = self.backup.take() {
            let _ = fs::remove_file(p);
        }
    }
}
fn stage_file(target: &Path, contents: &[u8]) -> io::Result<PathBuf> {
    let parent = target
        .parent()
        .ok_or_else(|| io::Error::other("missing parent"))?;
    // create_new makes collisions fail closed; never open a pre-existing temp file.
    for _ in 0..16 {
        let name = format!(
            ".isa-txn-{}-{}.tmp",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        );
        let path = parent.join(name);
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = match options.open(&path) {
            Ok(f) => f,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        };
        let result = (|| {
            file.write_all(contents)?;
            file.set_permissions(fs::metadata(target)?.permissions())?;
            file.sync_all()
        })();
        if let Err(e) = result {
            let _ = fs::remove_file(&path);
            return Err(e);
        }
        return Ok(path);
    }
    Err(io::Error::other("temporary file collisions"))
}

// `validate` must recheck the target and all path components for each invocation.
// It must reject protected paths, symlinks and aliases under the caller's workspace.
// This engine cannot prevent races with external or hostile processes.
pub(crate) fn commit<F>(changes: &[Change], mut validate: F) -> Outcome
where
    F: FnMut(&Path) -> bool,
{
    commit_with_hook(changes, &mut validate, |_, _| Ok(()))
}
fn commit_with_hook<F, H>(changes: &[Change], validate: &mut F, mut before_rename: H) -> Outcome
where
    F: FnMut(&Path) -> bool,
    H: FnMut(usize, &Path) -> io::Result<()>,
{
    if changes.is_empty() || changes.len() > MAX_FILES {
        return Outcome::NotStarted;
    }
    let mut seen = HashSet::new();
    if changes.iter().any(|c| {
        !seen.insert(c.path.clone())
            || c.before.len() > MAX_FILE_BYTES
            || c.after.len() > MAX_FILE_BYTES
            || c.before == c.after
    }) {
        return Outcome::NotStarted;
    }
    let matches = |c: &Change, expected: &[u8], validate: &mut F| -> bool {
        validate(&c.path)
            && fs::File::open(&c.path)
                .and_then(|f| {
                    let mut data = Vec::new();
                    f.take((MAX_FILE_BYTES + 1) as u64).read_to_end(&mut data)?;
                    Ok(data == expected)
                })
                .unwrap_or(false)
    };
    let mut stages = Vec::new();
    for c in changes {
        if !matches(c, &c.before, validate) {
            return Outcome::NotStarted;
        }
        let mut s = Stage {
            path: c.path.clone(),
            temp: None,
            backup: None,
        };
        s.temp = match stage_file(&c.path, &c.after) {
            Ok(p) => Some(p),
            Err(_) => return Outcome::NotStarted,
        };
        s.backup = match stage_file(&c.path, &c.before) {
            Ok(p) => Some(p),
            Err(_) => return Outcome::NotStarted,
        };
        stages.push(s);
    }
    // Validate every snapshot again after staging, before the first replacement.
    if changes.iter().any(|c| !matches(c, &c.before, validate)) {
        return Outcome::NotStarted;
    }
    let mut applied = 0;
    for (i, c) in changes.iter().enumerate() {
        if !matches(c, &c.before, validate)
            || before_rename(i, &c.path).is_err()
            || fs::rename(stages[i].temp.as_ref().unwrap(), &c.path).is_err()
        {
            if applied == 0 {
                return Outcome::NotStarted;
            }
            let mut unresolved = Vec::new();
            for j in (0..applied).rev() {
                // Do not overwrite intervening edits. A race between this check and rename remains.
                if !matches(&changes[j], &changes[j].after, validate)
                    || fs::rename(stages[j].backup.as_ref().unwrap(), &stages[j].path).is_err()
                {
                    unresolved.push(stages[j].path.clone());
                } else {
                    stages[j].backup = None;
                }
            }
            return if unresolved.is_empty() {
                Outcome::RolledBack
            } else {
                Outcome::Partial { unresolved }
            };
        }
        stages[i].temp = None;
        applied += 1;
    }
    Outcome::Applied
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Workspace(PathBuf);
    impl Workspace {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!(
                "isa-txn-test-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&p).unwrap();
            Self(p)
        }
        fn change(&self, name: &str) -> Change {
            let path = self.0.join(name);
            fs::write(&path, b"before").unwrap();
            Change {
                path,
                before: b"before".to_vec(),
                after: b"after".to_vec(),
            }
        }
        fn clean(&self) {
            assert!(fs::read_dir(&self.0).unwrap().all(|e| !e
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".isa-txn-")));
        }
    }
    impl Drop for Workspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn applies_all_and_cleans_staging() {
        let w = Workspace::new();
        let c = vec![w.change("a"), w.change("b")];
        assert_eq!(commit(&c, |_| true), Outcome::Applied);
        for x in &c {
            assert_eq!(fs::read(&x.path).unwrap(), b"after");
        }
        w.clean();
    }
    #[test]
    fn stale_or_duplicate_never_writes() {
        let w = Workspace::new();
        let mut c = vec![w.change("a"), w.change("b")];
        fs::write(&c[1].path, b"external").unwrap();
        assert_eq!(commit(&c, |_| true), Outcome::NotStarted);
        assert_eq!(fs::read(&c[0].path).unwrap(), b"before");
        w.clean();
        c[1].path = c[0].path.clone();
        assert_eq!(commit(&c, |_| true), Outcome::NotStarted);
    }
    #[test]
    fn failure_rolls_back_completed_files() {
        let w = Workspace::new();
        let c = vec![w.change("a"), w.change("b")];
        let result = commit_with_hook(&c, &mut |_| true, |i, _| {
            if i == 1 {
                Err(io::Error::other("injected"))
            } else {
                Ok(())
            }
        });
        assert_eq!(result, Outcome::RolledBack);
        for x in &c {
            assert_eq!(fs::read(&x.path).unwrap(), b"before")
        }
        w.clean();
    }
    #[test]
    fn rollback_does_not_overwrite_intervening_change() {
        let w = Workspace::new();
        let c = vec![w.change("a"), w.change("b")];
        let result = commit_with_hook(&c, &mut |_| true, |i, _| {
            if i == 1 {
                fs::write(&c[0].path, b"external").unwrap();
                Err(io::Error::other("injected"))
            } else {
                Ok(())
            }
        });
        assert_eq!(
            result,
            Outcome::Partial {
                unresolved: vec![c[0].path.clone()]
            }
        );
        assert_eq!(fs::read(&c[0].path).unwrap(), b"external");
        w.clean();
    }
    #[test]
    fn twelve_files_are_allowed_but_thirteen_are_rejected() {
        let w = Workspace::new();
        let mut c: Vec<_> = (0..13).map(|i| w.change(&format!("{i}"))).collect();
        assert_eq!(commit(&c, |_| true), Outcome::NotStarted);
        assert!(c.iter().all(|x| fs::read(&x.path).unwrap() == b"before"));
        c.pop();
        assert_eq!(commit(&c, |_| true), Outcome::Applied);
        assert!(c.iter().all(|x| fs::read(&x.path).unwrap() == b"after"));
        w.clean();
    }
    #[test]
    fn invalidated_path_and_limits_fail_closed() {
        let w = Workspace::new();
        let c = vec![w.change("a"), w.change("b")];
        assert_eq!(commit(&c, |p| p != c[1].path), Outcome::NotStarted);
        assert_eq!(fs::read(&c[0].path).unwrap(), b"before");
        assert_eq!(commit(&[], |_| true), Outcome::NotStarted);
        w.clean();
    }
}
