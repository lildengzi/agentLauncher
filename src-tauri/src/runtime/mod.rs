//! Agent runtime abstraction.
//!
//! The launcher's host logic — process supervision, log streaming, the kill
//! channel, instance storage — is agent-agnostic. The only agent-specific piece
//! of a run is *how the launch command is assembled*: binary name, argument
//! shape, how the task text is passed, and any per-run config overlay.
//! `AgentRuntime` captures exactly that seam, so each additional CLI is a new
//! impl rather than edits scattered through the executor.
//!
//! Today six runtimes exist: `DshRuntime` (first-class, the only one with a web
//! serve mode) plus the headless-only `pi`/`omp`/`claude`/`codex`/`opencode`
//! adapters. All six live in `model.rs` — one file, one table — and
//! `for_instance` dispatches on `instance.runtime.engine`; their contract is
//! asserted row by row in `model_test.rs`.
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

use std::path::Path;
use tokio::process::Command;

use crate::instance_manager::Instance;

/// Everything a runtime needs to assemble a one-shot (headless) launch command.
/// The executor supplies working directory, environment, and stdio afterwards.
pub struct SpawnRequest<'a> {
    /// The instance being launched.
    pub instance: &'a Instance,
    /// The instance's on-disk directory, for writing per-run config overlays.
    pub instance_dir: &'a Path,
    /// The resolved task text for this run.
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

    /// Whether this instance runs a long-running server whose URL the executor
    /// should watch stdout for (web mode), rather than a one-shot task. Only
    /// dsh's web-capable profiles return true today.
    fn is_serve(&self, _inst: &Instance) -> bool {
        false
    }
}

/// Build a `Command` for `default_bin`, or for `custom_bin` when it is non-empty
/// (its directory has already been put on PATH by the executor; see runtime::env).
fn program(custom_bin: &str, default_bin: &str) -> Command {
    let bin = custom_bin.trim();
    if bin.is_empty() {
        Command::new(default_bin)
    } else {
        Command::new(bin)
    }
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

/// Read a built command's program and args as owned strings — a small helper for
/// the engine test suite (`model_test`).
#[cfg(test)]
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
        default_task: String::new(),
        runtime: RuntimeConfig {
            engine: engine.into(),
            ..RuntimeConfig::default()
        },
        created_at: "1970-01-01T00:00:00Z".into(),
    }
}
