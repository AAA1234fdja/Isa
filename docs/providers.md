# Chat providers

Settings → Providers lists ISA's built-in Ollama option and lets users add
custom providers. Choose the provider for each conversation in Chat settings.
Auto tries server-configured Ollama first, then the saved custom providers in
stored order if the server is unavailable. Ollama requires `ISA_WEB_MODEL` on
the Rust server. With no usable provider the request returns an error.

Custom providers use the OpenAI-compatible chat-completions format; streaming
is optional and off by default. Enter the **full** public HTTPS endpoint (for
example, `https://example.com/v1/chat/completions`), a model identifier, a
display name, and optionally a bearer API key. URLs containing credentials,
query parameters, fragments, localhost, local hostnames, or IP literals are
rejected. Redirects are rejected. Providers with other protocols, custom
headers or query-string authentication are not supported. Except for the
xKiro bridge described below, endpoints must permit browser CORS. Image attachments use OpenAI-compatible image URLs and
require a vision-capable model; extracted document text is sent as untrusted
reference data. Follow the provider's terms, privacy policy, and model limits.

Except for the xKiro endpoint described below, requests to a custom provider go
**directly from the user's browser** to the configured endpoint with
`credentials: omit`, no referrer, and no redirects. The server and its Rust
harness, plugins, and Ollama routing are bypassed; the browser must be able to
reach the provider and its CORS policy must allow the request. This means custom-provider requests do not receive the automatic
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

Additional endpoint references: [Baseten](https://docs.baseten.co/reference/inference-api/chat-completions),
[Hugging Face](https://huggingface.co/docs/inference-providers/en/guides/gpt-oss),
[Nscale](https://docs.nscale.com/docs/use-cases/chat), and
[Scaleway](https://www.scaleway.com/en/developers/api/generative-apis).
These are suggestions, not connectivity or CORS guarantees. Some services require
account-specific, regional, deployment-specific or non-compatible endpoints;
those entries intentionally do not have a universal URL.

Directory references: [LiteLLM provider list](https://github.com/BerriAI/litellm#supported-providers)
and [AI inference providers comparison](https://infrabase.ai/blog/ai-inference-api-providers-compared).
Provider names and API capabilities can change; recheck official documentation
before configuring a provider.

The directory now includes xKiro and offers editable URL suggestions for
providers with documented public Chat Completions endpoints. The full model ID
and any API key must still be supplied, and browser CORS support has not been
verified. The exact xKiro endpoint uses the experimental local bridge below.
No endpoint is auto-connected. Saving a provider or creating an Agent
Pool does not test connectivity: the first request can still fail. For xKiro,
use `https://api.xkiro.com/v1/chat/completions` as the full endpoint and verify
the model ID against its current catalog. If a request fails with the generic
network/CORS message, check the browser or WebView developer tools for the
failed request and CORS details; network failures, rejected redirects, and
CORS can produce the same message. Do not share API keys, Authorization headers,
or unredacted network exports when requesting help. Do not disable browser
security to work around the error. Services without a fixed, documented
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
shown in expandable Agent pool details and stored locally with the chat. Subagent
instructions and configured `min_tokens` and `reasoning_effort` are sent to the
provider. In **Settings → Providers**, choose whether the subagent's maximum
output setting uses `max_tokens` (the default) or `max_completion_tokens`. A
separate optional **Maximum reasoning tokens** value can be set on each subagent;
it is sent as `max_reasoning_tokens` only if the provider's explicit support
switch is enabled. A configured value with the switch off fails the pool rather
than being ignored. Existing providers and workflows default to the previous
behavior; blank values are omitted. These options apply to pool subagents, not
direct chat or pool agents.

Select these parameters only when the provider and model support them. ISA does
not implement reasoning-token accounting or guarantee separate budgets: either
output field may include reasoning tokens, and the provider may reject or ignore
the parameters or combinations. No fallback retries change the requested fields.
If the provider returns reasoning but no final content,
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
requests have a 120-second browser timeout per attempt; other custom-provider
requests retain 45 seconds. The pool retries transient network errors, timeouts,
HTTP 408, 429 and 5xx at most twice per model request, with exponential delay
and jitter. `Retry-After` is honored when visible to the browser; if it exceeds
five seconds ISA does not retry. A CORS-hidden header cannot be honored.
Authentication, configuration, invalid output and other non-transient failures
are not retried. Retries may duplicate remote work and charges even after a
browser timeout or cancellation; usage for failed attempts may be unknown.

After transient retries are exhausted, an unavailable delegated branch is
reported to its parent as missing, and a parallel group can continue with its
remaining successful members. The final answer is explicitly marked partial and
pool details record the failure; missing work is not presented as completed.
If all agents in a group fail, its coordinator fails, a non-transient failure
occurs, or final synthesis fails, the pool may still fail. Cancellation aborts
browser requests and retry waits but may not stop remote computation. Pool
can optionally stream the final pool response with OpenAI-compatible SSE: enable
**Stream final pool response** in the specific provider settings only when that
provider and model support it. It is off by default. Planning, intermediate agents, goal/summary helpers and final responses from a
parallel group with only one surviving agent remain non-streaming. Direct chat
can separately opt in using **Stream ordinary chat responses** in the selected
provider settings; this does not enable streaming for Auto fallback or helpers. Streaming requires
normal `stop` and `[DONE]` markers; truncated, malformed or unsupported streams
fail rather than becoming completed answers. Provisional text is displayed only
in the active chat and is never saved on interruption, Stop or failure. ISA does
not retry a stream once visible text was received. Provider usage may be missing
from streamed responses, and CORS must permit SSE. Direct chat requests go from
the browser to the provider; Stop aborts the browser request but does not
guarantee the remote provider stops computation. This has been tested with
simulated providers, not real provider endpoints or Rust server streaming.

While a pool is running, the chat displays live, transient status for provider
requests, retries and pending request count. Status is per chat, not a model
response, and is cleared on completion or Stop. Parallel events can arrive in
any order. The provider may continue working after a browser cancellation.

Pool agents do not invoke tools, plugins, or the Rust harness, and the runner does
not provide sandboxing or autonomous file editing. Optionally, a chat can set a
**Pool symbol context** query: after explicit per-message confirmation, ISA
asks the configured Rust server to find matching declarations or syntactic call
sites (selected in chat settings) and sends up to 15 names, paths and line numbers
to pool providers as untrusted context. Declaration matching is partial and
case-insensitive; call-site matching is exact and case-sensitive. Call sites are
unresolved: identical names may refer to unrelated functions, and dynamic calls
may be missed. The lookup is bounded and may be incomplete; failure stops the pool.
It requires `ISA_WEB_AGENT_WORKSPACE`, not the browser folder picker, and is not
used by goal or context-compression helpers. Results may expose sensitive project
information. The same-origin endpoint is a development feature, not a security
boundary against untrusted users of the server. A pool may make up to 16 logical model
requests per message, with up to three attempts each, can incur charges, and sends the chat history, message,
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
and `node tests/browser-context-errors.cjs`. The dependency-free runtime regressions can be run with `node tests/pool-runtime.cjs` and `node tests/pool-resilience.cjs` and `node tests/pool-stream.cjs`. The ordinary-chat streaming UI regression uses `node tests/chat-stream-ui.cjs`. The provider/workflow form regression uses jsdom and can be run with `node tests/provider-budget-ui.cjs` (install jsdom outside the repository and set `JSDOM_PATH` if needed). Set `PLAYWRIGHT_CORE`, `CHROMIUM_PATH`,
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

### Directory entries without a universal URL

The setup form accepts public HTTPS Chat Completions URLs and bearer tokens,
not arbitrary native APIs. These examples are documentation, not values that
can be submitted unchanged. CORS and model access are not verified.

- [Amazon Bedrock](https://docs.aws.amazon.com/bedrock/latest/userguide/inference-chat-completions-mantle.html): regional endpoint and Bedrock API key or SigV4; ISA does not sign AWS requests.
- [Azure AI Foundry](https://learn.microsoft.com/en-us/azure/foundry/openai/how-to/reasoning): URL depends on the resource and deployment; authentication varies.
- [Cloudflare Workers AI](https://developers.cloudflare.com/workers-ai/configuration/open-ai-compatibility/): URL requires an account ID.
- [Databricks](https://docs.databricks.com/gcp/en/agents/custom-agents/query-agent): URL requires a workspace and a compatible serving endpoint.
- [Deepgram](https://developers.deepgram.com/docs/voice-agent-llm-models): its voice agent can call an OpenAI-compatible LLM; that does not establish a Deepgram Chat Completions URL for ISA.
- [Google Vertex AI](https://docs.cloud.google.com/vertex-ai/generative-ai/docs/migrate/openai/examples): URL needs a project and location; uses an access token.
- [IBM watsonx.ai](https://cloud.ibm.com/apidocs/watsonx-ai-cp/watsonx-ai-cp-2.1.2): native chat uses a version query and project ID; not supported by ISA's custom client.
- [Lepton AI](https://blog.lepton.ai/structural-decoding-function-calling-for-all-open-llms-a8823243b67c): documented examples use model/deployment-specific hosts; verify current service details.
- [LM Studio](https://lmstudio.ai/docs/developer/openai-compat): normally a local HTTP endpoint, which the custom-provider URL validator deliberately rejects.
- [Modal](https://modal.com/docs/examples/vllm_inference): URL depends on the deployed service and its implementation.
- [Oracle Cloud Infrastructure](https://docs.oracle.com/en-us/iaas/Content/generative-ai/openai-compatible-api.htm): endpoint and credentials depend on region and OCI configuration; native request signing is not supported by ISA.
- [Predibase](https://docs.predibase.com/api-reference/inference-api/create-chat-completion): URL includes tenant and deployment names.
- [Replicate](https://replicate.com/docs/topics/predictions/streaming): documented prediction/streaming protocol differs from ISA's Chat Completions client; a compatible universal URL was not verified.
- [RunPod](https://docs.runpod.io/migrate/replicate): compatible URL is specific to a public endpoint or deployment.
- [Snowflake Cortex](https://docs.snowflake.com/en/user-guide/snowflake-cortex/cortex-rest-api): URL includes the Snowflake account identifier.

Other newly suggested endpoints: [AI21](https://docs.ai21.com/reference/jamba-1-6-api-ref),
[FriendliAI](https://friendli.ai/docs/guides/openai-compatibility),
[Hyperbolic](https://www.hyperbolic.ai/docs/inference/text-apis),
[Infermatic](https://infermatic.ai/docs/overview/),
[Jina DeepSearch](https://github.com/jina-ai/node-deepresearch),
[Ollama Cloud](https://docs.ollama.com/api/introduction),
[Perplexity](https://docs.perplexity.ai/guides/chat-completions-sdk), and
[Vercel AI Gateway](https://vercel.com/docs/ai-gateway/sdks-and-apis/openai-chat-completions).
Ollama Cloud is a separate custom-provider option; it does not replace the
server-configured local Ollama integration. Jina's suggestion is specifically
for DeepSearch. Provider feature and streaming support vary by model.

### Experimental xKiro bridge

The exact `https://api.xkiro.com/v1/chat/completions` URL is routed through
`/api/providers/xkiro/chat/completions` on the Rust server for chat and pools.
The bridge accepts only direct IPv4 loopback, same-origin JSON requests at the
server's exact loopback address, and sends only the JSON body and optional
bearer authorization to the fixed HTTPS destination. It rejects redirects,
proxy environment settings, and non-public resolved addresses and pins checked
DNS addresses. It does not log the request or bearer key. The browser still
holds the key in session storage; local processes and scripts on the origin are
not isolated from it. Only this exact endpoint is bridged; other providers stay
direct. The Node preview does not implement the bridge. Remote/LAN origins are
not supported by this route.

The bridge forwards JSON and SSE with an 8 MiB response cap and a five-minute
timeout. A browser abort closes its connection; the server can detect it on a
subsequent write, but upstream computation may continue until then or until the
timeout. Provider HTTP errors are reported without their bodies and Retry-After
is forwarded when safe. A provider outage, invalid key or unsupported model can
still fail. This code has not yet been compiled or tested end-to-end with a
real xKiro account or Windows WebView; it should be treated as experimental.
