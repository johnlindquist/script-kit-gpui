# Environment Variable Prompt (`env()`)

**Document Date:** December 31, 2025  
**Scope:** Secure environment variable prompts with keychain storage

---

## Overview

The `env()` function prompts users for environment variable values and stores them securely in the system keychain. Once a value is stored, subsequent calls to `env()` with the same key automatically retrieve the stored value without prompting.

**Key Benefits:**
- **Secure Storage** - Values stored in macOS Keychain (not plain text files)
- **Automatic Retrieval** - Stored values auto-resolve on future runs
- **Secret Detection** - Automatically masks input for sensitive values
- **Persistence** - All values persist across sessions, not just secrets

---

## Quick Start

```typescript
// Basic usage - prompts for value, stores in keychain
const apiKey = await env("OPENAI_API_KEY");

// Force secret mode (masked input)
const password = await env("DATABASE_URL", { secret: true });

// Force non-secret mode (visible input)
const debugMode = await env("DEBUG_MODE", { secret: false });
```

---

## Keychain Integration

All environment values are stored in the macOS Keychain under the service name `com.scriptkit.env`. This provides:

| Feature | Description |
|---------|-------------|
| Encryption at rest | Values encrypted by macOS security framework |
| Per-user isolation | Each macOS user has separate storage |
| Persistence | Values survive app restarts and reboots |
| System integration | Managed via Keychain Access.app |

### Rust Implementation

```rust
// src/prompts/env.rs

const KEYRING_SERVICE: &str = "com.scriptkit.env";

pub fn get_secret(key: &str) -> Option<String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, key);
    entry.ok()?.get_password().ok()
}

pub fn set_secret(key: &str, value: &str) -> Result<(), String> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, key)?;
    entry.set_password(value)?;
    Ok(())
}
```

---

## Secret Detection

The SDK automatically detects secret values based on key name patterns:

| Pattern | Example Keys | Detected As |
|---------|--------------|-------------|
| Contains "secret" | `API_SECRET`, `MY_SECRET_KEY` | Secret |
| Contains "password" | `DATABASE_PASSWORD`, `SMTP_PASSWORD` | Secret |
| Contains "token" | `AUTH_TOKEN`, `ACCESS_TOKEN` | Secret |
| Contains "key" | `API_KEY`, `PRIVATE_KEY` | Secret |

### Detection Logic

```typescript
// scripts/kit-sdk.ts:3188-3194

const inferredSecret =
  key.toLowerCase().includes('secret') ||
  key.toLowerCase().includes('password') ||
  key.toLowerCase().includes('token') ||
  key.toLowerCase().includes('key');

const secret = typeof secretOverride === 'boolean' ? secretOverride : inferredSecret;
```

---

## Secret Override

Override automatic detection with the `secret` option:

```typescript
// Auto-detected as secret (contains "key")
const apiKey = await env("OPENAI_API_KEY");

// Force secret mode for non-matching names
const dbUrl = await env("DATABASE_URL", { secret: true });

// Force visible mode for key names that shouldn't be secret
const publicKey = await env("PUBLIC_API_KEY", { secret: false });
```

### When to Use Override

| Scenario | Recommendation |
|----------|----------------|
| URL with credentials | `{ secret: true }` |
| Public API keys | `{ secret: false }` |
| Configuration values | `{ secret: false }` |
| Tokens/passwords | Auto-detection works |

---

## Persistence Behavior

**All values persist in the keychain**, regardless of secret mode. The `secret` flag only controls:

1. **Input Masking** - Whether typed characters are shown or replaced with `•`
2. **UI Hints** - Different icons and messaging

```rust
// src/prompts/env.rs:158-167

fn submit(&mut self) {
    if !self.input_text.is_empty() {
        // Store in keyring for persistence (both secrets and regular values).
        // `secret` only controls masking/UI; persistence should be consistent.
        if let Err(e) = set_secret(&self.key, &self.input_text) {
            logging::log("ERROR", &format!("Failed to store value: {}", e));
        }
        (self.on_submit)(self.id.clone(), Some(self.input_text.clone()));
    }
}
```

---

## UI Hints

The prompt displays different visual cues based on secret mode:

| Mode | Icon | Input Display | Footer Message |
|------|------|---------------|----------------|
| Secret | 🔐 | `••••••••` | "Value will be stored in system keychain" |
| Non-Secret | 📝 | `actual-text` | "Value will be stored in system keychain for future runs" |

### Visual Structure

```
┌─────────────────────────────────────────┐
│ 🔐  Enter value for API_KEY             │
│     Key: API_KEY                        │
│ ┌─────────────────────────────────────┐ │
│ │ ••••••••••••                        │ │
│ └─────────────────────────────────────┘ │
│ 🔒 Value will be stored in system keychain │
│ Enter to submit    Esc to cancel        │
└─────────────────────────────────────────┘
```

---

## macOS Keychain Backend

The `keyring` crate with `apple-native` feature provides native Keychain integration:

```toml
# Cargo.toml:126-129

# Keyring integration for secure secret storage (env prompt)
# On macOS, enable the real Keychain backend
[target.'cfg(target_os = "macos")'.dependencies]
keyring = { version = "3", features = ["apple-native"] }
```

**Without `apple-native`:** The keyring crate may fall back to a mock/non-persistent store.

**With `apple-native`:** Values are stored in the real macOS Keychain and persist across reboots.

---

## Example Usage

### API Key Configuration

```typescript
// First run: prompts user, stores in keychain
// Subsequent runs: auto-retrieves from keychain
const openaiKey = await env("OPENAI_API_KEY");

const response = await fetch("https://api.openai.com/v1/chat/completions", {
  headers: { Authorization: `Bearer ${openaiKey}` }
});
```

### Database Connection

```typescript
// Force secret mode for URL with embedded credentials
const dbUrl = await env("DATABASE_URL", { secret: true });
const client = new Client({ connectionString: dbUrl });
```

### Debug Configuration

```typescript
// Non-secret: user can see what they type
const debugLevel = await env("DEBUG_LEVEL", { secret: false });
```

### Custom Prompt Function

```typescript
// Use a custom prompt instead of built-in UI
const customValue = await env("CUSTOM_KEY", {
  prompt: async () => {
    return await arg("Enter your custom value:");
  }
});
```

---

## Implementation Files

| File | Purpose |
|------|---------|
| `src/prompts/env.rs` | EnvPrompt struct, keyring operations |
| `scripts/kit-sdk.ts:3154-3213` | SDK `env()` function |
| `Cargo.toml:126-133` | keyring dependency with apple-native |

---

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Enter` | Submit value and store in keychain |
| `Escape` | Cancel and exit script |
| `Backspace` | Delete last character |

---

## Security Considerations

1. **Keychain Security** - Values are protected by macOS Keychain encryption
2. **Per-User Storage** - Each user account has isolated storage
3. **No Plain Text** - Values never written to disk as plain text
4. **Process Memory** - Values exist in process memory during script execution

### Deleting Stored Values

To remove a stored value, use Keychain Access.app:
1. Open Keychain Access
2. Search for `com.scriptkit.env`
3. Delete the entry for the specific key

Or programmatically (if exposed):
```rust
// src/prompts/env.rs:68-79
pub fn delete_secret(key: &str) -> Result<(), String>
```
