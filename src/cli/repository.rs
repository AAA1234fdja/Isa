use super::Result;
use std::fs;
use std::path::{Path, PathBuf};
use std::io::Read;
use std::process::{Command, Stdio};

const MAX_FILES: usize = 20_000;
const MAX_FILE_BYTES: u64 = 1024 * 1024;
const MAX_RESULTS: usize = 1_000;

fn excluded(name: &str) -> bool {
    matches!(name, ".git" | ".isa" | "node_modules" | "target" | "dist" | "build" | ".venv" | "vendor" | ".env" | ".git-credentials" | ".netrc" | ".ssh" | ".aws" | "id_rsa" | "id_ed25519")
        || name.starts_with(".env.")
}

pub fn files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut result = Vec::new();
    let walker = ignore::WalkBuilder::new(root)
        .hidden(false)
        .follow_links(false)
        .require_git(false)
        .filter_entry(|entry| entry.depth() == 0 || !excluded(&entry.file_name().to_string_lossy()))
        .build();
    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                eprintln!("Skipped inaccessible path: {error}");
                continue;
            }
        };
        if entry.file_type().is_some_and(|kind| kind.is_file() && !kind.is_symlink()) {
            result.push(entry.path().strip_prefix(root)?.to_path_buf());
            if result.len() >= MAX_FILES {
                eprintln!("File listing reached the {MAX_FILES}-file limit.");
                break;
            }
        }
    }
    result.sort();
    Ok(result)
}

pub struct Hit {
    pub path: PathBuf,
    pub line: usize,
    pub text: String,
}

pub fn search(root: &Path, query: &str) -> Result<Vec<Hit>> {
    if query.is_empty() {
        return Err("search text must not be empty".into());
    }
    let mut hits = Vec::new();
    for path in files(root)? {
        let absolute = root.join(&path);
        let metadata = match fs::symlink_metadata(&absolute) {
            Ok(metadata) if metadata.is_file() && metadata.len() <= MAX_FILE_BYTES => metadata,
            _ => continue,
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        let content = match fs::read_to_string(&absolute) {
            Ok(content) if !content.contains('\0') => content,
            _ => continue,
        };
        for (index, line) in content.lines().enumerate() {
            if line.contains(query) {
                hits.push(Hit {
                    path: path.clone(),
                    line: index + 1,
                    text: line.chars().take(500).collect(),
                });
                if hits.len() >= MAX_RESULTS {
                    return Ok(hits);
                }
            }
        }
    }
    Ok(hits)
}

const MAX_GIT_OUTPUT: u64 = 2 * 1024 * 1024;

fn git(root: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .current_dir(root)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", if cfg!(windows) { "NUL" } else { "/dev/null" })
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_PAGER", "cat")
        .env("GIT_EXTERNAL_DIFF", "")
        .args(["-c", "core.fsmonitor=false", "-c", "core.hooksPath=/dev/null", "-c", "color.ui=false"]);
    command
}

fn git_output(mut command: Command) -> Result<String> {
    let mut child = command.stdout(Stdio::piped()).stderr(Stdio::null()).stdin(Stdio::null()).spawn()?;
    let stdout = child.stdout.take().ok_or("could not capture git output")?;
    let mut data = Vec::new();
    let read_result = stdout.take(MAX_GIT_OUTPUT + 1).read_to_end(&mut data);
    if read_result.is_err() || data.len() as u64 > MAX_GIT_OUTPUT {
        let _ = child.kill();
        let _ = child.wait();
        read_result?;
        return Err("git output exceeds the 2 MiB limit".into());
    }
    let status = child.wait()?;
    if !status.success() { return Err(format!("git exited with {status}").into()); }
    Ok(String::from_utf8_lossy(&data).into_owned())
}

pub fn git_context(root: &Path) -> Result<String> {
    let mut command = git(root);
    command.args(["status", "--short", "--untracked-files=no"]);
    git_output(command)
}

pub fn git_diff(root: &Path, staged: bool) -> Result<String> {
    let mut command = git(root);
    command.args(["diff", "--no-ext-diff", "--no-textconv", "--no-color", "--no-renames"]);
    if staged { command.arg("--cached"); }
    command.arg("--");
    git_output(command)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn excludes_sensitive_and_generated_names() {
        assert!(excluded(".git"));
        assert!(excluded(".env.local"));
        assert!(excluded("node_modules"));
        assert!(!excluded("main.rs"));
    }

    #[test]
    fn rejects_empty_search() {
        assert!(search(Path::new("."), "").is_err());
    }
}
