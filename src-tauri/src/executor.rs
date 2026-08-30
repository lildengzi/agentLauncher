//! Generic run executor — agent-agnostic host logic.
//!
//! Owns everything around a run that does not depend on which agent is launched:
//! process spawn, streaming stdout/stderr to the frontend, the per-instance kill
//! channel, credential tiering ([`resolve_credentials`]), and status events. The
//! agent-specific command assembly is delegated to an `AgentRuntime` (see the
//! `runtime` module). The events emitted here (`runtime-status` / `runtime-log`)
//! carry every engine's output, so they are named after the seam rather than after
//! dsh.

use serde::Serialize;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_opener::OpenerExt;
use tokio::io::{AsyncReadExt, BufReader};
use tokio::sync::mpsc;

use crate::instance_manager;
use crate::providers;
use crate::runtime::{self, RunMode, SpawnRequest};

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
    /// For web (serve) runs: the served browser-UI URL once it appears on stdout.
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
        "runtime-status",
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
        "runtime-log",
        LogEvent {
            instance_id: id.to_string(),
            stream: stream.to_string(),
            chunk,
        },
    );
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
/// served browser-UI URL, and on the first hit opens it in the default browser
/// and re-emits a `running` status carrying the URL for the frontend.
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

/// Watch a spawned child until it exits or the kill channel fires, then report.
/// Shared by both hosted shapes: for a task run the child is the agent, for an
/// interactive one it is the terminal window holding the session — so 停止 closes
/// that window, which is the only "stop" the launcher can honestly offer once the
/// conversation belongs to the user.
fn supervise(
    app: AppHandle,
    state: &State<'_, RunnerState>,
    id: String,
    mut child: tokio::process::Child,
) {
    let (kill_tx, mut kill_rx) = mpsc::channel::<()>(1);
    state.0.lock().unwrap().insert(id.clone(), kill_tx);
    let map: KillMap = state.0.clone();
    tokio::spawn(async move {
        tokio::select! {
            status = child.wait() => {
                let code = status.ok().and_then(|s| s.code());
                emit_status(&app, &id, "exited", code, None, None);
            }
            _ = kill_rx.recv() => {
                let _ = child.kill().await;
                emit_status(&app, &id, "exited", None, Some("已手动结束".into()), None);
            }
        }
        map.lock().unwrap().remove(&id);
    });
}

/// Decide where this launch's API key comes from, and say so in the console.
///
/// Three tiers, most specific first, and the order is the whole point:
///
/// 1. **The instance's own `.env`.** If it already carries a key (recognised by
///    [`crate::instance_env::find_key`], which also decides what the 模型 page writes
///    there), that is the answer — the launcher does not consult its store at all (it
///    would be overridden a line later anyway, and asking would burn a rotation slot
///    on a key nobody uses).
/// 2. **The launcher's key store** (`providers.json`, via [`providers::dispatch`]).
/// 3. **The engine's own credential store**, which only dsh has
///    (`~/.dsh/.credentials.yaml`). No such assumption is made for the other five:
///    if they have a system-level login it is theirs to manage, so the launcher says
///    it injected nothing rather than pretending a key exists.
///
/// A failure from tier 2 is the user's own binding failing, so it is always loud —
/// but it only *stops* the launch when no lower tier could supply a credential.
fn resolve_credentials(
    app: &AppHandle,
    id: &str,
    inst: &instance_manager::Instance,
    engine: &str,
    own_env: &[(String, String)],
    envs: &mut Vec<(String, String)>,
) -> Result<(), String> {
    let note = |msg: String| emit_log(app, id, "stdout", format!("[agentLauncher] {msg}\n"));
    let warn = |msg: String| emit_log(app, id, "stderr", format!("[agentLauncher] {msg}\n"));

    // Tier 1 — the instance brought its own.
    if let Some((var, _)) = crate::instance_env::find_key(own_env) {
        note(format!("密钥来源：实例 .env 的 {var}"));
        return Ok(());
    }

    // dsh is the one engine with a credential store of its own, and it reads it
    // itself — so for dsh "nothing from the launcher" still means a working launch.
    let dsh_keys = if engine == "dsh" {
        runtime::dsh_home::list_credential_keys()
            .unwrap_or_default()
            .len()
    } else {
        0
    };

    // Tier 2 — the launcher's store (bound alias, or the next key in this provider's
    // rotation). This is the launcher's only chance to choose a credential: it never
    // sees a request, so it cannot react to a 401.
    match providers::dispatch::env_for_instance(inst) {
        Ok(pairs) if !pairs.is_empty() => {
            // Names only. A value never reaches the console, the same rule that keeps
            // it out of the frontend.
            let vars: Vec<&str> = pairs.iter().map(|(k, _)| k.as_str()).collect();
            note(format!("密钥来源：启动器密钥库 → {}", vars.join("、")));
            envs.extend(pairs);
            return Ok(());
        }
        Ok(_) => {}
        Err(e) => {
            if dsh_keys > 0 {
                warn(format!(
                    "启动器密钥库：{e}。改用 dsh 自己的凭据库 ~/.dsh/.credentials.yaml（共 {dsh_keys} 条）。"
                ));
                return Ok(());
            }
            emit_log(app, id, "stderr", format!("{e}\n"));
            emit_status(app, id, "error", None, Some(e.clone()), None);
            return Err(e);
        }
    }

    // Tier 3.
    if engine == "dsh" {
        if dsh_keys > 0 {
            note(format!(
                "密钥来源：dsh 自己的凭据库 ~/.dsh/.credentials.yaml（共 {dsh_keys} 条）"
            ));
        } else {
            warn(
                "三层都没有密钥：实例 .env 没有，启动器密钥库没有匹配的，dsh 的 \
                 ~/.dsh/.credentials.yaml 也是空的。这次启动会以未认证状态跑。"
                    .to_string(),
            );
        }
    } else {
        let bound = if !inst.api_key_ref.trim().is_empty() {
            format!("绑定的「{}」", inst.api_key_ref.trim())
        } else if !inst.provider.trim().is_empty() {
            format!("服务商「{}」", inst.provider.trim())
        } else {
            "这个实例".to_string()
        };
        warn(format!(
            "启动器没有为这次启动注入密钥（实例 .env 里没有，密钥库里也没有匹配{bound}的行）。\
             {engine} 的系统级凭据由它自己管理——如果它自己也没配置，这次启动会以未认证状态跑。"
        ));
    }
    Ok(())
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

    // Resolve this instance's runtime (dsh/pi/omp/claude/codex/opencode) and how
    // this run is hosted. The runtime owns binary/args/task-passing and per-run
    // config overlays; `RunMode` decides which of its two commands we build and
    // where the output goes — a terminal the user talks to, or a pipe into the
    // launcher's console.
    let agent = runtime::for_instance(&inst);
    let mode = RunMode::resolve(&inst, agent.as_ref());

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

    // Every way this launch can fail says why *in the console*, not only in the red
    // status dot: the reason is often a field the user has to go and fix (a provider
    // dsh has no route for, a key that resolves to nothing), and nothing renders the
    // status event's `message` today.
    let fail = |msg: String| -> String {
        emit_log(&app, &id, "stderr", format!("{msg}\n"));
        emit_status(&app, &id, "error", None, Some(msg.clone()), None);
        msg
    };

    // ---- the environment, layered once for both shapes ---------------------
    // Order is the contract: PATH first (resolved per this instance's env policy —
    // autodetect enriches from the login shell, isolated stays minimal), then the
    // launcher's stored API key, then the instance `.env`, so a value written by
    // hand there is the most specific source and wins.
    let mut envs: Vec<(String, String)> = Vec::new();
    if let Some(path) =
        runtime::env::resolve_child_path(&inst.runtime.env_policy, &inst.runtime.custom_bin).await
    {
        envs.push(("PATH".to_string(), path));
    }
    let own_env = crate::instance_env::parse(&env_path);
    resolve_credentials(&app, &id, &inst, agent.id(), &own_env, &mut envs)?;
    envs.extend(own_env);

    if mode == RunMode::Interactive {
        return start_interactive(app, state, id, &inst, &inst_dir, &workspace, envs, agent).await;
    }

    // ---- task / serve: the launcher hosts the pipes ------------------------
    let serve = mode == RunMode::Serve;
    let mut cmd = match agent.build_command(&SpawnRequest {
        instance: &inst,
        instance_dir: &inst_dir,
        task: &task_text,
    }) {
        Ok(c) => c,
        Err(e) => return Err(fail(e)),
    };
    cmd.current_dir(&workspace);
    cmd.envs(envs)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| fail(format!("无法启动 {}: {e}", agent.id())))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    if let Some(out) = stdout {
        spawn_reader(app.clone(), id.clone(), "stdout", out, serve);
    }
    if let Some(err) = stderr {
        spawn_reader(app.clone(), id.clone(), "stderr", err, false);
    }

    emit_status(&app, &id, "running", None, None, None);
    supervise(app, &state, id, child);
    Ok(())
}

/// Open an interactive session in the user's own terminal.
///
/// The launcher does not host the conversation — it writes `run.sh` (the exact
/// environment and argv for this launch, so the user can read or run it) and hands
/// that one path to a terminal emulator found on PATH. Nothing is piped: the
/// session's stdin/stdout belong to that window, which is the whole point. What
/// stays with the launcher is the window's process, so 停止 still means something
/// and the instance stops showing as running when the window closes.
#[allow(clippy::too_many_arguments)]
async fn start_interactive(
    app: AppHandle,
    state: State<'_, RunnerState>,
    id: String,
    inst: &instance_manager::Instance,
    inst_dir: &std::path::Path,
    workspace: &std::path::Path,
    envs: Vec<(String, String)>,
    agent: Box<dyn runtime::AgentRuntime>,
) -> Result<(), String> {
    let fail = |msg: String| -> String {
        emit_log(&app, &id, "stderr", format!("{msg}\n"));
        emit_status(&app, &id, "error", None, Some(msg.clone()), None);
        msg
    };

    let cmd = agent
        .build_interactive(&SpawnRequest {
            instance: inst,
            instance_dir: inst_dir,
            // An interactive session is given no task — the user types it.
            task: "",
        })
        .map_err(&fail)?;
    let (program, args) = runtime::program_and_args(&cmd);

    // The terminal is looked up on the same PATH the agent would get, not the
    // launcher's own: a GUI started from a dock inherits a stripped PATH, and a
    // terminal installed under ~/.local/bin is exactly the case that breaks.
    let path_var = envs
        .iter()
        .find(|(k, _)| k == "PATH")
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| std::env::var("PATH").unwrap_or_default());
    let term =
        runtime::term::pick(&path_var).ok_or_else(|| fail(runtime::term::not_found_message()))?;

    let script =
        runtime::term::write_run_script(inst_dir, workspace, &inst.name, &envs, &program, &args)
            .map_err(&fail)?;

    // Say what was launched, in one line: if the window never appears, this is the
    // only place that can explain why, and it names both halves — which terminal
    // the launcher picked and the script it can be re-run from by hand.
    emit_log(
        &app,
        &id,
        "stdout",
        format!(
            "[agentLauncher] 交互式会话：{} → {}\n",
            term.program,
            script.to_string_lossy()
        ),
    );

    let mut spawn = tokio::process::Command::new(&term.program);
    spawn.args(&term.args).arg(&script);
    spawn.current_dir(workspace);
    // Also set the env on the terminal itself. The script exports everything it
    // needs (that is why it exists — a D-Bus-client terminal would hand the child
    // its *daemon's* environment), so this is belt and braces, not the mechanism.
    spawn.envs(envs);
    spawn
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let child = spawn
        .spawn()
        .map_err(|e| fail(format!("无法启动终端 {}: {e}", term.program)))?;

    emit_status(&app, &id, "running", None, None, None);
    supervise(app, &state, id, child);
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

#[cfg(test)]
mod tests {
    use super::find_url;

    #[test]
    fn extracts_dsh_web_url() {
        // The real startup line dsh prints for a web profile.
        assert_eq!(
            find_url("dsh web: http://127.0.0.1:37645\n").as_deref(),
            Some("http://127.0.0.1:37645")
        );
        // https + trailing slash, terminated by following whitespace.
        assert_eq!(
            find_url("listening on https://0.0.0.0:3080/ now").as_deref(),
            Some("https://0.0.0.0:3080/")
        );
        // A quote terminates the token.
        assert_eq!(
            find_url("url=\"http://x:1\" done").as_deref(),
            Some("http://x:1")
        );
        // No URL present.
        assert_eq!(find_url("starting, no url yet"), None);
    }
}
