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
