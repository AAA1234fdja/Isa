# ISA response harness

This directory contains ISA's initial Rust response harness. It is **inspired by**, not the Google Harness² implementation. The [Harness² research project](https://github.com/google-research/harness2) uses a broader propose–probe–compose process for harness edits and an improvement library across tasks. ISA currently implements a deliberately smaller response-only version:

1. Propose two fixed, bounded response guidance strategies.
2. Probe both against the same conversation using the configured local Ollama model. Probe output is not printed or saved to the session.
3. Compose the final response using both drafts as untrusted data. Only the composed response is streamed and saved as the assistant response.

The baseline guardrails remain fixed; neither the model nor workspace content can edit the harness, grant permissions, execute tools, or change files. Draft comparison by a model is not a correctness guarantee and may propagate mistakes. The probes do not benchmark real actions, implement recursive improvement, store lessons, or integrate the official Harness² code. They use three sequential model calls, increasing latency and compute. Existing summary generation makes additional calls. If a probe or composition fails, the turn reports the error rather than displaying a draft as a successful answer. Disable the harness with `ISA_HARNESS=0` to use the prior single-call behavior.

This is integrated into `isa-cli chat`, `isa-cli ask`, and the Rust web server when `ISA_WEB_MODEL` is explicitly configured. The Node development preview uses the decoy and does not run the harness. Conversation content and drafts are sent only to the local Ollama endpoint as configured by ISA; stored sessions still contain conversation text.
