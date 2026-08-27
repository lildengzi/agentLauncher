//! Generic run executor — agent-agnostic host logic.
//!
//! Owns everything around a run that does not depend on which agent is launched:
//! process spawn, streaming stdout/stderr to the frontend, the per-instance kill
//! channel, and status events. The agent-specific command assembly is delegated
//! to an `AgentRuntime` (see the `runtime` module). Event names (`dsh-status` /
//! `dsh-log`) are the frontend contract and are intentionally left unchanged.

use serde::Serialize;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_opener::OpenerExt;
use tokio::io::{AsyncReadExt, BufReader};
use tokio::sync::mpsc;

use crate::dsh_config;
use crate::instance_manager;
use crate::runtime::{self, SpawnRequest};

/// Maps a running instance id to a channel that kills its process.
type KillMap = Arc<Mutex<HashMap<String, mpsc::Sender<()>>>>;

/// Tracks a kill channel per running instance id.
#[derive(Default)]
pub struct RunnerState(pub KillMap);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LogEvent {
    instance_id: String,
    stream: String,
    chunk: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StatusEvent {
    instance_id: String,
    status: String,
    code: Option<i32>,
    message: Option<String>,
    /// For web (serve) runs: the dsh browser-UI URL once it appears on stdout.
    url: Option<String>,
}

fn emit_status(
    app: &AppHandle,
    id: &str,
    status: &str,
    code: Option<i32>,
    message: Option<String>,
    url: Option<String>,
) {
    let _ = app.emit(
        "dsh-status",
        StatusEvent {
            instance_id: id.to_string(),
            status: status.to_string(),
            code,
            message,
            url,
        },
    );
}

fn emit_log(app: &AppHandle, id: &str, stream: &str, chunk: String) {
    let _ = app.emit(
        "dsh-log",
        LogEvent {
            instance_id: id.to_string(),
            stream: stream.to_string(),
            chunk,
        },
    );
}
/// Parse a minimal `.env` file into (key, value) pairs.
fn parse_env(path: &std::path::Path) -> Vec<(String, String)> {
    let mut out = vec![];
    if let Ok(text) = std::fs::read_to_string(path) {
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                let v = v.trim().trim_matches('"');
                out.push((k.trim().to_string(), v.to_string()));
            }
        }
    }
    out
}

/// Extract the first `http(s)://…` token from a chunk (the dsh web server prints
/// `dsh web: http://127.0.0.1:<port>` on startup). Reads up to the first
/// whitespace or quote.
fn find_url(s: &str) -> Option<String> {
    let start = s.find("http://").or_else(|| s.find("https://"))?;
    let rest = &s[start..];
    let end = rest
        .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
        .unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

/// Spawn a reader that streams a pipe to the frontend as chunks. When
/// `detect_url` is set (the stdout of a web/serve run), it also scans for the
/// dsh browser-UI URL, and on the first hit opens it in the default browser and
/// re-emits a `running` status carrying the URL for the frontend.
fn spawn_reader<R>(app: AppHandle, id: String, stream: &'static str, reader: R, detect_url: bool)
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut buf = [0u8; 4096];
        let mut r = BufReader::new(reader);
        let mut opened = false;
        loop {
            match r.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    let chunk = String::from_utf8_lossy(&buf[..n]).to_string();
                    if detect_url && !opened {
                        if let Some(url) = find_url(&chunk) {
                            opened = true;
                            let _ = app.opener().open_url(url.clone(), None::<&str>);
                            emit_status(&app, &id, "running", None, None, Some(url));
                        }
                    }
                    emit_log(&app, &id, stream, chunk);
                }
                Err(_) => break,
            }
        }
    });
}
pub async fn start(
    app: AppHandle,
    state: State<'_, RunnerState>,
    id: String,
    task: Option<String>,
) -> Result<(), String> {
    // Already running? stop the previous run first.
    let existing = state.0.lock().unwrap().get(&id).cloned();
    if let Some(tx) = existing {
        let _ = tx.send(()).await;
    }

    let inst = instance_manager::get_instance(&id)?;
    let workspace = instance_manager::workspace_dir(&id)?;
    let inst_dir = instance_manager::instance_dir(&id)?;
    let env_path = inst_dir.join(".env");

    // A web-capable profile boots dsh's browser-UI server (long-running, prints a
    // URL); anything else runs a one-shot task. The runtime assembles the matching
    // command; here we only need to know whether to watch stdout for the URL.
    let serve = dsh_config::profile_is_web_capable(&inst.profile);

    let task_text = task
        .filter(|t| !t.trim().is_empty())
        .or_else(|| {
            if inst.default_task.trim().is_empty() {
                None
            } else {
                Some(inst.default_task.clone())
            }
        })
        .unwrap_or_else(|| "介绍一下你自己，并列出当前 workspace 里的文件。".to_string());

    emit_status(&app, &id, "starting", None, None, None);

    // Assemble the launch command via this instance's runtime (dsh today). The
    // runtime owns binary/args/task-passing and per-run config overlays; the
    // executor adds the generic cwd / env / stdio wiring below.
    let agent = runtime::for_instance(&inst);
    let mut cmd = match agent.build_command(&SpawnRequest {
        instance: &inst,
        instance_dir: &inst_dir,
        task: &task_text,
    }) {
        Ok(c) => c,
        Err(e) => {
            emit_status(&app, &id, "error", None, Some(e.clone()), None);
            return Err(e);
        }
    };
    cmd.current_dir(&workspace)
        .envs(parse_env(&env_path))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|e| {
        let msg = format!("无法启动 dsh: {e}");
        emit_status(&app, &id, "error", None, Some(msg.clone()), None);
        msg
    })?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    if let Some(out) = stdout {
        spawn_reader(app.clone(), id.clone(), "stdout", out, serve);
    }
    if let Some(err) = stderr {
        spawn_reader(app.clone(), id.clone(), "stderr", err, false);
    }

    emit_status(&app, &id, "running", None, None, None);

    let (kill_tx, mut kill_rx) = mpsc::channel::<()>(1);
    state.0.lock().unwrap().insert(id.clone(), kill_tx);

    // Wait / kill supervisor.
    let app_wait = app.clone();
    let id_wait = id.clone();
    let map: KillMap = state.0.clone();
    tokio::spawn(async move {
        tokio::select! {
            status = child.wait() => {
                let code = status.ok().and_then(|s| s.code());
                emit_status(&app_wait, &id_wait, "exited", code, None, None);
            }
            _ = kill_rx.recv() => {
                let _ = child.kill().await;
                emit_status(&app_wait, &id_wait, "exited", None, Some("已手动结束".into()), None);
            }
        }
        map.lock().unwrap().remove(&id_wait);
    });

    Ok(())
}

pub async fn stop(state: State<'_, RunnerState>, id: String) -> Result<(), String> {
    let tx = state.0.lock().unwrap().remove(&id);
    if let Some(tx) = tx {
        let _ = tx.send(()).await;
        Ok(())
    } else {
        Err(format!("实例未在运行: {id}"))
    }
}
