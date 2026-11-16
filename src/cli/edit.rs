use super::Result;
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::{self, IsTerminal, Read, Write};
use std::path::{Component, Path, PathBuf};

const LIMIT: u64 = 1024 * 1024;

fn safe_path(root: &Path, relative: &Path) -> Result<PathBuf> {
    if relative.as_os_str().is_empty() { return Err("path cannot be empty".into()); }
    let mut path = root.to_path_buf();
    for component in relative.components() {
        let name = match component {
            Component::Normal(name) => name,
            _ => return Err("path must be relative and cannot contain parent traversal".into()),
        };
        let name_text = name.to_string_lossy();
        if name_text.chars().any(char::is_control) { return Err("path cannot contain control characters".into()); }
        if name_text == ".git" || name_text == ".isa" || name_text == ".env" || name_text.starts_with(".env.") || name_text == ".git-credentials" || name_text == ".netrc" {
            return Err("protected paths cannot be edited".into());
        }
        path.push(name);
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() { return Err("symlink paths are not supported".into()); }
    }
    let metadata = fs::metadata(&path)?;
    if !metadata.is_file() || metadata.len() > LIMIT { return Err("target must be a regular file smaller than 1 MiB".into()); }
    Ok(path)
}

fn confirm(question: &str) -> Result<bool> {
    if !io::stdin().is_terminal() { return Err("interactive approval is required; refusing unattended edit".into()); }
    eprint!("{question} [y/N] ");
    io::stderr().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().eq_ignore_ascii_case("y"))
}

fn visible(text: &str) -> String {
    text.chars().take(4000).map(|ch| if ch == '\n' || ch == '\t' || !ch.is_control() { ch } else { '�' }).collect()
}

fn backup_dir(root: &Path) -> Result<PathBuf> {
    let dir = root.join(".isa");
    match fs::symlink_metadata(&dir) {
        Ok(meta) if meta.file_type().is_symlink() || !meta.is_dir() => return Err(".isa must be a regular directory".into()),
        Ok(_) => (),
        Err(e) if e.kind() == io::ErrorKind::NotFound => fs::create_dir(&dir)?,
        Err(e) => return Err(e.into()),
    }
    Ok(dir)
}

fn atomic_write(path: &Path, data: &[u8], copy_permissions: bool) -> Result<()> {
    let parent = path.parent().ok_or("file has no parent")?;
    let temp = parent.join(format!(".isa-edit-{}-{}.tmp", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_nanos()));
    let result = (|| -> Result<()> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temp)?;
        file.write_all(data)?;
        if copy_permissions { file.set_permissions(fs::metadata(path)?.permissions())?; }
        file.sync_all()?;
        fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() { let _ = fs::remove_file(temp); }
    result
}

// One proposal changes one existing file. Delimiters use LF and do not form
// part of either text; an extra blank line preserves a trailing newline.
fn parse_proposal(text: &str) -> Result<(&str, &str, &str)> {
    const PREFIX: &str = "<<<<<<< SEARCH\n";
    const MIDDLE: &str = "\n=======\n";
    const END: &str = "\n>>>>>>> REPLACE";
    let (header, block) = text.split_once('\n').ok_or("missing proposal header")?;
    let file = header.strip_prefix("File: ").ok_or("expected File: <relative path> header")?;
    if file.is_empty() || file.len() > 1024 || file.chars().any(char::is_control) { return Err("invalid proposal path".into()); }
    let block = block.strip_prefix(PREFIX).ok_or("expected SEARCH marker")?;
    let (search, rest) = block.split_once(MIDDLE).ok_or("expected separator marker")?;
    let (replacement, trailing) = rest.split_once(END).ok_or("expected REPLACE marker")?;
    if !matches!(trailing, "" | "\n") || search.contains(MIDDLE) || replacement.contains(MIDDLE) || search.contains(END) || replacement.contains(END) {
        return Err("proposal must contain exactly one block without extra content or ambiguous markers".into());
    }
    if search.is_empty() { return Err("search text cannot be empty".into()); }
    if search.chars().chain(replacement.chars()).any(|ch| ch.is_control() && ch != '\n' && ch != '\t') {
        return Err("proposal contains control characters that cannot be reviewed safely".into());
    }
    Ok((file, search, replacement))
}

pub fn apply_proposal(root: &Path, proposal: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(proposal)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 32 * 1024 { return Err("proposal must be a regular file no larger than 32 KiB".into()); }
    let mut text = String::new();
    std::fs::File::open(proposal)?.take(32 * 1024 + 1).read_to_string(&mut text)?;
    if text.len() > 32 * 1024 { return Err("proposal exceeds 32 KiB".into()); }
    let (file, search, replacement) = parse_proposal(&text)?;
    replace(root, Path::new(file), search, replacement)
}

pub fn replace(root: &Path, relative: &Path, old: &str, new: &str) -> Result<()> {
    replace_with_approval(root, relative, old, new, || confirm("Apply this replacement?"))
}

fn replace_with_approval(root: &Path, relative: &Path, old: &str, new: &str, approve: impl FnOnce() -> Result<bool>) -> Result<()> {
    if old.is_empty() { return Err("search text cannot be empty".into()); }
    if old.chars().count() > 4000 || new.chars().count() > 4000 { return Err("replacement is too large to review safely (maximum 4000 characters per side)".into()); }
    let path = safe_path(root, relative)?;
    let before = fs::read_to_string(&path)?;
    if before.matches(old).count() != 1 { return Err("search text must match exactly once".into()); }
    let after = before.replacen(old, new, 1);
    if after == before { return Err("replacement would not change the file".into()); }
    if after.len() as u64 > LIMIT { return Err("result exceeds the 1 MiB limit".into()); }
    println!("File: {}\n--- remove ---\n{}\n--- add ---\n{}", relative.display(), visible(old), visible(new));
    if !approve()? { return Err("edit cancelled".into()); }
    if safe_path(root, relative)? != path || fs::read_to_string(&path)? != before { return Err("file changed while awaiting approval".into()); }
    let dir = backup_dir(root)?;
    let backup = serde_json::to_vec(&json!({"version": 1, "path": relative.to_str().ok_or("path is not UTF-8")?, "before": before, "after": after}))?;
    atomic_write(&dir.join("undo.json"), &backup, false)?;
    atomic_write(&path, after.as_bytes(), true)?;
    println!("Edit applied. Use 'isa-cli undo' to revert it if the file remains unchanged.");
    Ok(())
}

pub fn undo(root: &Path) -> Result<()> {
    let dir = backup_dir(root)?;
    let path = dir.join("undo.json");
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > LIMIT * 3 { return Err("invalid undo record".into()); }
    let record: Value = serde_json::from_slice(&fs::read(&path)?)?;
    if record.get("version").and_then(Value::as_u64) != Some(1) { return Err("unsupported undo record".into()); }
    let relative = Path::new(record.get("path").and_then(Value::as_str).ok_or("invalid undo path")?);
    let before = record.get("before").and_then(Value::as_str).ok_or("invalid undo content")?;
    let after = record.get("after").and_then(Value::as_str).ok_or("invalid undo content")?;
    if before.len() as u64 > LIMIT || after.len() as u64 > LIMIT { return Err("undo content exceeds limit".into()); }
    let target = safe_path(root, relative)?;
    if fs::read_to_string(&target)? != after { return Err("file has changed since the edit; refusing to overwrite it".into()); }
    println!("Undo the last ISA edit to {}", relative.display());
    if !confirm("Restore the previous file contents?")? { return Err("undo cancelled".into()); }
    if safe_path(root, relative)? != target || fs::read_to_string(&target)? != after { return Err("file changed while awaiting approval".into()); }
    atomic_write(&target, before.as_bytes(), true)?;
    fs::remove_file(path)?;
    println!("Edit undone.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traversal_is_rejected() {
        assert!(safe_path(Path::new("."), Path::new("../outside")).is_err());
    }

    #[test]
    fn parses_single_search_replace_and_preserves_trailing_newlines() {
        let (file, search, replacement) = parse_proposal("File: src/test.rs\n<<<<<<< SEARCH\nold\n\n=======\nnew\n\n>>>>>>> REPLACE\n").unwrap();
        assert_eq!((file, search, replacement), ("src/test.rs", "old\n", "new\n"));
        let (_, old, new) = parse_proposal("File: a\n<<<<<<< SEARCH\nold\n=======\n\n>>>>>>> REPLACE").unwrap();
        assert_eq!(old, "old"); assert_eq!(new, "");
    }

    #[test]
    fn refuses_ambiguous_malformed_and_unreviewable_proposals() {
        for text in [
            "File: a\n<<<<<<< SEARCH\n\n=======\nx\n>>>>>>> REPLACE",
            "File: a\n<<<<<<< SEARCH\nx\n=======\ny\n>>>>>>> REPLACE\nextra",
            "File: a\n<<<<<<< SEARCH\nx\n=======\ny\n=======\nz\n>>>>>>> REPLACE",
            "File: a\n<<<<<<< SEARCH\n\x1b[31m\n=======\ny\n>>>>>>> REPLACE",
            "File: a\n<<<<<<< SEARCH\nx\n=======\ny\n>>>>>>> REPLACE\nFile: b",
            "a\n<<<<<<< SEARCH\nx\n=======\ny\n>>>>>>> REPLACE",
        ] { assert!(parse_proposal(text).is_err(), "unexpectedly accepted {text:?}"); }
    }

    #[test]
    fn protected_paths_and_nonunique_search_do_not_change_files() {
        let root = std::env::temp_dir().join(format!("isa-proposal-test-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let target = root.join("test.txt"); fs::write(&target, "old old").unwrap();
        assert!(replace(&root, Path::new("test.txt"), "old", "new").is_err());
        assert_eq!(fs::read_to_string(&target).unwrap(), "old old");
        assert!(safe_path(&root, Path::new(".env")).is_err());
        assert!(safe_path(&root, Path::new("../escape")).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn approval_required_and_concurrent_change_rejected() {
        let root = std::env::temp_dir().join(format!("isa-proposal-approval-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let target = root.join("test.txt"); fs::write(&target, "old").unwrap();
        assert!(replace_with_approval(&root, Path::new("test.txt"), "old", "new", || Ok(false)).is_err());
        assert_eq!(fs::read_to_string(&target).unwrap(), "old");
        assert!(replace_with_approval(&root, Path::new("test.txt"), "old", "new", || { fs::write(&target, "changed")?; Ok(true) }).is_err());
        assert_eq!(fs::read_to_string(&target).unwrap(), "changed");
        fs::write(&target, "old").unwrap();
        replace_with_approval(&root, Path::new("test.txt"), "old", "new", || Ok(true)).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "new");
        let undo: Value = serde_json::from_slice(&fs::read(root.join(".isa/undo.json")).unwrap()).unwrap();
        assert_eq!(undo["before"], "old"); assert_eq!(undo["after"], "new");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn strips_terminal_controls() {
        assert!(!visible("\u{1b}[31m").contains('\u{1b}'));
    }
}
