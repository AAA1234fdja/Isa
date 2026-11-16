use super::{session, Result};
mod evaluation;
mod plan;
mod retrieval;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode { Quick, Balanced, Thorough }
impl Mode {
    fn parse(value: &str) -> Result<Self> {
        match value { "quick" => Ok(Self::Quick), "balanced" => Ok(Self::Balanced), "" | "thorough" => Ok(Self::Thorough), _ => Err("ISA_HARNESS_MODE must be quick, balanced, or thorough".into()) }
    }
}

trait Engine {
    fn run(&mut self, model: &str, messages: &[session::Message], guidance: &str, on_chunk: &mut dyn FnMut(&str) -> Result<()>) -> Result<String>;
}

struct Ollama;
impl Engine for Ollama {
    fn run(&mut self, model: &str, messages: &[session::Message], guidance: &str, on_chunk: &mut dyn FnMut(&str) -> Result<()>) -> Result<String> {
        session::ollama_stream_with_guidance(model, messages, guidance, on_chunk)
    }
}

fn execute<E: Engine>(engine: &mut E, model: &str, messages: &[session::Message], mode: Mode, on_chunk: &mut dyn FnMut(&str) -> Result<()>) -> Result<String> {
    plan::validate(messages)?;
    if mode == Mode::Quick {
        eprintln!("ISA harness: answering in quick mode...");
        return engine.run(model, messages, plan::BASELINE, on_chunk);
    }
    eprintln!("ISA harness: testing response strategies...");
    let mut drafts = Vec::with_capacity(2);
    for guidance in [plan::BASELINE, plan::ALTERNATIVE] {
        match engine.run(model, messages, guidance, &mut |_| Ok(())) {
            Ok(draft) if !draft.trim().is_empty() => drafts.push(draft),
            _ => eprintln!("ISA harness: a response probe failed."),
        }
    }
    if drafts.is_empty() { return Err("all harness response probes failed; no answer was produced".into()); }
    let critique = if mode == Mode::Thorough && drafts.len() > 1 {
        eprintln!("ISA harness: comparing candidate responses...");
        let review_messages = plan::augmented(messages, &drafts, None)?;
        match engine.run(model, &review_messages, plan::REVIEW, &mut |_| Ok(())) {
            Ok(text) if !text.trim().is_empty() => Some(text),
            _ => { eprintln!("ISA harness: comparison failed; composing without it."); None }
        }
    } else { None };
    let compose_messages = plan::augmented(messages, &drafts, critique.as_deref())?;
    eprintln!("ISA harness: composing the response...");
    engine.run(model, &compose_messages, plan::COMPOSE, on_chunk)
}

pub fn respond<F: FnMut(&str) -> Result<()>>(root: &Path, model: &str, messages: &[session::Message], mut on_chunk: F) -> Result<String> {
    let mode = Mode::parse(&std::env::var("ISA_HARNESS_MODE").unwrap_or_default())?;
    let retrieval = retrieval::Retrieval::parse(&std::env::var("ISA_HARNESS_CONTEXT").unwrap_or_default())?;
    let prepared = retrieval::prepare(root, messages, retrieval)?;
    let prepared = super::skills::prepare(root, &prepared, &std::env::var("ISA_SKILLS").unwrap_or_default())?;
    execute(&mut Ollama, model, &prepared, mode, &mut on_chunk)
}

pub fn evaluate(path: &Path, model: &str, mode: &str) -> Result<bool> {
    let parsed = Mode::parse(mode)?;
    let report = evaluation::run(path, model, mode, |prompt| {
        let messages = vec![session::Message::new("user", prompt)];
        execute(&mut Ollama, model, &messages, parsed, &mut |_| Ok(()))
    })?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(report.passed())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    struct Fake { outputs: VecDeque<Option<&'static str>>, calls: Vec<String>, conversations: Vec<Vec<session::Message>> }
    impl Engine for Fake {
        fn run(&mut self, _: &str, messages: &[session::Message], guidance: &str, on_chunk: &mut dyn FnMut(&str) -> Result<()>) -> Result<String> {
            self.calls.push(guidance.into()); self.conversations.push(messages.to_vec());
            match self.outputs.pop_front().expect("unexpected call") {
                Some(text) => { on_chunk(text)?; Ok(text.into()) }, None => Err("fake provider error".into()),
            }
        }
    }
    fn fake(outputs: Vec<Option<&'static str>>) -> Fake { Fake { outputs: outputs.into(), calls: Vec::new(), conversations: Vec::new() } }
    fn task() -> Vec<session::Message> { vec![session::Message::new("user", "Explain the task")] }
    #[test] fn full_pipeline_compares_drafts_and_only_streams_final() {
        let mut engine = fake(vec![Some("draft one"), Some("draft two"), Some("critique"), Some("final")]);
        let mut streamed = String::new();
        let reply = execute(&mut engine, "model", &task(), Mode::Thorough, &mut |s| { streamed.push_str(s); Ok(()) }).unwrap();
        assert_eq!(reply, "final"); assert_eq!(streamed, "final"); assert_eq!(engine.calls, vec![plan::BASELINE, plan::ALTERNATIVE, plan::REVIEW, plan::COMPOSE]);
        let material = &engine.conversations[3].last().unwrap().content;
        assert!(material.contains("draft one") && material.contains("draft two") && material.contains("critique"));
        assert_eq!(engine.conversations[0].len(), 1);
    }
    #[test] fn single_probe_failure_still_composes_and_never_streams_draft() {
        let mut engine = fake(vec![None, Some("draft"), Some("result")]); let mut streamed = String::new();
        assert_eq!(execute(&mut engine, "model", &task(), Mode::Thorough, &mut |s| { streamed.push_str(s); Ok(()) }).unwrap(), "result");
        assert_eq!(streamed, "result"); assert_eq!(engine.calls.len(), 3); assert_eq!(engine.calls[2], plan::COMPOSE);
    }
    #[test] fn failed_probes_never_produce_answer() {
        let mut engine = fake(vec![None, None]);
        assert!(execute(&mut engine, "model", &task(), Mode::Thorough, &mut |_| panic!("must not stream")).is_err());
        assert_eq!(engine.calls.len(), 2);
    }
    #[test] fn failed_review_does_not_prevent_composition() {
        let mut engine = fake(vec![Some("a"), Some("b"), None, Some("final")]);
        assert_eq!(execute(&mut engine, "model", &task(), Mode::Thorough, &mut |_| Ok(())).unwrap(), "final");
    }
    #[test] fn quick_and_balanced_modes_respect_call_budgets() {
        assert!(Mode::parse("unlimited").is_err());
        let mut quick = fake(vec![Some("answer")]);
        assert_eq!(execute(&mut quick, "model", &task(), Mode::Quick, &mut |_| Ok(())).unwrap(), "answer");
        assert_eq!(quick.calls.len(), 1);
        let mut balanced = fake(vec![Some("a"), Some("b"), Some("answer")]);
        assert_eq!(execute(&mut balanced, "model", &task(), Mode::Balanced, &mut |_| Ok(())).unwrap(), "answer");
        assert_eq!(balanced.calls, vec![plan::BASELINE, plan::ALTERNATIVE, plan::COMPOSE]);
    }
    #[test] fn failed_compose_is_an_error() {
        let mut engine = fake(vec![Some("a"), Some("b"), Some("review"), None]);
        assert!(execute(&mut engine, "model", &task(), Mode::Thorough, &mut |_| Ok(())).is_err());
    }
}
