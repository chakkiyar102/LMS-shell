//! "ChatGPT plan" provider: the learner's ChatGPT Plus or Pro plan, used through OpenAI's own Codex
//! CLI after `codex login` with "Sign in with ChatGPT". The app never sees a token; it runs
//! `codex exec` read-only in an empty folder and reads the final message. Text only: Codex runs its
//! own agent, so Sidekick's screen helper (which needs our typed tools) stays on the OpenAI API.
use shared::brand::APP_NAME;
use shared::brand::COMPANION;
use shared::Msg;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

/// What the Model dropdown shows for "no -m flag".
pub const DEFAULT_MODEL: &str = "Codex default";

/// The codex launcher plus a PATH that can run it. A Mac app started from the Dock doesn't inherit the
/// shell's PATH, and the npm-installed `codex` is a node script, so node's folder must be on PATH too.
fn locate() -> Option<&'static (PathBuf, String)> {
    static FOUND: OnceLock<Option<(PathBuf, String)>> = OnceLock::new();
    FOUND.get_or_init(|| {
        let home = std::env::var("HOME").unwrap_or_default();
        let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
        // nvm installs live under versioned folders, newest last after sorting
        if let Ok(rd) = std::fs::read_dir(format!("{home}/.nvm/versions/node")) {
            let mut v: Vec<PathBuf> = rd.flatten().map(|e| e.path().join("bin")).collect();
            v.sort();
            dirs.extend(v.into_iter().rev());
        }
        for d in ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin"] { dirs.push(d.into()); }
        for d in [".local/bin", ".npm-global/bin", ".bun/bin", ".volta/bin"] { dirs.push(PathBuf::from(&home).join(d)); }
        let exe = if cfg!(windows) { "codex.cmd" } else { "codex" };
        let codex = dirs.iter().map(|d| d.join(exe)).find(|p| p.is_file())?;
        let path = std::env::join_paths(std::iter::once(codex.parent()?.to_path_buf()).chain(dirs)).ok()?;
        Some((codex, path.to_string_lossy().into_owned()))
    }).as_ref()
}

fn command(args: &[&str]) -> Result<Command, String> {
    let (codex, path) = locate().ok_or(
        "Couldn't find the Codex CLI. Install it with `npm install -g @openai/codex`, then run `codex login` and choose Sign in with ChatGPT.")?;
    let mut c = Command::new(codex);
    c.args(args).env("PATH", path).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    Ok(c)
}

async fn run(mut c: Command, stdin: Option<String>) -> Result<std::process::Output, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if stdin.is_some() { c.stdin(Stdio::piped()); }
        let mut child = c.spawn().map_err(|e| format!("Couldn't start Codex: {e}"))?;
        if let Some(text) = stdin {
            use std::io::Write;
            child.stdin.take().ok_or("no stdin")?.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        }
        // no timeout of our own; Codex times out its own model calls
        child.wait_with_output().map_err(|e| e.to_string())
    }).await.map_err(|e| e.to_string())?
}

/// Ok when Codex is installed and signed in with ChatGPT (not with an API key).
pub async fn check() -> Result<(), String> {
    let out = run(command(&["login", "status"])?, None).await?;
    let text = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    if text.contains("ChatGPT") { return Ok(()); }
    Err("Codex isn't signed in with ChatGPT. Run `codex login` in a terminal and choose Sign in with ChatGPT.".into())
}

/// One reply, as plain text. The conversation is flattened into a single prompt because `codex exec`
/// takes one message; `--ephemeral` keeps it out of the learner's Codex history.
pub async fn chat(model: &str, system: &str, msgs: &[Msg]) -> Result<String, String> {
    let transcript: Vec<String> = msgs.iter().map(|m| format!("{}: {}", if m.role == "user" { "Learner" } else { concat!(shared::companion_name!()) }, m.content)).collect();
    let prompt = format!("{system}\n\nYou are replying inside {APP_NAME}. Answer with the reply text only: prose, formatted only as the instructions above allow. \
        Do not run commands, read files or make changes.\n\nConversation so far:\n{}\n\nWrite {COMPANION}'s next reply.", transcript.join("\n\n"));
    let dir = std::env::temp_dir().join(format!("kfa-codex-{}", std::process::id()))
        .join(format!("{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0)));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let out_file = dir.join("reply.txt");
    let (d, o) = (dir.to_string_lossy().into_owned(), out_file.to_string_lossy().into_owned());
    let mut args = vec!["exec", "--skip-git-repo-check", "--ephemeral", "--ignore-user-config", "--ignore-rules",
                        "--sandbox", "read-only", "--color", "never", "-C", &d, "-o", &o];
    if !model.trim().is_empty() && model != DEFAULT_MODEL { args.extend(["-m", model.trim()]); }
    args.push("-");
    let out = run(command(&args)?, Some(prompt)).await;
    let reply = std::fs::read_to_string(&out_file).unwrap_or_default();
    let _ = std::fs::remove_dir_all(&dir);
    let out = out?;
    if !reply.trim().is_empty() { return Ok(reply.trim().to_string()); }
    let err = String::from_utf8_lossy(&out.stderr);
    let last = err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("no reply");
    Err(format!("Codex didn't return a reply: {last}"))
}

#[cfg(test)]
mod tests {
    /// Uses the real Codex CLI and the learner's ChatGPT plan, so it only runs on request:
    /// cargo test --lib codex -- --ignored
    #[test]
    #[ignore]
    fn replies_through_chatgpt_plan() {
        tauri::async_runtime::block_on(async {
            super::check().await.expect("signed in with ChatGPT");
            let msgs = [shared::Msg { role: "user".into(), content: "In one short sentence: what is a notebook for?".into() }];
            let reply = super::chat(super::DEFAULT_MODEL, concat!("You are ", shared::companion_name!(), ", a concise course coach."), &msgs).await.expect("reply");
            assert!(reply.len() > 10, "{reply}");
            println!("reply: {reply}");
        });
    }
}
