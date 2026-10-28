//! LLM providers. The API key lives in the OS keychain and is only ever attached here,
//! in Rust — never handed to the UI or to the practice (page-agent) webview.
use serde_json::{json, Value};
use shared::{Msg, Provider};

#[cfg(desktop)]
const SERVICE: &str = "lms-shell";

/// One keychain slot per provider, so switching providers never loses a saved key.
fn account(p: &Provider) -> Option<&'static str> {
    match p {
        Provider::Ollama { .. } => None,
        Provider::OpenAi { base_url, .. } if is_openai(base_url) => Some("openai"),
        Provider::OpenAi { .. } => Some("custom"),
        Provider::Anthropic { .. } => Some("anthropic"),
        Provider::ChatGpt { .. } => None, // the plan is signed in inside Codex; no key here
    }
}

pub fn is_openai(base_url: &str) -> bool { base_url.trim_end_matches('/') == shared::OPENAI_BASE_URL }

/// Keys read from the Keychain once per run; macOS prompts on each read by a new (unsigned dev) binary.
fn cache() -> &'static std::sync::Mutex<std::collections::HashMap<&'static str, String>> {
    static C: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<&'static str, String>>> = std::sync::OnceLock::new();
    C.get_or_init(Default::default)
}

#[cfg(desktop)]
fn entry(p: &Provider) -> Result<keyring::Entry, String> {
    entry_for(account(p).ok_or("This provider doesn't use an API key.")?)
}

#[cfg(desktop)]
fn entry_for(acct: &str) -> Result<keyring::Entry, String> { keyring::Entry::new(SERVICE, acct).map_err(|e| e.to_string()) }

/// Mobile has no keyring backend; the file store (app-private storage) always handles keys there.
#[cfg(mobile)]
struct NoKeyring;
#[cfg(mobile)]
impl NoKeyring {
    fn delete_credential(&self) -> Result<(), String> { Err("no keyring".into()) }
    fn set_password(&self, _: &str) -> Result<(), String> { Err("no keyring".into()) }
    fn get_password(&self) -> Result<String, String> { Err("no keyring".into()) }
}
#[cfg(mobile)]
fn entry(_: &Provider) -> Result<NoKeyring, String> { Err("no keyring".into()) }
#[cfg(mobile)]
fn entry_for(_: &str) -> Result<NoKeyring, String> { Err("no keyring".into()) }

static KEY_DIR: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
/// The app's private data directory (set at startup); on Android only this app can read it.
pub fn set_key_dir(d: &std::path::Path) { let _ = KEY_DIR.set(d.to_path_buf()); }

/// Dev builds keep keys in a file only this user can read: every rebuild is a new unsigned
/// binary to macOS, so the Keychain would ask for the login password after each one.
/// release builds use the Keychain; sign the app before shipping so it asks once.
fn dev_store() -> Option<std::path::PathBuf> {
    if cfg!(mobile) { return KEY_DIR.get().map(|d| d.join("keys.json")); }
    cfg!(debug_assertions).then(|| std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(|h| std::path::Path::new(&h).join(".lms-shell-dev-keys.json"))).flatten()
}

fn dev_read() -> std::collections::HashMap<String, String> {
    dev_store().and_then(|p| std::fs::read_to_string(p).ok()).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

fn dev_write(m: &std::collections::HashMap<String, String>) -> Result<(), String> {
    let p = dev_store().ok_or("no dev store")?;
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600); // owner-only; Windows profile ACLs already restrict it
    let mut f = o.open(&p).map_err(|e| e.to_string())?;
    std::io::Write::write_all(&mut f, serde_json::to_string(m).map_err(|e| e.to_string())?.as_bytes()).map_err(|e| e.to_string())
}

pub fn set_key(p: &Provider, k: &str) -> Result<(), String> {
    if let Some(a) = account(p) { cache().lock().unwrap().remove(a); }
    if dev_store().is_some() {
        let acct = account(p).ok_or("This provider doesn't use an API key.")?;
        let mut m = dev_read();
        if k.is_empty() { m.remove(acct); } else { m.insert(acct.to_string(), k.to_string()); }
        dev_write(&m)?;
        if !k.is_empty() { cache().lock().unwrap().insert(acct, k.to_string()); }
        return Ok(());
    }
    if k.is_empty() {
        let _ = entry(p)?.delete_credential(); // already absent is fine
        return Ok(());
    }
    entry(p)?.set_password(k).map_err(|e| e.to_string())?;
    if let Some(a) = account(p) { cache().lock().unwrap().insert(a, k.to_string()); }
    Ok(())
}

/// Every account `account()` can name: what Settings → Start over erases.
const ACCOUNTS: [&str; 3] = ["openai", "custom", "anthropic"];

/// Deletes every provider's saved key (the Keychain items under this app's service, or the dev/mobile
/// key file's entries) and forgets the cached copies. Already absent is fine.
pub fn clear_keys() -> Result<(), String> {
    cache().lock().unwrap_or_else(|p| p.into_inner()).clear();
    if dev_store().is_some() {
        let mut m = dev_read();
        if ACCOUNTS.iter().fold(false, |any, a| m.remove(*a).is_some() || any) { dev_write(&m)?; }
        return Ok(());
    }
    for a in ACCOUNTS { if let Ok(e) = entry_for(a) { let _ = e.delete_credential(); } }
    Ok(())
}

pub fn has_key(p: &Provider) -> bool { key(p).is_ok() }

fn key(p: &Provider) -> Result<String, String> {
    let acct = account(p).ok_or("This provider doesn't use an API key.")?;
    if let Some(k) = cache().lock().unwrap().get(acct) { return Ok(k.clone()); }
    if dev_store().is_some() {
        let k = dev_read().remove(acct).ok_or("No API key saved for this provider. Add one in Settings.")?;
        cache().lock().unwrap().insert(acct, k.clone());
        return Ok(k);
    }
    let k = entry(p)?.get_password().map_err(|_| {
        "Couldn't read the API key from your Keychain. If macOS asked for a password, it's your Mac login password; choose Always Allow.".to_string()
    })?;
    cache().lock().unwrap().insert(acct, k.clone());
    Ok(k)
}

fn client() -> reqwest::Client {
    reqwest::Client::builder().timeout(std::time::Duration::from_secs(120)).build().expect("http client")
}

async fn send(req: reqwest::RequestBuilder) -> Result<Value, String> {
    let res = req.send().await.map_err(|e| format!("LLM unreachable: {e}"))?;
    let status = res.status();
    let v: Value = res.json().await.map_err(|e| e.to_string())?;
    if !status.is_success() { return Err(format!("LLM error {status}: {v}")); }
    Ok(v)
}

// ---- every AI request goes through one of the three entry points below ----
// Each checks the learner's soft daily limit first and records the request after, through the meter
// lib.rs installs at start-up (db::usage_check / db::usage_add). Every chat reply is capped in length.

/// `None`: may a request go now? `Some(tokens)`: one request finished, about this many tokens.
pub type Meter = Box<dyn Fn(Option<u64>) -> Result<(), String> + Send + Sync>;
static METER: std::sync::OnceLock<Meter> = std::sync::OnceLock::new();
pub fn set_meter(m: Meter) { let _ = METER.set(m); }
fn meter_check() -> Result<(), String> { METER.get().map_or(Ok(()), |m| m(None)) }
fn meter_record(tokens: u64) { if let Some(m) = METER.get() { let _ = m(Some(tokens)); } }

/// The provider's own count when it sends one, else about four characters a token.
fn tokens_of(v: &Value, sent_chars: usize, got_chars: usize) -> u64 {
    let u = &v["usage"];
    u["total_tokens"].as_u64()
        .or_else(|| Some(u["input_tokens"].as_u64()? + u["output_tokens"].as_u64()?))
        .unwrap_or(((sent_chars + got_chars) / 4) as u64)
}

/// One chat turn (Ask, Coach, the exercise review, the provider test). Secrets in the messages are
/// removed before they leave the machine, and the reply is capped.
pub async fn chat(p: &Provider, system: &str, msgs: &[Msg]) -> Result<String, String> {
    meter_check()?;
    let msgs: Vec<Msg> = msgs.iter().map(|m| Msg { role: m.role.clone(), content: crate::guard::redact_secrets(&m.content) }).collect();
    let sent = system.len() + msgs.iter().map(|m| m.content.len()).sum::<usize>();
    let reply = chat_once(p, system, &msgs).await?;
    meter_record(tokens_of(&Value::Null, sent, reply.len()));
    Ok(crate::guard::cap_reply(reply))
}

/// One turn of the screen helper's loop (OpenAI Responses API).
pub async fn respond(p: &Provider, body: Value) -> Result<Value, String> {
    meter_check()?;
    let sent = body.to_string().len();
    let v = respond_once(p, body).await?;
    meter_record(tokens_of(&v, sent, v["output"].to_string().len()));
    Ok(v)
}

async fn chat_once(p: &Provider, system: &str, msgs: &[Msg]) -> Result<String, String> {
    if let Provider::ChatGpt { model } = p {
        #[cfg(desktop)] return crate::codex::chat(model, system, msgs).await;
        #[cfg(mobile)] { let _ = model; return Err("A ChatGPT plan works on the desktop app only.".into()); }
    }
    let mut with_system = vec![json!({"role": "system", "content": system})];
    with_system.extend(msgs.iter().map(|m| json!({"role": m.role, "content": m.content})));
    let c = client();
    let (req, path): (_, &[&str]) = match p {
        Provider::Ollama { host, model } => (
            c.post(format!("{}/api/chat", host.trim_end_matches('/')))
                .json(&json!({"model": model, "messages": with_system, "stream": false})),
            &["message", "content"],
        ),
        Provider::OpenAi { base_url, model } => (
            c.post(format!("{}/chat/completions", base_url.trim_end_matches('/')))
                .bearer_auth(key(p)?).json(&json!({"model": model, "messages": with_system})),
            &["choices", "0", "message", "content"],
        ),
        Provider::Anthropic { model } => (
            c.post("https://api.anthropic.com/v1/messages")
                .header("x-api-key", key(p)?).header("anthropic-version", "2023-06-01")
                .json(&json!({"model": model, "max_tokens": 1024, "system": system, "messages": msgs})),
            &["content", "0", "text"],
        ),
        Provider::ChatGpt { .. } => unreachable!("handled above"),
    };
    let v = send(req).await?;
    path.iter().fold(&v, |v, k| k.parse::<usize>().map_or(&v[*k], |i| &v[i]))
        .as_str().map(str::to_owned).ok_or_else(|| format!("unexpected LLM response: {v}"))
}

/// One turn of OpenAI's Responses API, which supports tools together with reasoning models
/// (Chat Completions rejects that pairing for newer models). Used by Sidekick's screen helper.
async fn respond_once(p: &Provider, mut body: Value) -> Result<Value, String> {
    let Provider::OpenAi { base_url, model } = p else {
        return Err(if matches!(p, Provider::ChatGpt { .. }) {
            concat!("A ChatGPT plan covers coaching and answers. ", shared::companion_name!(), "'s screen helper needs an OpenAI API key: switch to OpenAI in Settings to use it.")
        } else {
            concat!(shared::companion_name!(), "'s screen helper needs an OpenAI API key and model. Set one in Settings.")
        }.into());
    };
    body["model"] = json!(model);
    send(client().post(format!("{}/responses", base_url.trim_end_matches('/'))).bearer_auth(key(p)?).json(&body)).await
}

/// Lists the models the saved key can use, so the learner picks from a dropdown instead of typing ids.
pub async fn list_models(p: &Provider) -> Result<Vec<String>, String> {
    // Codex lists no models; doubling as the sign-in check keeps Settings' "load models" flow unchanged
    if let Provider::ChatGpt { .. } = p {
        #[cfg(desktop)] { crate::codex::check().await?; return Ok(vec![crate::codex::DEFAULT_MODEL.into()]); }
        #[cfg(mobile)] return Err("A ChatGPT plan works on the desktop app only.".into());
    }
    let c = client();
    let (req, field) = match p {
        Provider::Ollama { host, .. } => (c.get(format!("{}/api/tags", host.trim_end_matches('/'))), "name"),
        Provider::OpenAi { base_url, .. } => (c.get(format!("{}/models", base_url.trim_end_matches('/'))).bearer_auth(key(p)?), "id"),
        Provider::Anthropic { .. } => (
            c.get("https://api.anthropic.com/v1/models?limit=100").header("x-api-key", key(p)?).header("anthropic-version", "2023-06-01"),
            "id",
        ),
        Provider::ChatGpt { .. } => unreachable!("handled above"),
    };
    let v = send(req).await?;
    let items = v.get("data").or_else(|| v.get("models")).and_then(|x| x.as_array()).ok_or("The provider returned no model list.")?;
    let mut ids: Vec<String> = items.iter().filter_map(|m| m.get(field)?.as_str().map(str::to_owned)).collect();
    if matches!(p, Provider::OpenAi { base_url, .. } if is_openai(base_url)) {
        ids.retain(|id| chat_capable(id));
    }
    ids.sort();
    ids.dedup();
    Ok(ids)
}

/// OpenAI's /models also lists embedding, speech, image and moderation models; the tutor needs chat ones.
fn chat_capable(id: &str) -> bool {
    const NOT_CHAT: [&str; 11] = ["embedding", "tts", "whisper", "dall-e", "image", "moderation", "audio", "realtime", "transcribe", "search", "davinci"];
    !NOT_CHAT.iter().any(|x| id.contains(x)) && !id.starts_with("babbage") && !id.starts_with("ft:")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_over_clears_every_key_account() {
        let m = String::new;
        for p in [
            Provider::Ollama { host: m(), model: m() },
            Provider::OpenAi { base_url: shared::OPENAI_BASE_URL.into(), model: m() },
            Provider::OpenAi { base_url: "https://openrouter.ai/api/v1".into(), model: m() },
            Provider::Anthropic { model: m() },
            Provider::ChatGpt { model: m() },
        ] {
            if let Some(a) = account(&p) { assert!(ACCOUNTS.contains(&a), "{a} is not in ACCOUNTS"); }
        }
    }

    #[test]
    fn chat_filter() {
        assert!(super::chat_capable("gpt-4o-mini"));
        assert!(!super::chat_capable("text-embedding-3-small"));
        assert!(!super::chat_capable("gpt-4o-mini-tts"));
        assert!(!super::chat_capable("dall-e-3"));
    }
}
