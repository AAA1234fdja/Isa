# ISA response harness

ISA's Rust response harness is inspired by the propose–probe–compose pattern in [Google Research Harness²](https://github.com/google-research/harness2); it is not the upstream implementation or a full agent harness.

For each turn it validates the conversation, proposes two fixed guidance strategies, and probes both using the configured Ollama model. If both probes succeed, the model reviews their omissions, contradictions and unsupported claims. A final model call composes a response from the original conversation, bounded draft excerpts and the optional review. Only the final response is returned or streamed. Probe drafts and the review are passed as untrusted, JSON-encoded data and are not stored in the chat. A failed probe permits the other to continue; if both fail the turn fails. A failed review is skipped; a failed final call fails the turn. The original safety guidance remains in place throughout. Progress and failures (not draft contents) are written to stderr.

This uses up to four sequential response model calls per turn, plus one task-planning call by default and can increase latency and compute significantly. CLI summary generation makes additional calls. Set `ISA_HARNESS=0` for a single model call. The harness does not execute tools, edit files, perform fact verification, use the semantic index, persist lessons, dynamically modify its own instructions, or implement recursive optimization. Model critiques are fallible; none of these checks guarantee correctness or resistance to prompt injection. Drafts are truncated before review and composition. A failed streaming final response can already have displayed partial output in the CLI, but is not saved as an assistant message.

The harness runs for `isa-cli chat`, `isa-cli ask`, and the Rust web server when `ISA_WEB_MODEL` is explicitly configured. The Node preview uses the decoy and does not run the harness. The web server returns the final response only after composition rather than streaming. Stopping the browser request does not yet stop server-side model calls. Conversation and drafts are sent to the local Ollama service. Chat history remains locally stored as described in the main README.

## Modes and workspace context

`ISA_HARNESS_MODE` controls the response call budget: `quick` makes one response call with baseline guidance, `balanced` probes two strategies and composes (up to three response calls), and `thorough` adds the review (up to four response calls, the default). A separate task-planning call runs by default. Invalid values are rejected. These are response strategies, not guarantees of answer quality. `ISA_HARNESS=0` bypasses the harness entirely, including retrieval.

Workspace retrieval is **off by default**. To opt in, set `ISA_HARNESS_CONTEXT=lexical` for the existing lexical context pack or `ISA_HARNESS_CONTEXT=semantic` for the local embedding index (first run `isa-cli semantic-index`). There is no automatic fallback: a retrieval error is reported rather than silently changing methods. At most five excerpts of 600 characters are included, with bounded paths and line ranges when available. Excerpts are JSON encoded and marked as untrusted; this reduces but cannot eliminate prompt injection. No retrieved snippets or probe drafts are stored in the chat history, though the model's answer may quote them.

For CLI commands the workspace is the supplied directory; for the Rust web server it is the server's **current working directory**, not the folder selected in the browser. Enabling retrieval shares matching source excerpts with the local Ollama model service, including every probe and review. Ignore rules and excluded paths apply, but secrets within otherwise indexable files cannot be reliably detected. Do not enable this mode on untrusted server deployments, and review the workspace and `.gitignore` first. Semantic retrieval additionally embeds the query using local Ollama, skips stale chunks, and requires the index model to be installed and unchanged. Retrieval is only for grounding responses; it does not authorize tools, edits or autonomous execution.

Example:

```sh
ISA_HARNESS_MODE=balanced ISA_HARNESS_CONTEXT=semantic isa-cli ask session1 "Where is the HTTP request parsed?" /path/to/workspace
```

## Evaluation

Run an explicitly supplied JSON suite against a local Ollama model without creating or modifying chat sessions:

```sh
ISA_MODEL=llama3.2 isa-cli harness-eval docs/examples/harness-smoke-suite.json quick
ISA_MODEL=llama3.2 isa-cli harness-eval docs/examples/harness-smoke-suite.json thorough
```

The required mode selects the actual harness pipeline regardless of `ISA_HARNESS`. Evaluation deliberately does not retrieve workspace files. Each independent case has a prompt and at least one case-insensitive substring `contains` or `excludes` check, or `max_chars`. The JSON report includes pass/fail, failed check indices and timing, but omits prompts and answers; provider failures are reported without the provider error text. A failed case exits nonzero. Suites are capped at 20 cases and 256 KiB. These simple checks measure only the specific assertions; they cannot establish factuality, safety, semantic quality, or a statistically meaningful improvement. Model outputs and timing can vary across runs, and each case can trigger up to four model calls. Never include secrets in test prompts. The example suite is a minimal wiring check, not a quality benchmark.

Optional user-reviewed skill references can be selected with `ISA_SKILLS`; see [Skills](skills.md). They are disabled by default and are not executable tools.

The harness identifies skill and tool needs before responding. Details, configuration, privacy and approval boundaries are documented in [Skills](skills.md#detecting-needs). Tool suggestions do not execute anything.
