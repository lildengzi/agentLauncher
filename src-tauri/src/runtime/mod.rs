//! Agent runtime abstraction.
//!
//! The launcher's host logic — process supervision, log streaming, the kill
//! channel, instance storage — is agent-agnostic. The only agent-specific piece
//! of a run is *how the launch command is assembled*: binary name, argument
//! shape, how the task text is passed, and any per-run config overlay.
//! `AgentRuntime` captures exactly that seam, so each additional CLI is a new
//! impl rather than edits scattered through the executor.
//!
//! Two commands come out of that seam, not one — see [`RunMode`]. Every one of
//! these CLIs is a conversation by default and a one-shot task only when told to
//! be (`-p` / `exec` / `run`), so the launcher assembles both shapes: an
//! interactive session, which [`term`] hands to the user's own terminal, and a
//! headless task, whose output the executor pipes into the launcher's console.
//! dsh is the one engine where the shape is decided by its profile instead.
//!
//! Today six runtimes exist: `DshRuntime` (first-class, the only one with a web
//! serve mode) plus the `pi`/`omp`/`claude`/`codex`/`opencode` adapters. All six
//! live in `model.rs` — one file, one table — and `for_instance` dispatches on
//! `instance.runtime.engine`; their contract is asserted row by row in
//! `model_test.rs`.
//!
//! `dsh_home.rs` is the other half of being agent-specific: dsh's own `$DSH_HOME`
//! installation (profiles, plugins, credential file). It lives here rather than at
//! the top level because none of it generalises — the other five engines are
//! configured through their own homes and the instance `.env`.

pub mod dsh_home;
pub mod env;
mod model;
#[cfg(test)]
mod model_test;
pub mod term;

use std::path::Path;
use tokio::process::Command;

use crate::instance_manager::Instance;

/// How one run is hosted. Not an engine property — the same engine answers a task
/// or holds a conversation depending on which flags it is given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    /// The engine's own REPL/TUI, in a terminal window the launcher opens (see
    /// [`term`]). No headless flag, no task text: the user talks to it directly.
    Interactive,
    /// One task in, one answer out, stdout/stderr captured into the launcher's
    /// console. This is what the headless flag (`-p` / `exec` / `run`) is for.
    Task,
    /// A long-running server whose URL the launcher opens in the browser. dsh's
    /// web profiles only.
    Serve,
}

impl RunMode {
    /// Resolve the mode for one launch.
    ///
    /// A web-capable dsh profile is a server no matter what the field says — it
    /// bundles the browser UI, and there is nothing else that run could mean.
    /// Otherwise the instance's stored `runtime.mode` decides, and **empty means
    /// "ask the engine"**: an `instance.json` written before this field existed
    /// must not be reinterpreted, and the honest reading of a bare `omp` / `claude`
    /// / `codex` / `opencode` invocation is the interactive one — every one of
    /// them starts a session by default and treats headless as the special case.
    /// dsh is the exception, and says so itself ([`AgentRuntime::default_mode`]).
    pub fn resolve(inst: &Instance, agent: &dyn AgentRuntime) -> RunMode {
        if agent.is_serve(inst) {
            return RunMode::Serve;
        }
        match inst.runtime.mode.trim() {
            "task" => RunMode::Task,
            "interactive" => RunMode::Interactive,
            _ => agent.default_mode(),
        }
    }
}

/// Everything a runtime needs to assemble a launch command. The executor supplies
/// working directory, environment, and stdio afterwards.
pub struct SpawnRequest<'a> {
    /// The instance being launched.
    pub instance: &'a Instance,
    /// The instance's on-disk directory, for writing per-run config overlays.
    pub instance_dir: &'a Path,
    /// The resolved task text for this run. Ignored by
    /// [`AgentRuntime::build_interactive`], which passes no task at all.
    pub task: &'a str,
}

/// A launchable agent runtime. Implementations own the command-assembly specific
/// to one agent CLI; the generic executor owns everything around it.
pub trait AgentRuntime: Send + Sync {
    /// Stable engine id ("dsh", "pi", …) — used for dispatch tests and the
    /// spawn-failure message.
    fn id(&self) -> &'static str;

    /// Build the headless one-shot command for this instance and task, writing
    /// any per-run config files under `req.instance_dir`. The executor adds the
    /// working directory, environment, and stdio wiring before spawning.
    fn build_command(&self, req: &SpawnRequest) -> Result<Command, String>;

    /// Build the *interactive* command: the same binary and the same model /
    /// provider selection, minus the headless flag and minus the task. The
    /// executor hands this to a terminal ([`term`]) rather than to a pipe.
    ///
    /// The difference between this and [`Self::build_command`] is one flag per
    /// engine — which is exactly why both live in the adapter: `-p`, `exec` and
    /// `run` are not interchangeable and only the adapter knows which is which.
    fn build_interactive(&self, req: &SpawnRequest) -> Result<Command, String>;

    /// What an instance with no stored `runtime.mode` means for this engine.
    /// Interactive for the five CLI agents (all of them start a session when
    /// invoked bare); dsh overrides, because for dsh the *profile* is the mode.
    fn default_mode(&self) -> RunMode {
        RunMode::Interactive
    }

    /// Whether this instance runs a long-running server whose URL the executor
    /// should watch stdout for (web mode), rather than a one-shot task. Only
    /// dsh's web-capable profiles return true today.
    fn is_serve(&self, _inst: &Instance) -> bool {
        false
    }
}

/// Build a `Command` for `default_bin`, or for `custom_bin` when it is non-empty
/// (its directory has already been put on PATH by the executor; see runtime::env).
///
/// On Windows the default binary is resolved to an absolute path first, because a
/// bare `claude` is not launchable there: npm installs the CLI as `claude.cmd`,
/// and CreateProcess only ever appends `.exe`. Detection already follows this rule
/// ([`crate::engines::find_on_path`]), so resolving here is what keeps "installed"
/// and "launchable" the same claim. Unresolvable falls through to the bare name so
/// the failure message stays the one the user can act on.
fn program(custom_bin: &str, default_bin: &str) -> Command {
    let bin = custom_bin.trim();
    if !bin.is_empty() {
        return Command::new(bin);
    }
    if cfg!(windows) {
        let path_var = std::env::var("PATH").unwrap_or_default();
        if let Some(found) = crate::engines::find_on_path(default_bin, &path_var) {
            return Command::new(found);
        }
    }
    Command::new(default_bin)
}

/// Resolve the runtime for an instance by its `runtime.engine`. Unknown, empty,
/// or "dsh" all map to `DshRuntime` (backward compatible with pre-multi-engine
/// instance.json files).
pub fn for_instance(inst: &Instance) -> Box<dyn AgentRuntime> {
    match inst.runtime.engine.as_str() {
        "pi" => Box::new(model::PiRuntime),
        "omp" => Box::new(model::OmpRuntime),
        "claude" => Box::new(model::ClaudeRuntime),
        "codex" => Box::new(model::CodexRuntime),
        "opencode" => Box::new(model::OpencodeRuntime),
        _ => Box::new(model::DshRuntime),
    }
}

/// Read a built command's program and args as owned strings.
///
/// The executor needs this for real, not only in tests: a terminal session is
/// launched through a generated `sh` script ([`term::write_run_script`]), and a
/// script needs the argv as text rather than as a `Command`.
pub(crate) fn program_and_args(cmd: &Command) -> (String, Vec<String>) {
    let std = cmd.as_std();
    let program = std.get_program().to_string_lossy().into_owned();
    let args = std
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    (program, args)
}

/// Minimal `Instance` for the engine test suite: pick the engine, provider, and
/// model; everything else is a sensible default (profile "headless").
#[cfg(test)]
pub(crate) fn test_instance(engine: &str, provider: &str, model: &str) -> Instance {
    use crate::instance_manager::RuntimeConfig;
    Instance {
        schema_version: 1,
        id: "t".into(),
        name: "T".into(),
        icon: "bot".into(),
        group: "g".into(),
        description: String::new(),
        profile: "headless".into(),
        provider: provider.into(),
        model: model.into(),
        api_key_ref: String::new(),
        default_task: String::new(),
        runtime: RuntimeConfig {
            engine: engine.into(),
            ..RuntimeConfig::default()
        },
        created_at: "1970-01-01T00:00:00Z".into(),
    }
}
