//! Agent runtime abstraction (P0).
//!
//! The launcher's host logic — process supervision, log streaming, the kill
//! channel, instance storage — is agent-agnostic. The only agent-specific piece
//! of a run is *how the launch command is assembled*: binary name, argument
//! shape, how the task text is passed, and any per-run config overlay.
//! `AgentRuntime` captures exactly that seam, so a second runtime (Claude Code,
//! P1) becomes a new impl rather than edits scattered through the executor.
//!
//! P0 is a pure move: only the existing dsh spawn behavior is extracted into
//! `DshRuntime`. Config / capability surfaces stay where they are until a second
//! runtime needs them (渐进泛化 — see docs/tradeoff.md §六).

mod dsh;

use std::path::Path;
use tokio::process::Command;

use crate::instance_manager::Instance;

pub use dsh::DshRuntime;

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
    /// Build the headless one-shot command for this instance and task, writing
    /// any per-run config files under `req.instance_dir`. The executor adds the
    /// working directory, environment, and stdio wiring before spawning.
    fn build_command(&self, req: &SpawnRequest) -> Result<Command, String>;
}

/// Resolve the runtime for an instance. Today every instance is dsh; when a
/// second runtime lands (P1) this switches on an instance-level runtime field.
pub fn for_instance(_inst: &Instance) -> Box<dyn AgentRuntime> {
    Box::new(DshRuntime)
}
