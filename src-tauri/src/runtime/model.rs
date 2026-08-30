//! The six engine adapters — one file, one table.
//!
//! Every agent CLI differs only in *how the launch command is assembled*: the
//! binary name, the flag that makes the run non-interactive, and how provider /
//! model / task are injected. That is a handful of lines each, so a file per
//! engine hid the very thing it was meant to expose. Side by side the six
//! `build_command` bodies read as the matrix they are, and adding an engine is one
//! `impl` here plus rows in `model_test.rs` (which fails if a registered engine
//! has no rows).
//!
//! Each adapter answers twice, because a run has two shapes (see
//! [`super::RunMode`]): `build_interactive` — the CLI as its authors ship it,
//! a session the user talks to — and `build_command`, the same selection plus
//! whichever flag makes it answer once and exit. **The headless flag is the only
//! difference**, and it is different for every engine, which is why the pair
//! lives here rather than in the executor.
//!
//! Invariants across all six:
//!
//! * **An empty provider or model omits its flag** — the engine then falls back to
//!   its own default instead of the launcher guessing one.
//! * **Provider strings pass through verbatim — with exactly one exception.**
//!   Namespaces are engine-specific (pi's `google` is not dsh's `deepseek-official`)
//!   and the launcher does not normalize across engines. dsh is the exception
//!   because it is the one engine whose provider is not a free-form flag but a
//!   *registered route*: an unregistered name makes `agent-default-model`
//!   unresolvable and the run fails as 模型不可用. So dsh's provider is checked
//!   against the routes that exist and refused if it names none — see [`dsh_route`].
//! * **No credentials here.** Each CLI reads its own env, injected from the
//!   instance `.env` by the executor.
//! * **`custom_bin` wins over the default binary** (see [`super::program`]).
//!
//! | engine | program | headless | provider | model |
//! |---|---|---|---|---|
//! | dsh | `dsh` | default (non-web profile) | `provider:` inside `model.patch.yml` | `--patch <file>` |
//! | pi | `pi` | `-p` | `--provider <p>` | `--model <m>` |
//! | omp | `omp` | `-p` | `--provider <p>` | `--model <m>` |
//! | claude | `claude` | `-p` | env only (`ANTHROPIC_*`), no flag | `--model <m>` |
//! | codex | `codex` | `exec` | `-c model_provider=<p>` | `-c model=<m>` |
//! | opencode | `opencode` | `run` | folded into `-m <p>/<m>` | `-m <p>/<m>` |

use tokio::process::Command;

use super::{dsh_home, program, AgentRuntime, RunMode, SpawnRequest};
use crate::instance_manager::Instance;

// ---------------------------------------------------------------- dsh

/// dsh (DeepSeek Harness) — the launcher's first-class agent and the only engine
/// with a web serve mode. `dsh --profile <p> [--patch <model.patch.yml>]
/// "<task>"`, where the patch overrides the `agent-default-model` plugin config
/// for this instance's model.
pub struct DshRuntime;

impl AgentRuntime for DshRuntime {
    fn id(&self) -> &'static str {
        "dsh"
    }

    fn build_command(&self, req: &SpawnRequest) -> Result<Command, String> {
        let mut cmd = self.build_interactive(req)?;
        if dsh_home::profile_is_web_capable(&req.instance.profile) {
            // Web profile: a long-running browser-UI server, not a one-shot task.
            // Pass no task; keep the launcher from hijacking the browser
            // (`--no-open` — the launcher opens the URL itself once it appears on
            // stdout), and let the OS pick a free port (`--port 0`) so multiple
            // web instances never collide on the default 3080.
            cmd.arg("--no-open").arg("--port").arg("0");
        } else {
            cmd.arg(req.task);
        }
        Ok(cmd)
    }

    /// `dsh --profile <p> [--patch …]` and nothing else: dsh boots the profile's
    /// own app, and a trailing task is what turns that app into a one-shot run
    /// (`dsh --profile headless "run the tests"`). Which is also why dsh's default
    /// mode is [`RunMode::Task`] — for dsh the profile *is* the shape, so a
    /// profile called `headless` should keep answering one task, and a profile
    /// carrying a TUI app becomes a session by booting with no arguments.
    fn build_interactive(&self, req: &SpawnRequest) -> Result<Command, String> {
        let patch_path = write_model_patch(req)?;
        let mut cmd = program(&req.instance.runtime.custom_bin, "dsh");
        cmd.arg("--profile").arg(&req.instance.profile);
        if let Some(p) = &patch_path {
            cmd.arg("--patch").arg(p);
        }
        Ok(cmd)
    }

    fn default_mode(&self) -> RunMode {
        RunMode::Task
    }

    fn is_serve(&self, inst: &Instance) -> bool {
        dsh_home::profile_is_web_capable(&inst.profile)
    }
}

/// Escape a scalar for a YAML double-quoted string.
fn yaml_quote(s: &str) -> String {
    let esc = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{esc}\"")
}

/// Resolve an instance's `provider` onto a dsh **route** — the name
/// `agent-default-model`'s `provider` field must carry.
///
/// These are two namespaces, and conflating them is what made a dsh instance report
/// 模型不可用: `providers.json` ids (`deepseek`, `free-api`, `mcapple`) say where the
/// launcher keeps a key, while a route is a name registered on dsh's own `ctx.llm`.
/// The launcher's `deepseek` row and dsh's `deepseek-official` route are the same
/// vendor reached two ways, so that one pair is aliased; everything else must match a
/// route dsh actually has ([`dsh_home::model_routes`]).
///
/// An unknown provider is refused here instead of being written into the patch. The
/// alternative is a launch that boots, resolves nothing, and blames the model — and
/// the launcher is the only side of that exchange that knows both namespaces.
fn dsh_route(provider: &str) -> Result<String, String> {
    let p = provider.trim();
    if p.is_empty() {
        // No provider ⇒ dsh's own wiring, which is what pre-provider instances did.
        return Ok(dsh_home::NATIVE_ROUTE.to_string());
    }
    let routes = dsh_home::model_routes();
    if routes.iter().any(|r| r == p) {
        return Ok(p.to_string());
    }
    // The launcher's own DeepSeek row names the vendor dsh reaches natively.
    if p == "deepseek" {
        return Ok(dsh_home::NATIVE_ROUTE.to_string());
    }
    Err(format!(
        "dsh 没有名为「{p}」的模型路由，可用的是：{}。\n\
         这里要填的是 dsh 自己的 provider route，不是启动器「设置 → 模型与 API」里的服务商 id——\
         两者是两套名字。除 {} 之外的路由由 dsh 自己的设置文档（$DSH_HOME/settings.yaml 的 \
         llm-pi-ai: providers:）注册，dsh 网页版的 Models 页写的就是它。",
        routes.join("、"),
        dsh_home::NATIVE_ROUTE,
    ))
}

/// Write a `--patch` overlay overriding `agent-default-model` for this instance's
/// model, or `None` when the instance has no model set.
///
/// Both fields are always written: the plugin's schema requires `provider` *and*
/// `model` (`z.string().required()` on each), and a loader patch **replaces** the
/// targeted row's whole `config` rather than merging into it — so a patch carrying
/// only `model` would leave the row invalid and fail dsh's boot instead of the run.
fn write_model_patch(req: &SpawnRequest) -> Result<Option<String>, String> {
    let model = req.instance.model.trim();
    if model.is_empty() {
        return Ok(None);
    }
    let provider = dsh_route(&req.instance.provider)?;
    let body = format!(
        "# Generated by agentLauncher — maps this instance's model onto dsh.\n\
         - id: agent-default-model\n  \
           config:\n    \
             provider: {}\n    \
             model: {}\n",
        yaml_quote(&provider),
        yaml_quote(model),
    );
    let path = req.instance_dir.join("model.patch.yml");
    std::fs::write(&path, body).map_err(|e| e.to_string())?;
    Ok(Some(path.to_string_lossy().to_string()))
}

// ------------------------------------------------------- pi family

/// The shared pi-family shape: `<bin> [-p] [--provider <p>] [--model <m>]
/// ["<task>"]`, where `-p/--print` is the non-interactive switch. `pi` and `omp`
/// (oh-my-pi) are the same CLI lineage and differ only in the binary name.
fn pi_family(bin: &'static str, req: &SpawnRequest, headless: bool) -> Command {
    let mut cmd = program(&req.instance.runtime.custom_bin, bin);
    if headless {
        cmd.arg("-p");
    }
    let provider = req.instance.provider.trim();
    if !provider.is_empty() {
        cmd.arg("--provider").arg(provider);
    }
    let model = req.instance.model.trim();
    if !model.is_empty() {
        cmd.arg("--model").arg(model);
    }
    if headless {
        cmd.arg(req.task);
    }
    cmd
}

/// pi (pi-coding-agent) — a TUI session by default, one-shot under `-p`. Discovers
/// AGENTS.md from the working tree, so the executor's `cwd` placement is all the
/// context it needs.
pub struct PiRuntime;

impl AgentRuntime for PiRuntime {
    fn id(&self) -> &'static str {
        "pi"
    }

    fn build_command(&self, req: &SpawnRequest) -> Result<Command, String> {
        Ok(pi_family("pi", req, true))
    }

    fn build_interactive(&self, req: &SpawnRequest) -> Result<Command, String> {
        Ok(pi_family("pi", req, false))
    }
}

/// omp (oh-my-pi) — identical flag shape to `pi`.
pub struct OmpRuntime;

impl AgentRuntime for OmpRuntime {
    fn id(&self) -> &'static str {
        "omp"
    }

    fn build_command(&self, req: &SpawnRequest) -> Result<Command, String> {
        Ok(pi_family("omp", req, true))
    }

    fn build_interactive(&self, req: &SpawnRequest) -> Result<Command, String> {
        Ok(pi_family("omp", req, false))
    }
}

// ------------------------------------------------- claude / codex / opencode

/// claude (Claude Code) — "starts an interactive session by default, use
/// -p/--print for non-interactive output", in its own help's words. No provider
/// flag exists: provider, base URL, and key all come from `ANTHROPIC_*` in the
/// instance `.env`. Discovers AGENTS.md / CLAUDE.md from the working tree.
pub struct ClaudeRuntime;

impl ClaudeRuntime {
    fn args(req: &SpawnRequest, headless: bool) -> Command {
        let mut cmd = program(&req.instance.runtime.custom_bin, "claude");
        if headless {
            cmd.arg("-p");
        }
        let model = req.instance.model.trim();
        if !model.is_empty() {
            cmd.arg("--model").arg(model);
        }
        if headless {
            cmd.arg(req.task);
        }
        cmd
    }
}

impl AgentRuntime for ClaudeRuntime {
    fn id(&self) -> &'static str {
        "claude"
    }

    fn build_command(&self, req: &SpawnRequest) -> Result<Command, String> {
        Ok(Self::args(req, true))
    }

    fn build_interactive(&self, req: &SpawnRequest) -> Result<Command, String> {
        Ok(Self::args(req, false))
    }
}

/// codex — interactive when no subcommand is given ("options will be forwarded to
/// the interactive CLI"), one-shot under `exec`. Model and provider ride in as
/// `-c` config overrides either way; codex parses each value as JSON with a string
/// fallback, so bare ids like `o3` / `gpt-5-codex` pass through unquoted. The
/// provider id must reference a `model_providers.<id>` table in the user's
/// `~/.codex/config.toml` — black-box passthrough, the launcher does not validate.
pub struct CodexRuntime;

impl CodexRuntime {
    fn args(req: &SpawnRequest, headless: bool) -> Command {
        let mut cmd = program(&req.instance.runtime.custom_bin, "codex");
        if headless {
            cmd.arg("exec");
        }
        let model = req.instance.model.trim();
        if !model.is_empty() {
            cmd.arg("-c").arg(format!("model={model}"));
        }
        let provider = req.instance.provider.trim();
        if !provider.is_empty() {
            cmd.arg("-c").arg(format!("model_provider={provider}"));
        }
        if headless {
            cmd.arg(req.task);
        }
        cmd
    }
}

impl AgentRuntime for CodexRuntime {
    fn id(&self) -> &'static str {
        "codex"
    }

    fn build_command(&self, req: &SpawnRequest) -> Result<Command, String> {
        Ok(Self::args(req, true))
    }

    fn build_interactive(&self, req: &SpawnRequest) -> Result<Command, String> {
        Ok(Self::args(req, false))
    }
}

/// opencode — the TUI is its default command, one-shot under `run`. It takes a
/// single `provider/model` string, so the two halves fold into one `-m` (the same
/// flag in both shapes); with no provider we pass the bare model and let opencode
/// resolve it against its own default. (`opencode web` exists but is deliberately
/// not wired — web stays dsh-only.)
pub struct OpencodeRuntime;

impl OpencodeRuntime {
    fn args(req: &SpawnRequest, headless: bool) -> Command {
        let mut cmd = program(&req.instance.runtime.custom_bin, "opencode");
        if headless {
            cmd.arg("run");
        }
        let provider = req.instance.provider.trim();
        let model = req.instance.model.trim();
        if !model.is_empty() {
            let m = if provider.is_empty() {
                model.to_string()
            } else {
                format!("{provider}/{model}")
            };
            cmd.arg("-m").arg(m);
        }
        if headless {
            cmd.arg(req.task);
        }
        cmd
    }
}

impl AgentRuntime for OpencodeRuntime {
    fn id(&self) -> &'static str {
        "opencode"
    }

    fn build_command(&self, req: &SpawnRequest) -> Result<Command, String> {
        Ok(Self::args(req, true))
    }

    fn build_interactive(&self, req: &SpawnRequest) -> Result<Command, String> {
        Ok(Self::args(req, false))
    }
}
