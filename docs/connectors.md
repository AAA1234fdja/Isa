# ISA connector authorization

## Current implementation

The Rust CLI provides GitHub device authorization and uses the operating system's credential store for the token and the enabled/disabled setting. It is scoped to the current OS user account, not an ISA cloud profile. On Linux it requires a running Secret Service (such as GNOME Keyring or KWallet); on macOS and Windows it uses the platform credential store. An unavailable credential store fails closed. The client never prints the access token.

Register an ISA GitHub OAuth app with device flow enabled, set its **public client ID** in `ISA_GITHUB_CLIENT_ID` in the environment, then run:

```sh
cargo run --bin isa-cli -- connector github connect
cargo run --bin isa-cli -- connector github status
cargo run --bin isa-cli -- connector github disable
cargo run --bin isa-cli -- connector github enable
cargo run --bin isa-cli -- connector github whoami
cargo run --bin isa-cli -- connector github disconnect
```

During connection, the CLI displays a short-lived device code for entry at GitHub's official verification page. The requested `read:user` permission is for identity only; it does not grant repository or pull-request access. The user must explicitly grant additional permissions in a future implementation before ISA accesses or modifies repositories. Status indicates local credential presence and preference, not a guarantee that GitHub has not revoked access. `whoami` verifies access online. Disconnect removes local credentials; the user must also revoke access in GitHub settings to invalidate the authorization remotely.

The browser UI does **not** yet invoke this CLI flow or read the OS credential store. It does not claim the CLI connection is active. There is no multi-user profile backend, synchronization, or web-accessible token endpoint. Do not expose the local preview server as a public OAuth service.

## Remaining providers

- **Google Drive:** Requires a Google Cloud OAuth app and a desktop OAuth flow with PKCE. Begin with the narrow `drive.file` permission when appropriate; broad Drive permissions are restricted and may require verification.
- **Gmail:** Requires a Google Cloud OAuth app, Gmail API enablement, a desktop PKCE flow, and carefully selected Gmail scopes. Gmail scopes are restricted and may require verification and security assessment. Drive and Gmail authorization must be tracked separately even if the same Google account is used.
- **Notion:** Public integrations use authorization-code OAuth and require a confidential client secret at token exchange. A distributable desktop app cannot safely keep a shared secret; this requires a properly secured authorization backend and registered redirect URI.

No provider secrets, tokens, or authorization codes belong in the repository or browser local storage. Production deployment additionally requires secure account isolation, token refresh and revocation, permission enforcement, auditability, and explicit consent for each action. These providers and the browser connection flow are not implemented yet.

## Official references

- [GitHub device flow](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps)
- [Google OAuth for desktop apps](https://developers.google.com/identity/protocols/oauth2/native-app)
- [Google Workspace data policy](https://developers.google.com/workspace/workspace-api-user-data-developer-policy)
- [Notion authorization](https://developers.notion.com/docs/authorization)
