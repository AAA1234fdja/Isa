# Chat providers

Settings → Providers lists ISA's built-in Ollama option and lets users add
custom providers. Choose the provider for each conversation in Chat settings.
Auto tries server-configured Ollama first, then the saved custom providers in
stored order if the server is unavailable. Ollama requires `ISA_WEB_MODEL` on
the Rust server. With no usable provider the request returns an error.

Custom providers use the OpenAI-compatible, non-streaming chat-completions
request and response format. Enter the **full** public HTTPS endpoint (for
example, `https://example.com/v1/chat/completions`), a model identifier, a
display name, and optionally a bearer API key. URLs containing credentials,
query parameters, fragments, localhost, local hostnames, or IP literals are
rejected. Redirects are rejected. Providers with other protocols, custom
headers, query-string authentication, or endpoints that do not permit browser
CORS are not supported. Image attachments use OpenAI-compatible image URLs and
require a vision-capable model; extracted document text is sent as untrusted
reference data. Follow the provider's terms, privacy policy, and model limits.

Requests to a custom provider go **directly from the user's browser** to the
configured endpoint with `credentials: omit`, no referrer, and no redirects.
The server and its Rust harness, plugins, and Ollama routing are bypassed;
the browser must be able to reach the provider and its CORS policy must allow
the request. This means custom-provider requests do not receive the automatic
plugin skill excerpts currently implemented for the Rust/Ollama path. Stopping
a request aborts the browser request but cannot guarantee remote computation
has stopped.

Names, endpoint URLs, and model identifiers are stored in localStorage. API
keys are stored separately in unencrypted sessionStorage for the current tab,
not in the chat history or chat export. Browsers may restore tab sessions.
Scripts running on this origin can read the key; this is not an OS credential
vault. Avoid shared browsers and untrusted scripts, and remove the key or close
the session when finished. Editing a provider to use a different endpoint
clears the old key unless a replacement is entered. Deleting a provider clears
its key in the current tab; it cannot clear separate tabs or revoke remote
credentials. Rotate or revoke a key at the provider if needed. If a saved
chat references a deleted provider, it reports an error instead of silently
falling back to another one.

## Provider directory

Settings contains a static alphabetical directory of 52 commonly encountered
AI providers and platforms, for discovery rather than a verified popularity
ranking. Eight rows are visible at a time; scroll the directory for the rest.
The list does not assert that the provider is connected, that its native API is
supported by ISA, or that it permits browser CORS. “Set up” opens the custom-provider form and may pre-fill a documented
endpoint. Users must verify the URL and supply a model and any needed key.
Ollama is the sole built-in option currently.

Directory references: [LiteLLM provider list](https://github.com/BerriAI/litellm#supported-providers)
and [AI inference providers comparison](https://infrabase.ai/blog/ai-inference-api-providers-compared).
Provider names and API capabilities can change; recheck official documentation
before configuring a provider.

The directory now includes xKiro and offers editable URL suggestions for
providers with documented public Chat Completions endpoints. The full model ID
and any API key must still be supplied, and browser CORS support has not been
verified. No endpoint is auto-connected. Services without a fixed, documented
compatible endpoint still require a manually supplied URL. Region-specific or
account-specific endpoints may differ from a suggested default. Set up always creates a new configuration; each saved configuration must have a
unique name (case-insensitive). Edit existing configurations in the saved list.
Claude is listed separately for discovery and uses Anthropic’s OpenAI-compatible
endpoint. Multiple configurations may use the same endpoint and different models.
Agents lists saved custom providers and can start a new chat with one selected.
Pools run bounded model workflows in the browser, not autonomous tool-using agents.

Preset reference pages (checked 2026-11-16):

- [xKiro](https://docs.xkiro.com/api/overview/), [OpenAI](https://platform.openai.com/docs/guides/migrate-to-responses), [Anthropic](https://platform.claude.com/docs/en/cli-sdks-libraries/libraries/openai-sdk), [Google Gemini](https://ai.google.dev/gemini-api/docs/openai)
- [Cerebras](https://inference-docs.cerebras.ai/resources/openai), [Cohere](https://docs.cohere.com/docs/compatibility-api), [DeepInfra](https://docs.deepinfra.com/chat/overview), [DeepSeek](https://api-docs.deepseek.com/), [Fireworks AI](https://docs.fireworks.ai/nexus/apis-and-sdks), [Groq](https://console.groq.com/docs/api-reference)
- [Inception](https://docs.inceptionlabs.ai/get-started), [Mistral AI](https://docs.mistral.ai/api), [Moonshot AI](https://platform.moonshot.ai/docs/api/chat.en-US), [Nebius AI Studio](https://docs.nebius.com/studio/inference/integrations/postman), [Novita AI](https://novita.ai/docs/guides/llm-api), [NVIDIA NIM](https://docs.api.nvidia.com/nim/reference/google-codegemma-7b-infer), [OpenRouter](https://openrouter.ai/docs/api_reference/overview)
- [SambaNova](https://docs.sambanova.ai/cloud/docs/integrations/vscode), [SiliconFlow](https://docs.siliconflow.com/en/usercases/use-siliconcloud-in-immersive-translate), [Together AI](https://docs.together.ai/docs/chat-overview), [Upstage](https://developers.upstage.ai/docs/apis/chat), [xAI](https://docs.x.ai/developers/model-capabilities/legacy/chat-completions), [Z.ai](https://docs.z.ai/guides/llm/glm-4.6), [Alibaba Cloud Model Studio](https://www.alibabacloud.com/help/tc/model-studio/qwen-api-via-openai-chat-completions).

## Agents and pools

In Agents, select a saved provider to edit its system prompt or delete the
provider. The prompt is saved in localStorage, is not secret, and is sent as a
system message on every direct custom-provider chat request using that provider,
including existing conversations. The provider's model must support system
messages. Editing provider settings preserves the prompt. Deletion makes saved
chat references unavailable; pool members are shown as unavailable until removed.

The provider list scrolls after ten rows. Agents Pool lets you create, rename,
delete and edit local teams. Edit workflow opens a scrollable diagram with
movable agent and subagent nodes. Choose a team provider and a shape in the
palette, then left-click the map to place it. Select a node to edit its
properties; drag the node or its grip to move it. Select it and use the
remove button to delete the node and its connections (the provider remains in
the team). Use the mouse wheel or zoom buttons to
zoom, and right-drag the map to pan. Each team provider starts with an agent
node. A workflow can have at most one agent node per provider, while multiple
subagent nodes may use the same provider. A removed agent can be placed again.
Older duplicate agent nodes are merged on opening; connections are redirected
where possible. Invalid, redundant or cyclic connections are dropped from the
working copy and reported before saving. Cancel leaves the original map intact. Agent system prompts are shared with direct provider chats
and saved when the workflow is saved. Subagent instructions, minimum and maximum
tokens, and reasoning effort are stored per node. Blank values mean inherit the
provider/model defaults; ISA does not currently set token or effort defaults,
and supported parameters vary by provider.

Use the arrow tools and select two nodes: a one-way arrow records management or
delegation from the first to the second, while a two-way arrow records parallel
work and intended synthesis. The connection list removes arrows. Duplicate,
self-referential and cyclic management connections are rejected. Previously
saved management connections are migrated into the diagram when it opens.
Removing a team member removes its nodes and connections. Cancel discards changes.
Pools and workflows are stored in this browser and are not included in chat
exports. Choose a pool using the composer or chat settings to run it. The browser
runner requires 1–8 nodes and one connected root (an agent or parallel group),
with no cycles after parallel groups are combined, at most one manager per
node or parallel group, and at least one agent in the root group. A manager first generates a JSON plan assigning a specific
task to each directly managed group. Subagents are optional: the coordinator
selects them only if the user requests them or the model judges them necessary;
it is instructed to honor requests not to use them. This model judgment is
fallible, not a hard guarantee of necessity. A missing or invalid selection fails
closed instead of starting subagents. An unselected subagent makes no model
request and cannot delegate; a group containing only unselected subagents is
skipped along with its descendants. An invalid or incomplete plan fails the run
before its children start. Children execute their assigned tasks concurrently;
the manager then reviews their responses and completes its own task. For a
parallel group, active members respond concurrently and its first active agent (or
first active subagent when there is no agent) compares
the responses for correctness, completeness and relevance, reconciles differences,
and synthesizes an answer. If a parallel group has subordinates, its first active agent (or first active
subagent) also coordinates delegation for that group. Only active nodes can
delegate along their outgoing arrows. Agent output is supplied to
subsequent models as untrusted reference data. The comparison is a model response,
not a guaranteed objective evaluation. Delegated tasks and parallel responses are
shown in expandable Agent pool details and stored locally with the chat. Subagent instructions and configured
`min_tokens`, `max_tokens`, and `reasoning_effort` are sent to the provider;
unsupported parameters may cause errors. Blank values are omitted. ISA does not
independently control the reasoning-token budget: providers may count reasoning
against `max_tokens`. If the provider returns reasoning but no final content,
ISA displays a specific error without storing the reasoning, and includes any
reported usage. There is no automatic retry or guarantee that a higher limit
will work.

Pool delegation requests can optionally use OpenAI-compatible strict JSON Schema
via the **Request strict JSON Schema for pool delegation** checkbox in each
custom provider's settings. It defaults off; enable it only for a provider and
model that support this exact API. ISA still validates task and subagent IDs,
and rejects unsupported or invalid responses without silently retrying. This
requests a provider feature; ISA does not implement constrained decoding.

Intermediate agent-to-agent answers are instructed to be concise while retaining
important findings and uncertainty. To avoid silently discarding material, an
intermediate answer exceeding 8,000 characters fails the pool instead of being
truncated; a pool response reported by the provider as stopped by its output
limit is also rejected. The final user-facing answer is not subject to the
intermediate character limit. Conciseness and token or latency reductions are not guaranteed. Pool
requests have a 120-second browser timeout; other custom-provider requests retain
45 seconds. A failed node fails the pool; cancellation aborts browser requests
but may not stop remote computation.

The runner does not invoke tools, plugins, or the Rust harness, and does not
provide sandboxing or autonomous file editing. A pool may make up to 16 model
requests per message, can incur charges, and sends the chat history, message,
attachments, and intermediate responses to its participating providers. Auto
may also send the same data to multiple providers during fallback. ISA asks for
confirmation before each such send and lists the destinations. Auto retries
custom providers after network errors, timeout, HTTP 401, 403, 404, 408, 429 and
5xx; other errors stop fallback. A timeout or network error cannot prove the
previous provider did not receive the request. Provider keys remain tab-local.

The microphone uses the browser's Web Speech API, when available, with browser
permission. Speech recognition may send audio to the browser's recognition
service; ISA asks before starting. Dictation inserts text into the composer and
does not automatically send it. Real recognition support depends on browser,
platform, permissions, and network.

Browser routing regression tests (mocked providers, no real API calls): start
`dev/preview.mjs`, install `playwright-core` and Chromium outside the repository,
then run `node tests/browser-routing.cjs` and
`node tests/browser-routing-errors.cjs`, `node tests/browser-pool-workflow.cjs`,
`node tests/browser-subagent-on-demand.cjs`, `node tests/browser-context-tokens.cjs`,
and `node tests/browser-context-errors.cjs`. The dependency-free runtime regression can be run with `node tests/pool-runtime.cjs`. Set `PLAYWRIGHT_CORE`, `CHROMIUM_PATH`,
and optionally `ISA_TEST_URL` to match the local installation.


## Token reporting

The composer Tokens panel shows per-conversation input, output, cached and total
counts summed from OpenAI-compatible `usage` objects returned by successful
custom-provider requests and Ollama chat counters reported by the Rust server. Pool planning, delegation, parallel, comparison, goal
and compression calls are counted individually when their providers report
usage. Cached tokens are part of input, not an additional cost. Unknown fields
are shown as unavailable or incomplete; failures, cancellations and providers
that do not return usage may still incur costs. The Rust server reports each
completed Ollama chat call, including planner, harness and read-only loop calls,
when Ollama supplies both counters; missing counters and failed internal calls
are recorded as unknown. Embedding requests are not counted, and a failed or
cancelled server request cannot return any of its partial usage. Ollama does not
report cached counts here. Historical conversations have no retroactive usage.
Totals may therefore be incomplete.

Context is a rough estimate of currently selected conversation text, using UTF-8
bytes divided by four; it excludes system prompts, attachments, pool instructions
and model-specific formatting, and is not a context-window measurement. The
daily chart sums reported total tokens by local date: no reported usage is gray,
then positive totals up to 100K, over 100K–1M, over 1M–10M, over 10M–200M, over
200M–1B and over 1B use levels 1–6 respectively. A day with unreported calls
may be undercounted. Counts, summaries, plans and messages are stored locally
in the browser; they are not billing records.
