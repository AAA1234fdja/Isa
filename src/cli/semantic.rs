use super::{repository, Result};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MAX_FILE: u64 = 256 * 1024;
const MAX_INDEX: u64 = 128 * 1024 * 1024;
const MAX_CHUNKS: usize = 10_000;
const CHUNK_CHARS: usize = 2_000;
const MAX_VECTOR: usize = 8192;

#[derive(Serialize, Deserialize, Clone)]
struct Chunk { path: String, start_line: usize, end_line: usize, hash: String, text: String, vector: Vec<f32> }
#[derive(Serialize, Deserialize)]
struct Index { version: u32, model: String, digest: String, chunks: Vec<Chunk> }

fn digest(text: &str) -> String { format!("{:x}", Sha256::digest(text.as_bytes())) }
fn safe_path(path: &str) -> bool {
    !path.is_empty() && Path::new(path).components().all(|part| matches!(part, Component::Normal(_)))
}
fn location(root: &Path, create: bool) -> Result<PathBuf> {
    let dir = root.join(".isa");
    match fs::symlink_metadata(&dir) {
        Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => (),
        Ok(_) => return Err(".isa must be a real directory".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && create => fs::create_dir(&dir)?,
        Err(e) => return Err(e.into()),
    }
    Ok(dir.join("semantic-index.json"))
}
fn load(root: &Path) -> Result<Index> {
    let path = location(root, false)?;
    let meta = fs::symlink_metadata(&path)?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > MAX_INDEX { return Err("invalid semantic index file".into()); }
    let index: Index = serde_json::from_reader(fs::File::open(path)?.take(MAX_INDEX + 1))?;
    if index.version != 1 || index.chunks.len() > MAX_CHUNKS || index.model.is_empty() || index.digest.is_empty() { return Err("invalid semantic index metadata".into()); }
    for chunk in &index.chunks {
        if !safe_path(&chunk.path) || chunk.start_line == 0 || chunk.end_line < chunk.start_line || chunk.text.chars().count() > CHUNK_CHARS || !valid_vector(&chunk.vector) { return Err("invalid semantic index chunk".into()); }
    }
    Ok(index)
}
fn save(root: &Path, index: &Index) -> Result<()> {
    let path = location(root, true)?;
    let bytes = serde_json::to_vec(index)?;
    if bytes.len() as u64 > MAX_INDEX { return Err("semantic index exceeds size limit".into()); }
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let temp = path.with_file_name(format!(".semantic-{}-{nonce}.tmp", std::process::id()));
    let result = (|| -> Result<()> {
        let mut opts = OpenOptions::new(); opts.write(true).create_new(true);
        #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; opts.mode(0o600); }
        let mut file = opts.open(&temp)?; file.write_all(&bytes)?; file.sync_all()?; fs::rename(&temp, &path)?; Ok(())
    })();
    if result.is_err() { let _ = fs::remove_file(temp); } result
}
fn client() -> Result<Client> {
    Ok(Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none()).timeout(Duration::from_secs(120)).build()?)
}
fn model_digest(client: &Client, model: &str) -> Result<String> {
    let response = client.get("http://127.0.0.1:11434/api/tags").send()?.error_for_status()?;
    let data: serde_json::Value = serde_json::from_reader(response.take(4 * 1024 * 1024))?;
    let name = if model.contains(':') { model.to_owned() } else { format!("{model}:latest") };
    data["models"].as_array().and_then(|models| models.iter().find(|entry| entry["name"].as_str() == Some(name.as_str())))
        .and_then(|entry| entry["digest"].as_str()).filter(|s| !s.is_empty()).map(str::to_owned)
        .ok_or_else(|| "embedding model not found in local Ollama; pull it first".into())
}
fn valid_vector(vector: &[f32]) -> bool {
    !vector.is_empty() && vector.len() <= MAX_VECTOR && vector.iter().all(|v| v.is_finite()) && vector.iter().any(|v| *v != 0.0)
}
fn embed(client: &Client, model: &str, input: &str) -> Result<Vec<f32>> {
    let response = client.post("http://127.0.0.1:11434/api/embed").json(&serde_json::json!({"model": model, "input": input, "truncate": false})).send()?.error_for_status()?;
    let value: serde_json::Value = serde_json::from_reader(response.take(1024 * 1024))?;
    let vector: Vec<f32> = serde_json::from_value(value["embeddings"][0].clone())?;
    if !valid_vector(&vector) { return Err("Ollama returned an invalid embedding".into()); } Ok(vector)
}
fn split(content: &str) -> Vec<(usize, usize, String)> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut start = 1;
    let mut end = 1;
    for (idx, line) in content.lines().enumerate() {
        let line_num = idx + 1;
        for part in line.chars().collect::<Vec<_>>().chunks(CHUNK_CHARS - 1) {
            let piece: String = part.iter().collect();
            if current.chars().count() + piece.chars().count() + 1 > CHUNK_CHARS {
                result.push((start, end, std::mem::take(&mut current))); start = line_num;
            }
            if current.is_empty() { start = line_num; }
            current.push_str(&piece); current.push('\n'); end = line_num;
        }
        if line.is_empty() && !current.is_empty() && current.chars().count() < CHUNK_CHARS { current.push('\n'); end = line_num; }
    }
    if !current.trim().is_empty() { result.push((start, end, current)); }
    result
}
fn cosine(a: &[f32], b: &[f32]) -> Result<f64> {
    if !valid_vector(a) || !valid_vector(b) || a.len() != b.len() { return Err("embedding dimensions or values do not match; rebuild the index".into()); }
    let (mut dot, mut aa, mut bb) = (0.0f64, 0.0f64, 0.0f64);
    for (&x, &y) in a.iter().zip(b) { let x = f64::from(x); let y = f64::from(y); dot += x*y; aa += x*x; bb += y*y; }
    Ok(dot / (aa.sqrt() * bb.sqrt()))
}
fn read_source(root: &Path, path: &Path) -> Option<String> {
    let path = root.join(path); let meta = fs::symlink_metadata(&path).ok()?;
    if !meta.is_file() || meta.file_type().is_symlink() || meta.len() > MAX_FILE { return None; }
    let text = fs::read_to_string(path).ok()?; if text.contains('\0') { None } else { Some(text) }
}

pub fn build(root: &Path, model: &str) -> Result<()> {
    if model.len() > 200 || model.trim().is_empty() { return Err("invalid model name".into()); }
    let client = client()?; let model_hash = model_digest(&client, model)?;
    let old = match load(root) { Ok(index) if index.model == model && index.digest == model_hash => index.chunks, _ => Vec::new() };
    let mut cache: HashMap<String, Vec<f32>> = HashMap::new();
    for chunk in old { cache.insert(chunk.hash, chunk.vector); }
    let mut chunks = Vec::new(); let mut reused = 0; let mut dimensions = None;
    for path in repository::files(root)? {
        let Some(path_str) = path.to_str().filter(|p| safe_path(p)) else { continue };
        let Some(content) = read_source(root, &path) else { continue };
        for (start, end, text) in split(&content) {
            if text.trim().is_empty() { continue; }
            if chunks.len() >= MAX_CHUNKS { return Err("semantic index chunk limit reached; narrow the workspace".into()); }
            let hash = digest(&text);
            let vector = if let Some(vector) = cache.get(&hash) { reused += 1; vector.clone() } else { let vector = embed(&client, model, &text)?; cache.insert(hash.clone(), vector.clone()); vector };
            if dimensions.is_some_and(|n| n != vector.len()) { return Err("inconsistent embedding dimensions".into()); } dimensions = Some(vector.len());
            chunks.push(Chunk { path: path_str.to_owned(), start_line: start, end_line: end, hash, text, vector });
        }
    }
    if model_digest(&client, model)? != model_hash { return Err("embedding model changed during indexing; retry".into()); }
    let count = chunks.len(); save(root, &Index { version: 1, model: model.into(), digest: model_hash, chunks })?;
    println!("Indexed {count} chunks ({reused} reused) with {model}. Rebuild after workspace changes."); Ok(())
}
pub fn search(root: &Path, query: &str) -> Result<serde_json::Value> {
    if query.trim().is_empty() || query.chars().count() > CHUNK_CHARS { return Err("query must contain 1 to 2000 characters".into()); }
    let index = load(root)?; let client = client()?;
    if model_digest(&client, &index.model)? != index.digest { return Err("embedding model changed; rebuild the index".into()); }
    let vector = embed(&client, &index.model, query)?;
    let allowed: HashSet<PathBuf> = repository::files(root)?.into_iter().collect();
    let mut fresh: HashMap<String, HashSet<(usize, usize, String)>> = HashMap::new();
    let mut hits = Vec::new();
    for chunk in &index.chunks {
        if !allowed.contains(Path::new(&chunk.path)) { continue; }
        let hashes = fresh.entry(chunk.path.clone()).or_insert_with(|| read_source(root, Path::new(&chunk.path)).map(|text| split(&text).into_iter().map(|(start,end,text)| (start, end, digest(&text))).collect()).unwrap_or_default());
        if !hashes.contains(&(chunk.start_line, chunk.end_line, chunk.hash.clone())) { continue; }
        hits.push((cosine(&vector, &chunk.vector)?, chunk));
    }
    hits.sort_by(|a,b| b.0.total_cmp(&a.0).then_with(|| a.1.path.cmp(&b.1.path)).then_with(|| a.1.start_line.cmp(&b.1.start_line)));
    let sources: Vec<_> = hits.into_iter().take(10).map(|(score, c)| serde_json::json!({"path": c.path,"start_line":c.start_line,"end_line":c.end_line,"score":score,"excerpt":c.text})).collect();
    Ok(serde_json::json!({"version":1,"method":"semantic","model":index.model,"query":query,"sources_are_untrusted":true,"sources":sources}))
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn cosine_ranks_by_direction() { assert!(cosine(&[1.0, 0.0], &[1.0, 0.0]).unwrap() > cosine(&[1.0, 0.0], &[0.0, 1.0]).unwrap()); assert!(cosine(&[0.0], &[0.0]).is_err()); assert!(cosine(&[1.0], &[1.0, 2.0]).is_err()); }
    #[test] fn chunks_are_bounded_and_preserve_unicode() { let text = format!("first\n{}\nlast", "é".repeat(3000)); let chunks = split(&text); assert!(chunks.iter().all(|(_,_,s)| s.chars().count() <= CHUNK_CHARS)); assert_eq!(chunks[0].0, 1); assert_eq!(chunks.last().unwrap().1, 3); }
    #[test] fn paths_are_relative() { assert!(safe_path("src/main.rs")); assert!(!safe_path("../secret")); assert!(!safe_path("/etc/passwd")); }
}
