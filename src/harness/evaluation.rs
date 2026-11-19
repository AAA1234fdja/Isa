use super::super::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::Path;
use std::time::Instant;

const MAX_SUITE_BYTES: u64 = 256 * 1024;
const MAX_CASES: usize = 20;
const MAX_TERMS: usize = 20;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Suite { version: u32, cases: Vec<Case> }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    name: String,
    prompt: String,
    #[serde(default)] contains: Vec<String>,
    #[serde(default)] excludes: Vec<String>,
    #[serde(default)] max_chars: Option<usize>,
}
#[derive(Serialize)]
struct CaseResult { name: String, passed: bool, failed_checks: Vec<String>, elapsed_ms: u64 }
#[derive(Serialize)]
pub struct Report { version: u32, model: String, mode: String, total: usize, passed: usize, failed: usize, cases: Vec<CaseResult> }

fn validate(suite: &Suite) -> Result<()> {
    if suite.version != 1 || suite.cases.is_empty() || suite.cases.len() > MAX_CASES { return Err("suite must be version 1 and contain 1 to 20 cases".into()); }
    let mut names = HashSet::new();
    for case in &suite.cases {
        if case.name.trim().is_empty() || case.name.len() > 100 || !names.insert(&case.name) { return Err("case names must be unique and 1 to 100 bytes".into()); }
        if case.prompt.trim().is_empty() || case.prompt.len() > 16 * 1024 { return Err("case prompt must be 1 to 16384 bytes".into()); }
        if case.contains.len() + case.excludes.len() > MAX_TERMS { return Err("case has too many checks".into()); }
        if case.contains.is_empty() && case.excludes.is_empty() && case.max_chars.is_none() { return Err("each case must contain at least one check".into()); }
        if case.contains.iter().chain(&case.excludes).any(|s| s.trim().is_empty() || s.len() > 200) { return Err("check terms must be 1 to 200 bytes".into()); }
        if case.max_chars.is_some_and(|n| n == 0 || n > 100_000) { return Err("max_chars must be between 1 and 100000".into()); }
    }
    Ok(())
}
fn checks(case: &Case, answer: &str) -> Vec<String> {
    let mut failed = Vec::new();
    if answer.trim().is_empty() { failed.push("empty_response".into()); return failed; }
    let answer_lower = answer.to_lowercase();
    for (index, term) in case.contains.iter().enumerate() {
        if !answer_lower.contains(&term.to_lowercase()) { failed.push(format!("contains[{}]", index)); }
    }
    for (index, term) in case.excludes.iter().enumerate() {
        if answer_lower.contains(&term.to_lowercase()) { failed.push(format!("excludes[{}]", index)); }
    }
    if case.max_chars.is_some_and(|limit| answer.chars().count() > limit) { failed.push("max_chars".into()); }
    failed
}
fn run_with<F>(suite: Suite, model: &str, mode: &str, mut run: F) -> Report where F: FnMut(&str) -> Result<String> {
    let mut cases = Vec::new();
    for case in &suite.cases {
        let start = Instant::now();
        let failed_checks = match run(&case.prompt) { Ok(answer) => checks(case, &answer), Err(_) => vec!["provider_error".into()] };
        let elapsed_ms = start.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
        cases.push(CaseResult { name: case.name.clone(), passed: failed_checks.is_empty(), failed_checks, elapsed_ms });
    }
    let passed = cases.iter().filter(|c| c.passed).count();
    Report { version: 1, model: model.into(), mode: mode.into(), total: cases.len(), passed, failed: cases.len() - passed, cases }
}
impl Report { pub fn passed(&self) -> bool { self.failed == 0 } }

pub(super) fn run<F>(path: &Path, model: &str, mode: &str, run: F) -> Result<Report> where F: FnMut(&str) -> Result<String> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > MAX_SUITE_BYTES { return Err("suite must be a regular file of at most 256 KiB".into()); }
    let bytes = fs::File::open(path)?.take(MAX_SUITE_BYTES + 1);
    let suite: Suite = serde_json::from_reader(bytes)?;
    validate(&suite)?;
    Ok(run_with(suite, model, mode, run))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn checks_are_case_insensitive_and_report_indices() {
        let suite: Suite = serde_json::from_str(r#"{"version":1,"cases":[{"name":"one","prompt":"test","contains":["HELLO"],"excludes":["bad"],"max_chars":5}]}"#).unwrap();
        assert!(validate(&suite).is_ok());
        assert!(checks(&suite.cases[0], "Hello").is_empty());
        assert_eq!(checks(&suite.cases[0], "bad hello"), vec!["excludes[0]", "max_chars"]);
        assert_eq!(checks(&suite.cases[0], ""), vec!["empty_response"]);
    }
    #[test] fn invalid_suites_are_rejected() {
        for json in [r#"{"version":1,"cases":[]}"#, r#"{"version":1,"cases":[{"name":"x","prompt":"p"}]}"#, r#"{"version":1,"cases":[{"name":"x","prompt":"p","contains":[""]}]}"#] {
            let suite: Suite = serde_json::from_str(json).unwrap(); assert!(validate(&suite).is_err());
        }
    }
    #[test] fn provider_error_does_not_expose_error_or_prompt() {
        let suite: Suite = serde_json::from_str(r#"{"version":1,"cases":[{"name":"safe","prompt":"sensitive prompt","contains":["ok"]}]}"#).unwrap();
        let report = run_with(suite, "model", "quick", |_| Err("secret provider error".into()));
        let json = serde_json::to_string(&report).unwrap();
        assert!(!json.contains("secret")); assert!(!json.contains("sensitive prompt")); assert_eq!(report.failed, 1);
    }
}
