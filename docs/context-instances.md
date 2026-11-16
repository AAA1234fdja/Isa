# Context instances and utility commands

## Utility commands

`./help/.` displays current usage and status without adding a message to the conversation. `./resume-AC/.` displays the all-chat summary, and `./resume-LP/.` displays the last-prompt summary. The commands do not modify conversation history. In the browser these commands open an informational window; the browser is not yet connected to CLI sessions, so it cannot display their saved summaries.

## Summary instances

- **AC (All Chat):** A 200–300 word factual overview of the project and conversation, including supported decisions, progress, and pending work.
- **LP (Last Prompt):** A 100–200 word factual summary of the most recent user prompt and its assistant response.

After each completed Ollama response in the Rust CLI, ISA requests both summaries from the same model and saves valid results in `.isa/summaries/<session-id>-AC.md` and `.isa/summaries/<session-id>-LP.md`. The files are local and ignored by Git. Summary generation requires two additional model requests and can take time. A summary is saved only if it meets its word-count range; otherwise the previous file is retained and an error is printed. The conversation response is still saved when a summary update fails. Generated summaries should be reviewed for accuracy.

Input is bounded to recent conversation content; AC also receives its previous summary when available. Consequently, AC is a rolling approximation, not a guarantee that every older detail is included. The LP instance uses the most recent user message and following response. Content is untrusted data, not a source of instructions. The browser and CLI currently use separate session stores.

## Code style instruction

The model receives a system instruction to favor clean, maintainable code and avoid unnecessary notes and comments while retaining comments that explain important intent or safety constraints. This is guidance, not an enforcement mechanism.


## Browser context compression and goal planning

The browser supports `./resume/.` for manually compressing older chat messages.
It also requests a summary automatically before sending if the unsummarized text
exceeds 48,000 UTF-8 bytes (roughly 12,000 tokens, not a model-tokenizer count)
or 18 messages. These are fixed application limits, not the model's actual context
window. The application controls compression; the model cannot invoke this
command or other tools itself. The selected chat route performs the summary, so
a pool can make multiple calls and costs may increase. Existing multi-provider
consent applies. Only text saved with messages and available extracted text are
included, not prior images or other ephemeral attachments.

A bounded rolling summary is saved locally with its covered message index and
included as untrusted reference in subsequent requests. Original messages are not
deleted. Summaries may omit or distort information and should not be treated as
a verified record. Each summary call handles a bounded prefix; if a single
message is too large, a provider fails, or safe limits are exceeded, the request
fails rather than silently discarding unsummarized context. Compression does not
work while another request in the same chat is running. CLI AC/LP summaries
remain separate from the browser summary.

Use `./goal/.` or `./goal/. objective` in the browser to open a goal dialog and
generate a plan in Task overview. The selected provider or pool generates the
plan; a failure is shown instead of inventing one. The plan is stored locally,
requires human review and does not execute tools or tasks. This browser plan is
not a CLI goal record and does not enforce an execution workflow.
