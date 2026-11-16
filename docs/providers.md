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
