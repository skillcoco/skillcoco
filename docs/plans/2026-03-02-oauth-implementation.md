# OAuth via Zeroclaw Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Let users sign in with OpenAI/Google OAuth so they can use their existing subscriptions without API keys.

**Architecture:** Make zeroclaw's `auth` module public, then add two Tauri async commands (`start_oauth_login`, `check_oauth_status`) that call zeroclaw's OAuth functions (PKCE, loopback listener, token exchange). Frontend adds "Sign in" buttons to Settings that open system browser and poll for completion.

**Tech Stack:** Rust (zeroclaw auth, Tauri commands, tokio), React + TypeScript (Zustand, Lucide), Vitest

---

### Task 1: Make zeroclaw auth module public

**Files:**
- Modify: `/Users/gshah/work/agentix/upstream/zeroclaw/src/lib.rs:44`

**Step 1: Change auth visibility**

In `/Users/gshah/work/agentix/upstream/zeroclaw/src/lib.rs`, line 44:

```rust
// Change from:
pub(crate) mod auth;
// To:
pub mod auth;
```

**Step 2: Verify zeroclaw compiles**

Run: `cd /Users/gshah/work/agentix/upstream/zeroclaw && cargo check 2>&1 | tail -10`
Expected: Compiles (warnings OK, no errors)

**Step 3: Verify SkillCoco still compiles**

Run: `cd /Users/gshah/work/apps/skillcoco/skillcoco/src-tauri && cargo check 2>&1 | tail -10`
Expected: Compiles

**Step 4: Commit zeroclaw change**

```bash
cd /Users/gshah/work/agentix/upstream/zeroclaw
git add src/lib.rs
git commit -m "feat: make auth module public for downstream crate access"
```

---

### Task 2: Add OAuthState and start_oauth_login Tauri command

**Files:**
- Create: `src-tauri/src/auth/oauth.rs`
- Modify: `src-tauri/src/auth/mod.rs` (add `pub mod oauth;`)
- Modify: `src-tauri/src/lib.rs` (register command, add state)

This task adds the core OAuth flow: generate PKCE, build authorize URL, open browser, spawn background listener, exchange code for tokens, store in AuthState.

**Step 1: Add oauth module declaration**

In `src-tauri/src/auth/mod.rs`, add at line 1:

```rust
pub mod oauth;
```

**Step 2: Create the OAuth module**

Create `src-tauri/src/auth/oauth.rs`:

```rust
use crate::auth::{AuthMethod, AuthState, ProviderCredential};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use std::collections::HashMap;
use std::time::Duration;
use tauri::State;
use zeroclaw::auth::oauth_common::generate_pkce_state;
use zeroclaw::auth::openai_oauth;
use zeroclaw::auth::profiles::TokenSet;

/// Tracks in-flight OAuth flows.
pub struct OAuthFlowState {
    /// Maps provider -> whether OAuth completed successfully
    pub completed: Mutex<HashMap<String, bool>>,
}

impl OAuthFlowState {
    pub fn new() -> Self {
        Self {
            completed: Mutex::new(HashMap::new()),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct OAuthStartResult {
    pub started: bool,
    pub provider: String,
}

#[derive(Debug, Serialize)]
pub struct OAuthStatusResult {
    pub completed: bool,
    pub provider: String,
    pub authenticated: bool,
}

/// Start an OAuth login flow for the given provider.
/// Opens the system browser and spawns a background listener for the callback.
#[tauri::command]
pub async fn start_oauth_login(
    app: tauri::AppHandle,
    auth: State<'_, AuthState>,
    flow: State<'_, OAuthFlowState>,
    provider: String,
) -> Result<OAuthStartResult, String> {
    // Mark flow as not completed
    {
        let mut completed = flow.completed.lock().map_err(|e| e.to_string())?;
        completed.insert(provider.clone(), false);
    }

    match provider.as_str() {
        "openai" => start_openai_oauth(app, auth.inner().clone(), flow.inner().clone(), provider.clone()).await,
        "gemini" => Err("Gemini OAuth requires GEMINI_OAUTH_CLIENT_ID and GEMINI_OAUTH_CLIENT_SECRET environment variables. Set them and try again.".to_string()),
        _ => Err(format!("OAuth not supported for provider: {}", provider)),
    }
}

async fn start_openai_oauth(
    app: tauri::AppHandle,
    auth: AuthState,
    flow: OAuthFlowState,
    provider: String,
) -> Result<OAuthStartResult, String> {
    let pkce = generate_pkce_state();
    let url = openai_oauth::build_authorize_url(&pkce);

    // Open system browser
    tauri::async_runtime::spawn(async move {
        // Open URL in default browser
        let _ = open::that(&url);

        // Wait for callback on localhost:1455
        let code_result = openai_oauth::receive_loopback_code(
            &pkce.state,
            Duration::from_secs(120),
        ).await;

        match code_result {
            Ok(code) => {
                let client = reqwest::Client::new();
                match openai_oauth::exchange_code_for_tokens(&client, &code, &pkce).await {
                    Ok(token_set) => {
                        // Store the access token in SkillCoco's AuthState
                        let token = token_set.access_token.clone();
                        let mut store = auth.store.lock().unwrap();
                        store.credentials.insert(
                            "openai".to_string(),
                            ProviderCredential {
                                provider: "openai".to_string(),
                                method: AuthMethod::OAuth,
                                api_key: None,
                                oauth_token: Some(token),
                                display_name: Some("OpenAI (OAuth)".to_string()),
                                model: Some("gpt-4o".to_string()),
                                base_url: None,
                            },
                        );
                        if store.active_provider.is_none() {
                            store.active_provider = Some("openai".to_string());
                        }
                        drop(store);
                        let _ = auth.persist();

                        // Mark flow as completed
                        if let Ok(mut completed) = flow.completed.lock() {
                            completed.insert("openai".to_string(), true);
                        }
                    }
                    Err(e) => {
                        eprintln!("OAuth token exchange failed: {}", e);
                    }
                }
            }
            Err(e) => {
                eprintln!("OAuth callback failed: {}", e);
            }
        }
    });

    Ok(OAuthStartResult {
        started: true,
        provider,
    })
}

/// Check if an OAuth flow has completed for the given provider.
#[tauri::command]
pub fn check_oauth_status(
    auth: State<AuthState>,
    flow: State<OAuthFlowState>,
    provider: String,
) -> Result<OAuthStatusResult, String> {
    let completed = flow.completed.lock().map_err(|e| e.to_string())?;
    let is_completed = completed.get(&provider).copied().unwrap_or(false);

    // Also check if credential exists
    let has_credential = auth.get_credential(&provider)
        .map(|c| c.is_some())
        .unwrap_or(false);

    Ok(OAuthStatusResult {
        completed: is_completed,
        provider,
        authenticated: has_credential,
    })
}
```

**Step 3: Make AuthState fields accessible and Clone**

In `src-tauri/src/auth/mod.rs`, the `CredentialStore` and `AuthState` internal fields need to be accessible from the `oauth` submodule. Change:

```rust
// Change:
struct CredentialStore {
// To:
pub(crate) struct CredentialStore {
```

And:
```rust
// Change:
pub struct AuthState {
    store_path: PathBuf,
    store: Mutex<CredentialStore>,
}
// To:
#[derive(Clone)]
pub struct AuthState {
    pub(crate) store_path: PathBuf,
    pub(crate) store: Arc<Mutex<CredentialStore>>,
}
```

Add `use std::sync::Arc;` to imports and update `AuthState::new`:

```rust
Self {
    store_path,
    store: Arc::new(Mutex::new(store)),
}
```

Also derive Clone for OAuthFlowState by wrapping its Mutex in an Arc:

In `src-tauri/src/auth/oauth.rs`, change OAuthFlowState:

```rust
#[derive(Clone)]
pub struct OAuthFlowState {
    pub completed: Arc<Mutex<HashMap<String, bool>>>,
}

impl OAuthFlowState {
    pub fn new() -> Self {
        Self {
            completed: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}
```

And add `use std::sync::Arc;` to imports.

**Step 4: Register commands and state in lib.rs**

In `src-tauri/src/lib.rs`, add the new state and commands:

Add after the existing `app.manage(auth_state);` line:
```rust
app.manage(crate::auth::oauth::OAuthFlowState::new());
```

Add to the `.invoke_handler(tauri::generate_handler![...])`:
```rust
crate::auth::oauth::start_oauth_login,
crate::auth::oauth::check_oauth_status,
```

**Step 5: Add `open` crate to Cargo.toml**

Add to `[dependencies]` in `src-tauri/Cargo.toml`:
```toml
open = "5"
```

**Step 6: Run Rust tests**

Run: `cd /Users/gshah/work/apps/skillcoco/skillcoco/src-tauri && cargo test 2>&1 | tail -20`
Expected: All tests pass, compilation succeeds.

**Step 7: Commit**

```bash
git add src-tauri/src/auth/oauth.rs src-tauri/src/auth/mod.rs src-tauri/src/lib.rs src-tauri/Cargo.toml src-tauri/Cargo.lock
git commit -m "feat: add start_oauth_login and check_oauth_status Tauri commands"
```

---

### Task 3: Add frontend OAuth command wrappers and types

**Files:**
- Modify: `src/types/ai.ts` (add OAuth types)
- Modify: `src/lib/tauri-commands.ts` (add command wrappers)

**Step 1: Add OAuth types**

Add to bottom of `src/types/ai.ts`:

```typescript
// ── OAuth ──

export interface OAuthStartResult {
  started: boolean;
  provider: string;
}

export interface OAuthStatusResult {
  completed: boolean;
  provider: string;
  authenticated: boolean;
}
```

**Step 2: Add command wrappers**

Add to `src/lib/tauri-commands.ts` after the auth section:

```typescript
// ── OAuth ──

export async function startOAuthLogin(provider: string): Promise<import("@/types/ai").OAuthStartResult> {
  return invoke("start_oauth_login", { provider });
}

export async function checkOAuthStatus(provider: string): Promise<import("@/types/ai").OAuthStatusResult> {
  return invoke("check_oauth_status", { provider });
}
```

**Step 3: Run tests**

Run: `cd /Users/gshah/work/apps/skillcoco/skillcoco && npx vitest run 2>&1 | tail -20`
Expected: All tests pass.

**Step 4: Commit**

```bash
git add src/types/ai.ts src/lib/tauri-commands.ts
git commit -m "feat: add OAuth command wrappers and types"
```

---

### Task 4: Add "Sign in" buttons to Settings UI

**Files:**
- Modify: `src/pages/Settings.tsx`

**Step 1: Add OAuth state and handler**

Add to the Settings component state (after the existing state declarations around line 68):

```typescript
const [oauthPending, setOauthPending] = useState<string | null>(null);
```

Add the OAuth handler function (after `handleOllamaCheck`, around line 174):

```typescript
async function handleOAuthLogin(providerId: string) {
  setOauthPending(providerId);
  try {
    await commands.startOAuthLogin(providerId);

    // Poll for completion
    const maxAttempts = 30; // 60 seconds at 2s intervals
    for (let i = 0; i < maxAttempts; i++) {
      await new Promise((r) => setTimeout(r, 2000));
      const status = await commands.checkOAuthStatus(providerId);
      if (status.completed) {
        await loadAuthStatus();
        setOauthPending(null);
        return;
      }
    }
    // Timeout
    setOauthPending(null);
  } catch (err) {
    console.error("OAuth login failed:", err);
    setOauthPending(null);
  }
}
```

**Step 2: Add OAuth button to provider cards**

In the provider card actions section (around line 349-369, the `!isConnected` branch), add the OAuth button before the BYOK toggle. Change the block to:

```tsx
{/* OAuth for supported providers */}
{(provider.id === "openai" || provider.id === "gemini") && (
  <button
    onClick={() => handleOAuthLogin(provider.id)}
    disabled={oauthPending === provider.id}
    className="flex items-center gap-1.5 rounded-lg bg-primary px-4 py-2 text-xs font-medium text-primary-foreground transition-colors hover:bg-primary/90 disabled:opacity-50"
  >
    {oauthPending === provider.id ? (
      <>
        <Loader2 size={12} className="animate-spin" />
        Waiting for browser...
      </>
    ) : (
      <>
        <ExternalLink size={12} />
        Sign in with {provider.name}
      </>
    )}
  </button>
)}

{/* BYOK Toggle */}
<button
  onClick={() =>
    setExpandedBYOK(
      isBYOKExpanded ? null : provider.id,
    )
  }
  className="flex items-center gap-1.5 rounded-lg border border-border px-3 py-2 text-xs font-medium text-muted-foreground transition-colors hover:text-foreground"
>
  <Key size={12} />
  Use API Key
  {isBYOKExpanded ? (
    <ChevronUp size={12} />
  ) : (
    <ChevronDown size={12} />
  )}
</button>
```

Add `Loader2` to the existing lucide-react import if not already there (check line 5 imports).

**Step 3: Run tests**

Run: `cd /Users/gshah/work/apps/skillcoco/skillcoco && npx vitest run 2>&1 | tail -20`
Expected: All tests pass.

**Step 4: Commit**

```bash
git add src/pages/Settings.tsx
git commit -m "feat: add OAuth sign-in buttons for OpenAI and Gemini in Settings"
```

---

### Task 5: Update ai_request to use OAuth tokens with bearer auth

**Files:**
- Modify: `src-tauri/src/ai/service.rs`

The `ai_request` function already reads `oauth_token` from credentials (line 39: `cred.api_key.as_deref().or(cred.oauth_token.as_deref())`), but zeroclaw's `create_provider_with_options` passes this as an API key. For OAuth, the token needs to be used as a Bearer token.

**Step 1: Update provider creation to handle OAuth**

In `src-tauri/src/ai/service.rs`, change the provider creation block (lines 46-51):

```rust
let provider = if let Some(base_url) = &cred.base_url {
    providers::create_provider_with_url(&provider_name, api_key, Some(base_url))
} else if cred.method == crate::auth::AuthMethod::OAuth {
    // For OAuth, pass the token as bearer
    let mut oauth_options = options.clone();
    oauth_options.auth_profile_override = None; // use token directly
    providers::create_provider_with_options(&provider_name, api_key, &oauth_options)
} else {
    providers::create_provider_with_options(&provider_name, api_key, &options)
}
.map_err(|e| format!("Failed to create AI provider: {}", e))?;
```

Note: zeroclaw's OpenAI provider auto-detects bearer vs API key based on the token format (JWT = bearer), so this should work as-is. But we add the explicit branch for clarity.

Also add the `PartialEq` derive to `AuthMethod` if not already there (it already has it in the current code).

**Step 2: Run Rust tests**

Run: `cd /Users/gshah/work/apps/skillcoco/skillcoco/src-tauri && cargo test 2>&1 | tail -20`
Expected: All tests pass.

**Step 3: Commit**

```bash
git add src-tauri/src/ai/service.rs
git commit -m "feat: handle OAuth bearer tokens in ai_request provider creation"
```

---

### Task 6: Final integration verification

**Step 1: Run all Rust tests**

Run: `cd /Users/gshah/work/apps/skillcoco/skillcoco/src-tauri && cargo test`
Expected: All tests pass (53+).

**Step 2: Run all React tests**

Run: `cd /Users/gshah/work/apps/skillcoco/skillcoco && npx vitest run`
Expected: All tests pass (27+).

**Step 3: Verify Rust compiles cleanly**

Run: `cd /Users/gshah/work/apps/skillcoco/skillcoco/src-tauri && cargo check`
Expected: No errors.

**Step 4: Manual test**

Run: `cd /Users/gshah/work/apps/skillcoco/skillcoco && pnpm tauri dev`

1. Open Settings
2. Click "Sign in with OpenAI" on the OpenAI card
3. Browser should open to OpenAI auth page
4. After authenticating, button should change to "Connected"
5. Go to Dashboard, create a track -- should use OpenAI as provider

---

## Summary

| Task | What | Files |
|------|------|-------|
| 1 | Make zeroclaw auth module public | `zeroclaw/src/lib.rs` |
| 2 | Add Tauri OAuth commands + state | `auth/oauth.rs`, `auth/mod.rs`, `lib.rs`, `Cargo.toml` |
| 3 | Frontend OAuth types + wrappers | `types/ai.ts`, `tauri-commands.ts` |
| 4 | Settings UI "Sign in" buttons | `Settings.tsx` |
| 5 | Update ai_request for OAuth bearer | `ai/service.rs` |
| 6 | Final integration verification | -- |
