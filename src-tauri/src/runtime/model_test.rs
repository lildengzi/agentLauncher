//! The 「框架 × LLM」 test suite — one place for the whole free-combination matrix.
//!
//! Each engine adapter used to be its own file carrying its own `#[cfg(test)] mod
//! tests`. The adapters now sit side by side in `model.rs` and their assertions
//! side by side here, so the contract reads as a single table: adding an engine
//! means adding rows, not a new test module, and a new engine that nobody wrote
//! rows for fails `every_known_engine_is_covered`.
//!
//! Four layers, all automated — no human step, no API key, no quota spent:
//!   1. `argv_matrix` / `interactive_argv_matrix` — what
//!      `for_instance(..).build_command(..)` and `..build_interactive(..)` actually
//!      exec, per engine, including the "空值即省略 flag" and `custom_bin` rules.
//!      Plus `an_unset_mode_asks_the_engine_and_an_explicit_one_wins`, which pins
//!      the third thing a run needs: which of the two commands gets built.
//!   2. `dispatch` — engine id → runtime, and which engines serve (web) vs run
//!      one-shot.
//!   3. `create_instance_per_engine_then_build_from_disk` — creates six real
//!      instances under a throwaway HOME, reads them back through the same path
//!      the UI uses, and builds each launch command from what actually landed on
//!      disk: create → persist → reload → dispatch → argv.
//!   4. `installed_engines_are_runnable` — spawns the real binary the launcher
//!      would exec, via each CLI's read-only `--version`. This is the automated
//!      replacement for the old manual "launch all six from the GUI" step; it
//!      costs no quota, and engines absent from the host are reported and
//!      skipped so the suite stays green anywhere.

use std::path::Path;
use std::time::Duration;

use super::{for_instance, program_and_args, test_instance, RunMode, SpawnRequest};
use crate::engines::known_engines;
use crate::instance_manager::{self, NewInstance, RuntimeConfig};
use crate::test_support::{temp_tree, EnvGuard, TempTree, DSH_HOME_LOCK, HOME_LOCK};

/// Task text used across the argv table.
const TASK: &str = "hello";

/// Stand-in for the absolute `model.patch.yml` path dsh receives — substituted
/// with the real per-run path before comparing.
const PATCH: &str = "{patch}";

/// One expected command line: engine + LLM selection in, program + argv out.
struct Argv {
    what: &'static str,
    engine: &'static str,
    provider: &'static str,
    model: &'static str,
    custom_bin: &'static str,
    prog: &'static str,
    args: &'static [&'static str],
}
/// The full matrix. Two shapes per engine at minimum: everything supplied, and
/// nothing supplied plus a `custom_bin` override (proving both the omit rule and
/// that the per-instance binary replaces the PATH lookup).
const ARGV_CASES: &[Argv] = &[
    // dsh — model travels in a --patch overlay, not on the command line.
    Argv {
        what: "dsh headless, no model ⇒ no --patch",
        engine: "dsh",
        provider: "",
        model: "",
        custom_bin: "",
        prog: "dsh",
        args: &["--profile", "headless", TASK],
    },
    Argv {
        what: "dsh headless with model ⇒ --patch overlay",
        engine: "dsh",
        provider: "",
        model: "deepseek-reasoner",
        custom_bin: "",
        prog: "dsh",
        args: &["--profile", "headless", "--patch", PATCH, TASK],
    },
    Argv {
        what: "dsh honors custom_bin",
        engine: "dsh",
        provider: "",
        model: "",
        custom_bin: "/opt/tools/dsh/bin/dsh",
        prog: "/opt/tools/dsh/bin/dsh",
        args: &["--profile", "headless", TASK],
    },
    // pi / omp — same pi-family shape: -p, then optional --provider/--model.
    Argv {
        what: "pi with provider + model",
        engine: "pi",
        provider: "google",
        model: "gemini-2.0-flash",
        custom_bin: "",
        prog: "pi",
        args: &[
            "-p",
            "--provider",
            "google",
            "--model",
            "gemini-2.0-flash",
            TASK,
        ],
    },
    Argv {
        what: "pi omits empty flags, honors custom_bin",
        engine: "pi",
        provider: "",
        model: "",
        custom_bin: "/opt/pi/bin/pi",
        prog: "/opt/pi/bin/pi",
        args: &["-p", TASK],
    },
    Argv {
        what: "omp with provider + model",
        engine: "omp",
        provider: "openai",
        model: "gpt-4.1",
        custom_bin: "",
        prog: "omp",
        args: &["-p", "--provider", "openai", "--model", "gpt-4.1", TASK],
    },
    Argv {
        what: "omp omits empty flags, honors custom_bin",
        engine: "omp",
        provider: "",
        model: "",
        custom_bin: "/opt/omp/bin/omp",
        prog: "/opt/omp/bin/omp",
        args: &["-p", TASK],
    },
    // claude — no provider flag exists; ANTHROPIC_* come from the instance .env.
    Argv {
        what: "claude takes the model flag but never the provider",
        engine: "claude",
        provider: "anthropic",
        model: "claude-sonnet-4",
        custom_bin: "",
        prog: "claude",
        args: &["-p", "--model", "claude-sonnet-4", TASK],
    },
    Argv {
        what: "claude omits empty model, honors custom_bin",
        engine: "claude",
        provider: "",
        model: "",
        custom_bin: "/home/u/.local/bin/claude",
        prog: "/home/u/.local/bin/claude",
        args: &["-p", TASK],
    },
    // codex — model/provider ride in as `-c` config overrides under `exec`.
    Argv {
        what: "codex exec with both config overrides",
        engine: "codex",
        provider: "openai",
        model: "o3",
        custom_bin: "",
        prog: "codex",
        args: &[
            "exec",
            "-c",
            "model=o3",
            "-c",
            "model_provider=openai",
            TASK,
        ],
    },
    Argv {
        what: "codex omits empty overrides, honors custom_bin",
        engine: "codex",
        provider: "",
        model: "",
        custom_bin: "/opt/codex/bin/codex",
        prog: "/opt/codex/bin/codex",
        args: &["exec", TASK],
    },
    // opencode — one `-m provider/model` string; bare model when no provider.
    Argv {
        what: "opencode folds provider into -m provider/model",
        engine: "opencode",
        provider: "anthropic",
        model: "claude-sonnet-4",
        custom_bin: "",
        prog: "opencode",
        args: &["run", "-m", "anthropic/claude-sonnet-4", TASK],
    },
    Argv {
        what: "opencode passes a bare model when no provider is set",
        engine: "opencode",
        provider: "",
        model: "some-model",
        custom_bin: "",
        prog: "opencode",
        args: &["run", "-m", "some-model", TASK],
    },
    Argv {
        what: "opencode omits -m entirely, honors custom_bin",
        engine: "opencode",
        provider: "",
        model: "",
        custom_bin: "/opt/opencode/bin/opencode",
        prog: "/opt/opencode/bin/opencode",
        args: &["run", TASK],
    },
];

/// The same matrix for the *interactive* shape — the CLI as its authors ship it.
///
/// Every row is the headless row minus two things: the flag that suppresses the
/// session (`-p` / `exec` / `run`, and for dsh the trailing task) and the task
/// text. The model / provider selection is unchanged, which is the property worth
/// asserting: switching an instance to 交互式 must not quietly drop the LLM it was
/// pointed at, and the flag that does the switching is different for all six.
const INTERACTIVE_CASES: &[Argv] = &[
    Argv {
        what: "dsh boots the profile with no task at all",
        engine: "dsh",
        provider: "",
        model: "",
        custom_bin: "",
        prog: "dsh",
        args: &["--profile", "headless"],
    },
    Argv {
        what: "dsh keeps the model overlay when interactive",
        engine: "dsh",
        provider: "",
        model: "deepseek-reasoner",
        custom_bin: "",
        prog: "dsh",
        args: &["--profile", "headless", "--patch", PATCH],
    },
    Argv {
        what: "pi drops -p and the task, keeps the LLM",
        engine: "pi",
        provider: "google",
        model: "gemini-2.0-flash",
        custom_bin: "",
        prog: "pi",
        args: &["--provider", "google", "--model", "gemini-2.0-flash"],
    },
    Argv {
        what: "omp bare TUI, custom_bin still wins",
        engine: "omp",
        provider: "",
        model: "",
        custom_bin: "/opt/omp/bin/omp",
        prog: "/opt/omp/bin/omp",
        args: &[],
    },
    Argv {
        what: "omp interactive with provider + model",
        engine: "omp",
        provider: "free-api",
        model: "nvidia/llama-3.3-nemotron",
        custom_bin: "",
        prog: "omp",
        args: &[
            "--provider",
            "free-api",
            "--model",
            "nvidia/llama-3.3-nemotron",
        ],
    },
    Argv {
        what: "claude interactive is the bare binary plus the model",
        engine: "claude",
        provider: "anthropic",
        model: "claude-sonnet-4",
        custom_bin: "",
        prog: "claude",
        args: &["--model", "claude-sonnet-4"],
    },
    Argv {
        what: "claude with nothing set is the bare binary",
        engine: "claude",
        provider: "",
        model: "",
        custom_bin: "",
        prog: "claude",
        args: &[],
    },
    Argv {
        what: "codex drops the exec subcommand, keeps the -c overrides",
        engine: "codex",
        provider: "openai",
        model: "o3",
        custom_bin: "",
        prog: "codex",
        args: &["-c", "model=o3", "-c", "model_provider=openai"],
    },
    Argv {
        what: "opencode drops the run subcommand, keeps -m",
        engine: "opencode",
        provider: "anthropic",
        model: "claude-sonnet-4",
        custom_bin: "",
        prog: "opencode",
        args: &["-m", "anthropic/claude-sonnet-4"],
    },
];
/// A `DSH_HOME` with no profiles at all, so `profile_is_web_capable` answers the
/// same on every machine no matter what the developer has installed locally.
fn neutral_dsh_home(tree: &TempTree) -> EnvGuard {
    let home = tree.path().join("dsh-home");
    std::fs::create_dir_all(&home).unwrap();
    EnvGuard::set("DSH_HOME", &home)
}

/// Layer 1 — every row of the matrix, through the same `for_instance` seam the
/// executor uses.
#[test]
fn argv_matrix() {
    let _g = DSH_HOME_LOCK.lock().unwrap();
    let tree = temp_tree("argv");
    let _dsh = neutral_dsh_home(&tree);

    for c in ARGV_CASES {
        check_argv(c, tree.path(), false);
    }
}

/// Layer 1, interactive half — the same seam, the other command.
#[test]
fn interactive_argv_matrix() {
    let _g = DSH_HOME_LOCK.lock().unwrap();
    let tree = temp_tree("argv-tui");
    let _dsh = neutral_dsh_home(&tree);

    for c in INTERACTIVE_CASES {
        check_argv(c, tree.path(), true);
    }
    // The task text is not merely unused, it is absent: an interactive command
    // that still carried it would answer once and exit, which is precisely the
    // behaviour this mode exists to stop.
    for c in INTERACTIVE_CASES {
        assert!(!c.args.contains(&TASK), "{}: 交互式不该带任务文本", c.what);
    }
}

/// Build one row's command and compare program + argv.
fn check_argv(c: &Argv, dir: &Path, interactive: bool) {
    let mut inst = test_instance(c.engine, c.provider, c.model);
    inst.runtime.custom_bin = c.custom_bin.into();
    let agent = for_instance(&inst);
    let req = SpawnRequest {
        instance: &inst,
        instance_dir: dir,
        task: if interactive { "" } else { TASK },
    };
    let cmd = if interactive {
        agent.build_interactive(&req)
    } else {
        agent.build_command(&req)
    }
    .unwrap_or_else(|e| panic!("{}: build failed: {e}", c.what));

    let (prog, args) = program_and_args(&cmd);
    assert_eq!(prog, c.prog, "{}: program", c.what);
    let want: Vec<String> = c
        .args
        .iter()
        .map(|a| {
            if *a == PATCH {
                dir.join("model.patch.yml").to_string_lossy().into_owned()
            } else {
                (*a).to_string()
            }
        })
        .collect();
    assert_eq!(args, want, "{}: argv", c.what);
}
/// The table must keep pace with the engine catalog in both directions: a newly
/// registered engine with no rows fails here rather than shipping untested. Both
/// halves count — an engine with a headless row and no interactive one would ship
/// a mode nobody checked.
#[test]
fn every_known_engine_is_covered() {
    for spec in known_engines() {
        assert!(
            ARGV_CASES.iter().any(|c| c.engine == spec.id),
            "engine {} is in known_engines() but has no argv case",
            spec.id
        );
        assert!(
            INTERACTIVE_CASES.iter().any(|c| c.engine == spec.id),
            "engine {} is in known_engines() but has no interactive case",
            spec.id
        );
    }
    for c in ARGV_CASES.iter().chain(INTERACTIVE_CASES) {
        assert!(
            known_engines().iter().any(|s| s.id == c.engine),
            "argv case names engine {}, which is not in known_engines()",
            c.engine
        );
    }
}

/// An empty `runtime.mode` — which is what every instance.json written before the
/// field existed says — must resolve to the engine's own default, not to a guess
/// baked into the executor. The five CLI agents open a session; dsh answers a task,
/// because for dsh the profile is the shape.
#[test]
fn an_unset_mode_asks_the_engine_and_an_explicit_one_wins() {
    let _g = DSH_HOME_LOCK.lock().unwrap();
    let tree = temp_tree("mode");
    let _dsh = neutral_dsh_home(&tree);

    for spec in known_engines() {
        let mut inst = test_instance(spec.id, "", "");
        inst.runtime.mode = String::new();
        let agent = for_instance(&inst);
        let want = if spec.id == "dsh" {
            RunMode::Task
        } else {
            RunMode::Interactive
        };
        assert_eq!(
            RunMode::resolve(&inst, agent.as_ref()),
            want,
            "{}: unset mode",
            spec.id
        );

        // Either value, spelled out, overrides that default for every engine.
        for (stored, want) in [
            ("task", RunMode::Task),
            ("interactive", RunMode::Interactive),
        ] {
            inst.runtime.mode = stored.into();
            assert_eq!(
                RunMode::resolve(&inst, agent.as_ref()),
                want,
                "{}: stored mode {stored}",
                spec.id
            );
        }
        // Anything unrecognized falls back to the engine default rather than
        // failing a launch — same rule as `runtime.engine` and `env_policy`.
        inst.runtime.mode = "sideways".into();
        assert_eq!(
            RunMode::resolve(&inst, agent.as_ref()),
            want,
            "{}: junk",
            spec.id
        );
    }
}

/// Layer 2 — engine id on disk picks the matching runtime.
#[test]
fn dispatch_by_engine_id() {
    for spec in known_engines() {
        let inst = test_instance(spec.id, "", "");
        assert_eq!(for_instance(&inst).id(), spec.id);
    }
    // Empty and unknown both fall back to dsh: an instance.json written before
    // multi-engine has no runtime.engine at all and must keep launching dsh.
    assert_eq!(for_instance(&test_instance("", "", "")).id(), "dsh");
    assert_eq!(for_instance(&test_instance("nope", "", "")).id(), "dsh");
}
/// web (serve) mode is dsh-only this round, and only for a profile that actually
/// bundles the web app — the profile name alone means nothing.
#[test]
fn only_a_dsh_web_profile_serves() {
    let _g = DSH_HOME_LOCK.lock().unwrap();
    let tree = temp_tree("serve");
    let dsh_home = tree.path().join("dsh-home");
    let prof = dsh_home.join("profiles").join("web");
    std::fs::create_dir_all(&prof).unwrap();
    std::fs::write(
        prof.join("package.json"),
        r#"{"dsh":{"profile":{"bundles":["@deepseek-ai/dsh-base","@deepseek-ai/dsh-web-app"]}}}"#,
    )
    .unwrap();
    let _dsh = EnvGuard::set("DSH_HOME", &dsh_home);

    // The headless engines are one-shot even on a profile named "web".
    for spec in known_engines().iter().filter(|s| !s.web) {
        let mut inst = test_instance(spec.id, "", "");
        inst.profile = "web".into();
        assert!(
            !for_instance(&inst).is_serve(&inst),
            "{} must never serve",
            spec.id
        );
    }

    // dsh on a web-capable profile: the long-running browser-UI server, no task.
    // `--no-open` leaves the browser to the launcher (it scrapes the URL from
    // stdout) and `--port 0` keeps two web instances off the same default port.
    let mut inst = test_instance("dsh", "", "");
    inst.profile = "web".into();
    let rt = for_instance(&inst);
    assert!(rt.is_serve(&inst));
    let cmd = rt
        .build_command(&SpawnRequest {
            instance: &inst,
            instance_dir: tree.path(),
            task: "ignored for a serve run",
        })
        .unwrap();
    let (prog, args) = program_and_args(&cmd);
    assert_eq!(prog, "dsh");
    assert_eq!(args, ["--profile", "web", "--no-open", "--port", "0"]);

    // Same engine, a profile without the web bundle ⇒ back to one-shot.
    inst.profile = "headless".into();
    assert!(!for_instance(&inst).is_serve(&inst));
}
/// dsh carries the LLM choice in a generated `--patch` overlay rather than on the
/// command line, so the overlay's contents are part of the contract.
#[test]
fn dsh_patch_carries_provider_and_model() {
    let _g = DSH_HOME_LOCK.lock().unwrap();
    let tree = temp_tree("patch");
    let _dsh = neutral_dsh_home(&tree);
    let patch_path = tree.path().join("model.patch.yml");

    // No provider ⇒ the official DeepSeek route, so dsh instances created before
    // the provider field existed keep behaving exactly as they did.
    let inst = test_instance("dsh", "", "deepseek-reasoner");
    for_instance(&inst)
        .build_command(&SpawnRequest {
            instance: &inst,
            instance_dir: tree.path(),
            task: TASK,
        })
        .unwrap();
    let patch = std::fs::read_to_string(&patch_path).unwrap();
    assert!(patch.contains("agent-default-model"), "{patch}");
    assert!(
        patch.contains(r#"provider: "deepseek-official""#),
        "{patch}"
    );
    assert!(patch.contains(r#"model: "deepseek-reasoner""#), "{patch}");

    // A route dsh actually has passes through.
    let inst = test_instance("dsh", "deepseek-official", "deepseek-v4-pro");
    for_instance(&inst)
        .build_command(&SpawnRequest {
            instance: &inst,
            instance_dir: tree.path(),
            task: TASK,
        })
        .unwrap();
    let patch = std::fs::read_to_string(&patch_path).unwrap();
    assert!(
        patch.contains(r#"provider: "deepseek-official""#),
        "{patch}"
    );
    assert!(patch.contains(r#"model: "deepseek-v4-pro""#), "{patch}");

    // The launcher's own `deepseek` row is the same vendor as dsh's native route,
    // and every dsh instance the New Instance dialog creates carries that id (it
    // comes from providers.json), so this alias is the common case, not a courtesy.
    let inst = test_instance("dsh", "deepseek", "deepseek-v4-flash");
    for_instance(&inst)
        .build_command(&SpawnRequest {
            instance: &inst,
            instance_dir: tree.path(),
            task: TASK,
        })
        .unwrap();
    let patch = std::fs::read_to_string(&patch_path).unwrap();
    assert!(
        patch.contains(r#"provider: "deepseek-official""#),
        "启动器的 deepseek 应映射到 dsh 的原生路由: {patch}"
    );
}

/// The other half of the same contract: a provider dsh has no route for fails the
/// build, and the way to *give* it one is dsh's own settings document.
///
/// This is the bug the user hit — a dsh instance carrying the launcher's `free-api`
/// id wrote `provider: "free-api"` into the overlay, dsh resolved no such route, and
/// the failure surfaced as 模型不可用 with nothing pointing at the field.
#[test]
fn dsh_refuses_a_provider_that_is_not_a_route_until_settings_declare_it() {
    let _g = DSH_HOME_LOCK.lock().unwrap();
    let tree = temp_tree("routes");
    let home = tree.path().join("dsh-home");
    std::fs::create_dir_all(&home).unwrap();
    let _dsh = EnvGuard::set("DSH_HOME", &home);

    let inst = test_instance("dsh", "free-api", "nvidia/nemotron-3-ultra-550b-a55b");
    let err = for_instance(&inst)
        .build_command(&SpawnRequest {
            instance: &inst,
            instance_dir: tree.path(),
            task: TASK,
        })
        .expect_err("未注册的路由必须在写 patch 之前失败");
    // The message has to name both the offending value and the namespace, because
    // the user's next move is to fix one field and they cannot see either list.
    assert!(err.contains("free-api"), "{err}");
    assert!(err.contains("deepseek-official"), "{err}");
    assert!(err.contains("settings.yaml"), "{err}");
    assert!(
        !tree.path().join("model.patch.yml").exists(),
        "失败的解析不该留下一份 dsh 解不开的 overlay"
    );

    // `llm-pi-ai: providers:` is what registers extra routes (dsh's own web Models
    // page writes that section) — so with one declared, the same instance builds.
    std::fs::write(
        home.join("settings.yaml"),
        "llm-pi-ai:\n  providers:\n    free-api:\n      apiKeyEnv: FREE_API_API_KEY\n",
    )
    .unwrap();
    for_instance(&inst)
        .build_command(&SpawnRequest {
            instance: &inst,
            instance_dir: tree.path(),
            task: TASK,
        })
        .unwrap();
    let patch = std::fs::read_to_string(tree.path().join("model.patch.yml")).unwrap();
    assert!(patch.contains(r#"provider: "free-api""#), "{patch}");
}

/// A plausible provider/model pair per engine for the on-disk round-trip. The
/// launcher never validates these strings (black-box passthrough, no cross-engine
/// normalization) — they only have to survive write → read → build_command intact.
fn llm_for(engine: &str) -> (&'static str, &'static str) {
    match engine {
        "dsh" => ("deepseek-official", "deepseek-reasoner"),
        "pi" => ("google", "gemini-2.0-flash"),
        "omp" => ("openai", "gpt-4.1"),
        "claude" => ("anthropic", "claude-sonnet-4"),
        "codex" => ("openai", "o3"),
        "opencode" => ("anthropic", "claude-sonnet-4"),
        other => panic!("no test LLM pair for engine {other}"),
    }
}
/// Layer 3 — the whole chain the GUI walks, minus the GUI: create one instance
/// per engine under a throwaway HOME, reload it through the UI's own read path,
/// and assemble the launch command from what actually landed on disk.
#[test]
fn create_instance_per_engine_then_build_from_disk() {
    let _h = HOME_LOCK.lock().unwrap();
    let _d = DSH_HOME_LOCK.lock().unwrap();
    let tree = temp_tree("engines-home");
    let _home = EnvGuard::set("HOME", tree.path());
    let _dsh = neutral_dsh_home(&tree);

    for spec in known_engines() {
        let (provider, model) = llm_for(spec.id);
        let created = instance_manager::create_instance(NewInstance {
            name: format!("{} probe", spec.id),
            icon: "bot".into(),
            group: "自动化".into(),
            description: String::new(),
            profile: "headless".into(),
            provider: provider.into(),
            model: model.into(),
            api_key_ref: String::new(),
            default_task: String::new(),
            runtime: RuntimeConfig {
                engine: spec.id.into(),
                ..RuntimeConfig::default()
            },
        })
        .unwrap_or_else(|e| panic!("create_instance({}) failed: {e}", spec.id));

        let dir = instance_manager::instance_dir(&created.id).unwrap();
        assert!(
            dir.starts_with(tree.path().join(".agentlauncher")),
            "{}: instance dir {dir:?} must live under ~/.agentlauncher",
            spec.id
        );
        // What landed on disk: the snake_case contract, the engine recorded, and
        // no credentials — keys belong in the instance `.env`, never in the
        // contract file.
        let raw = std::fs::read_to_string(dir.join("instance.json")).unwrap();
        for field in [
            "\"engine\"",
            "\"env_policy\"",
            "\"provider\"",
            "\"schema_version\"",
            "\"api_key_ref\"",
        ] {
            assert!(
                raw.contains(field),
                "{}: instance.json missing {field}",
                spec.id
            );
        }
        let lower = raw.to_lowercase();
        for leak in ["apikey", "credential", "sk-"] {
            assert!(
                !lower.contains(leak),
                "{}: instance.json must stay secret-free (found {leak})",
                spec.id
            );
        }
        // `api_key_ref` names a key stored in `providers.json`; it is a reference, and
        // the only `api_key`-shaped thing allowed here. Any other spelling would mean
        // a value had crept into the contract file.
        assert!(
            !lower.replace("\"api_key_ref\"", "").contains("api_key"),
            "{}: instance.json must stay secret-free (found api_key)",
            spec.id
        );

        // Reload through the same path the UI reads, then dispatch and build.
        let stored = instance_manager::get_instance(&created.id).unwrap();
        assert_eq!(stored.runtime.engine, spec.id, "engine must round-trip");
        assert_eq!(stored.provider, provider, "provider must round-trip");
        assert_eq!(stored.model, model, "model must round-trip");

        let rt = for_instance(&stored);
        assert_eq!(
            rt.id(),
            spec.id,
            "the engine on disk must pick its own runtime"
        );
        assert!(
            !rt.is_serve(&stored),
            "{}: a headless profile is one-shot",
            spec.id
        );

        let cmd = rt
            .build_command(&SpawnRequest {
                instance: &stored,
                instance_dir: &dir,
                task: "probe",
            })
            .unwrap_or_else(|e| panic!("{}: build_command failed: {e}", spec.id));
        let (prog, args) = program_and_args(&cmd);
        assert_eq!(prog, spec.default_bin, "{}: program", spec.id);
        assert_eq!(
            args.last().map(String::as_str),
            Some("probe"),
            "{}: the task must be the final positional in {args:?}",
            spec.id
        );
        // The chosen model must actually reach the engine: on the command line
        // for everyone but dsh, which routes it through the patch overlay.
        if spec.id == "dsh" {
            let patch = std::fs::read_to_string(dir.join("model.patch.yml"))
                .expect("dsh must write the model patch next to the instance");
            assert!(patch.contains(model) && patch.contains(provider), "{patch}");
        } else {
            assert!(
                args.iter().any(|a| a.contains(model)),
                "{}: model {model} missing from {args:?}",
                spec.id
            );
        }
    }

    // All six coexist, and the UI's list path sees every one of them.
    let listed = instance_manager::list_instances().unwrap();
    assert_eq!(
        listed.len(),
        known_engines().len(),
        "one instance per engine"
    );
    let mut got: Vec<&str> = listed.iter().map(|i| i.runtime.engine.as_str()).collect();
    got.sort_unstable();
    let mut want: Vec<&str> = known_engines().iter().map(|s| s.id).collect();
    want.sort_unstable();
    assert_eq!(got, want);
}

/// Run `<bin> --version` with a hard timeout. Read-only: it reaches no API and
/// spends no quota, so it is safe on every `cargo test`.
async fn version_of(bin: &str) -> Result<String, String> {
    let run = tokio::process::Command::new(bin)
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .output();
    match tokio::time::timeout(Duration::from_secs(60), run).await {
        Err(_) => Err("`--version` timed out".into()),
        Ok(Err(e)) => Err(format!("spawn failed: {e}")),
        Ok(Ok(out)) if !out.status.success() => Err(format!(
            "exited with {:?}: {}",
            out.status.code(),
            String::from_utf8_lossy(&out.stderr).trim()
        )),
        Ok(Ok(out)) => {
            let mut s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if s.is_empty() {
                s = String::from_utf8_lossy(&out.stderr).trim().to_string();
            }
            Ok(s)
        }
    }
}
/// Layer 4 — the automated stand-in for the old manual "launch all six from the
/// GUI" step. A real end-to-end run would spend real API quota and need six
/// logins, so instead this proves the two halves a test *can* prove for free:
/// detection resolves through the launcher's own enriched PATH, and the exact
/// binary `for_instance` would exec is present and actually runs. Engines the
/// host does not have are reported and skipped, so the suite is green anywhere.
#[tokio::test]
// The guard is held across the awaits on purpose: HOME must stay put for the
// whole probe. Safe here — `#[tokio::test]` runs one current-thread task, so
// nothing else in this future can contend for the lock.
#[allow(clippy::await_holding_lock)]
async fn installed_engines_are_runnable() {
    // Detection probes a login shell, which reads the real HOME — keep the tests
    // that swap HOME out of the way.
    let _h = HOME_LOCK.lock().unwrap();

    let found = crate::engines::detect_engines().await;
    assert_eq!(
        found.len(),
        known_engines().len(),
        "detection must report every known engine, installed or not"
    );

    let mut ran = 0usize;
    let mut skipped: Vec<&str> = Vec::new();
    for info in &found {
        if !info.installed {
            skipped.push(&info.id);
            continue;
        }
        assert!(
            Path::new(&info.path).is_file(),
            "{}: detected path {} is not a file",
            info.id,
            info.path
        );
        match version_of(&info.path).await {
            Ok(v) => {
                assert!(!v.is_empty(), "{}: --version printed nothing", info.id);
                eprintln!(
                    "engine {:<9} runnable · {} · {}",
                    info.id,
                    v.lines().next().unwrap_or_default(),
                    info.path
                );
                ran += 1;
            }
            Err(e) => panic!(
                "{}: the binary the launcher would exec is not runnable — {e}",
                info.id
            ),
        }
    }
    eprintln!(
        "liveness: {ran}/{} engines runnable; not installed: {skipped:?}",
        found.len()
    );
}
