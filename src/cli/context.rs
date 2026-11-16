use super::repository;
use super::Result;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

const MAX_FILE_BYTES: u64 = 512 * 1024;
const MAX_INDEXED: usize = 5_000;
const MAX_TOKENS_PER_FILE: usize = 30_000;
const MAX_QUERY_TERMS: usize = 32;

#[derive(Debug)]
pub struct RankedFile {
    pub path: PathBuf,
    pub score: f64,
    pub excerpt: String,
}

fn tokens(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|term| term.len() >= 2 && term.len() <= 80)
        .map(str::to_lowercase)
        .collect()
}

fn excerpt(content: &str, terms: &HashSet<String>) -> String {
    let mut selected = Vec::new();
    for (index, line) in content.lines().enumerate() {
        if tokens(line).iter().any(|word| terms.contains(word)) {
            selected.push(format!("{}: {}", index + 1, line.chars().take(200).collect::<String>()));
            if selected.len() == 3 { break; }
        }
    }
    selected.join("\n")
}

pub fn rank(root: &Path, query: &str, limit: usize) -> Result<Vec<RankedFile>> {
    let terms: HashSet<String> = tokens(query).into_iter().take(MAX_QUERY_TERMS).collect();
    if terms.is_empty() { return Err("query must contain at least one searchable word".into()); }
    let mut documents = Vec::new();
    let mut frequencies: HashMap<String, usize> = HashMap::new();
    for path in repository::files(root)?.into_iter().take(MAX_INDEXED) {
        let absolute = root.join(&path);
        let metadata = match fs::symlink_metadata(&absolute) {
            Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() && metadata.len() <= MAX_FILE_BYTES => metadata,
            _ => continue,
        };
        if metadata.len() == 0 { continue; }
        let content = match fs::read_to_string(&absolute) {
            Ok(content) if !content.contains('\0') => content,
            _ => continue,
        };
        let words = tokens(&content);
        let mut counts = HashMap::<String, usize>::new();
        for word in words.iter().take(MAX_TOKENS_PER_FILE) {
            if terms.contains(word) { *counts.entry(word.clone()).or_default() += 1; }
        }
        for word in counts.keys() { *frequencies.entry(word.clone()).or_default() += 1; }
        documents.push((path, content, counts, words.len().min(MAX_TOKENS_PER_FILE)));
    }
    let total = documents.len() as f64;
    let mut results = Vec::new();
    for (path, content, counts, length) in documents {
        let mut score = 0.0;
        for (term, count) in &counts {
            let documents_with_term = *frequencies.get(term).unwrap_or(&0) as f64;
            let idf = ((total + 1.0) / (documents_with_term + 1.0)).ln() + 1.0;
            score += (1.0 + (*count as f64).ln()) * idf / (1.0 + (length as f64 / 1000.0).ln_1p());
        }
        if score > 0.0 {
            results.push(RankedFile { path, score, excerpt: excerpt(&content, &terms) });
        }
    }
    results.sort_by(|a, b| b.score.total_cmp(&a.score).then_with(|| a.path.cmp(&b.path)));
    results.truncate(limit.min(100));
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenization_is_case_insensitive() {
        assert_eq!(tokens("Hello, HELLO_world! x"), vec!["hello", "hello_world"]);
    }

    #[test]
    fn excerpt_reports_line_numbers() {
        let terms = HashSet::from(["needle".to_string()]);
        assert_eq!(excerpt("first\nNeedle here\nlast", &terms), "2: Needle here");
    }

    #[test]
    fn empty_query_is_rejected() {
        assert!(rank(Path::new("."), "!", 10).is_err());
    }
}

const MAX_PACK_CHARS: usize = 12_000;
const MAX_PACK_FILES: usize = 12;

pub fn pack(root: &Path, query: &str) -> Result<serde_json::Value> {
    let results = rank(root, query, MAX_PACK_FILES)?;
    let mut remaining = MAX_PACK_CHARS;
    let mut sources = Vec::new();
    for result in results {
        let path = result.path.to_string_lossy().into_owned();
        let overhead = path.chars().count();
        if overhead >= remaining { break; }
        remaining -= overhead;
        let excerpt: String = result.excerpt.chars().take(remaining).collect();
        remaining -= excerpt.chars().count();
        sources.push(serde_json::json!({
            "path": path,
            "score": result.score,
            "excerpt": excerpt,
        }));
        if remaining == 0 { break; }
    }
    Ok(serde_json::json!({
        "version": 1,
        "method": "lexical",
        "query": query,
        "sources_are_untrusted": true,
        "sources": sources,
    }))
}

#[cfg(test)]
mod pack_tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn pack_limits_sources_and_marks_them_untrusted() {
        let path = std::env::temp_dir().join(format!("isa-pack-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir(&path).unwrap();
        fs::write(path.join("one.rs"), "fn token_example() { token(); }\n").unwrap();
        let packed = pack(&path, "token").unwrap();
        assert_eq!(packed["method"], "lexical");
        assert_eq!(packed["sources_are_untrusted"], true);
        assert_eq!(packed["sources"].as_array().unwrap().len(), 1);
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn pack_rejects_empty_query() {
        assert!(pack(Path::new("."), "?").is_err());
    }
}
