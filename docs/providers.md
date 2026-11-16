# Chat providers

Settings → Providers lists ISA's built-in Ollama option and lets users add
custom providers. Choose the provider for each conversation in Chat settings.
Automatic selects the server-configured Ollama model, not a custom provider.
Ollama requires `ISA_WEB_MODEL` on the Rust server; otherwise the decoy
returns a no-provider error.

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
Agents lists saved custom providers and can start a new chat with one selected;
it does not create or execute autonomous agents.

Preset reference pages (checked 2026-11-16):

- [xKiro](https://docs.xkiro.com/api/overview/), [OpenAI](https://platform.openai.com/docs/guides/migrate-to-responses), [Anthropic](https://platform.claude.com/docs/en/cli-sdks-libraries/libraries/openai-sdk), [Google Gemini](https://ai.google.dev/gemini-api/docs/openai)
- [Cerebras](https://inference-docs.cerebras.ai/resources/openai), [Cohere](https://docs.cohere.com/docs/compatibility-api), [DeepInfra](https://docs.deepinfra.com/chat/overview), [DeepSeek](https://api-docs.deepseek.com/), [Fireworks AI](https://docs.fireworks.ai/nexus/apis-and-sdks), [Groq](https://console.groq.com/docs/api-reference)
- [Inception](https://docs.inceptionlabs.ai/get-started), [Mistral AI](https://docs.mistral.ai/api), [Moonshot AI](https://platform.moonshot.ai/docs/api/chat.en-US), [Nebius AI Studio](https://docs.nebius.com/studio/inference/integrations/postman), [Novita AI](https://novita.ai/docs/guides/llm-api), [NVIDIA NIM](https://docs.api.nvidia.com/nim/reference/google-codegemma-7b-infer), [OpenRouter](https://openrouter.ai/docs/api_reference/overview)
- [SambaNova](https://docs.sambanova.ai/cloud/docs/integrations/vscode), [SiliconFlow](https://docs.siliconflow.com/en/usercases/use-siliconcloud-in-immersive-translate), [Together AI](https://docs.together.ai/docs/chat-overview), [Upstage](https://developers.upstage.ai/docs/apis/chat), [xAI](https://docs.x.ai/developers/model-capabilities/legacy/chat-completions), [Z.ai](https://docs.z.ai/guides/llm/glm-4.6), [Alibaba Cloud Model Studio](https://www.alibabacloud.com/help/tc/model-studio/qwen-api-via-openai-chat-completions).
