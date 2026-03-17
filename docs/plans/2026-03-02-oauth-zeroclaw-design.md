# LearnForge OAuth via Zeroclaw Native

**Date:** 2026-03-02
**Status:** Approved
**Goal:** Let users sign in with their OpenAI/Google accounts (existing subscriptions) instead of pasting API keys

---

## Architecture

Two new Tauri commands:

1. **`start_oauth_login(provider: "openai" | "gemini")`** — Calls zeroclaw's `build_authorize_url()` with PKCE, opens the system browser via Tauri shell, starts the loopback TCP listener in a background task. When the callback arrives, exchanges the code for tokens via zeroclaw, stores the token set in zeroclaw's `AuthProfilesStore` (encrypted), and copies the `oauth_token` to LearnForge's `AuthState` credentials.json.

2. **`check_oauth_status(provider: string)`** — Returns whether OAuth completed for that provider (checks if `oauth_token` is now stored in `AuthState`). Frontend polls this every 2 seconds after triggering login.

Token refresh is handled internally by zeroclaw when `ai_request()` calls provider methods. No new refresh command needed.

## Settings UI Changes

For OpenAI and Gemini cards: add "Sign in with OpenAI" / "Sign in with Google" button alongside existing BYOK option. Flow:
- Button click -> spinner "Waiting for browser..."
- Poll `check_oauth_status` every 2s
- On success: refresh auth status, show "Connected"
- On timeout (60s): show "Try again" with BYOK fallback

Anthropic stays BYOK-only (no OAuth available from Anthropic's side).

## Token Storage

Dual storage:
- **zeroclaw's `auth-profiles.json`**: Encrypted at rest, handles token refresh with expiry tracking
- **LearnForge's `credentials.json`**: References the oauth_token for `ai_request()` routing

When OAuth succeeds, write to both stores.

## Data Flow

```
User clicks "Sign in with OpenAI"
  -> start_oauth_login("openai")
  -> zeroclaw builds PKCE authorize URL
  -> tauri::shell::open(url) opens system browser
  -> zeroclaw TCP listener on localhost:1455 (1456 for Gemini)
  -> user authenticates in browser, redirected back
  -> zeroclaw receives callback, exchanges code for tokens
  -> tokens stored in zeroclaw AuthProfilesStore (encrypted)
  -> oauth_token copied to LearnForge AuthState credentials.json
  -> frontend polls check_oauth_status, sees authenticated
  -> ai_request() uses oauth_token as bearer token
  -> zeroclaw auto-refreshes when token expires
```

## Error Handling

- **Port in use**: Fall back to zeroclaw's stdin/manual code entry
- **User closes browser**: 60s timeout, frontend shows "Try again"
- **Token refresh fails**: zeroclaw retries with exponential backoff (3 attempts). On failure, ai_request() errors, user re-authenticates from Settings
- **Gemini client ID/secret**: Embed defaults or document setup requirement

## Testing

- Unit test: `start_oauth_login` returns without error
- Unit test: `check_oauth_status` returns false when no token, true when present
- Integration: manual browser test (OAuth can't be automated)
- All existing Rust + React tests must pass

## Not Included

- Anthropic OAuth (doesn't exist)
- Token display in UI
- Multiple accounts per provider
- Automatic re-auth on expiry from UI
