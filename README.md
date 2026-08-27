<div align="center">

# 🔷 agentLauncher

**Manage AI agents the way Prism Launcher manages Minecraft instances.**

[![Tauri](https://img.shields.io/badge/Tauri-2.0-24C8DB?logo=tauri&logoColor=white)](https://tauri.app)
[![Vue](https://img.shields.io/badge/Vue-3-42b883?logo=vuedotjs&logoColor=white)](https://vuejs.org)
[![Rust](https://img.shields.io/badge/Rust-stable-000000?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![Engine](https://img.shields.io/badge/engine-DeepSeek%20Harness%20(dsh)-5b9dff)](docs/wiki/Architecture.md)
![Status](https://img.shields.io/badge/status-MVP%20v0.1.0-e8963a)

**[中文](README_zh.md)**

</div>

---

## 📸 Screenshots

| Dashboard | Plugin Hub | Edit Instance |
|-----------|------------|---------------|
| <img src="docs/images/mainPage.png" alt="Dashboard" width="300"> | <img src="docs/images/pluginsPage.png" alt="Plugin Hub" width="300"> | <img src="docs/images/edit.png" alt="Edit Instance" width="300"> |
| Grid cards, action panel, status bar | Search, tags, scored picks | General, model, prompt |

---

> One engine, a whole shelf of agents.

`agentLauncher` is a graphical launcher for AI agents, built on **Prism Launcher's isolation philosophy**. It treats each agent as an isolated *instance* — its own sandboxed directory, model, plugins and keys — and runs it with a single **Launch**.

It only **manages**; it never intercepts the chat: interaction happens in **dsh's own web page**.

### ✨ Highlights

| | Feature | What it does |
|---|---|---|
| 🧱 | **Sandboxed instances** | Each agent gets its own directory; `workspace/` is the safe root it reads and writes within. |
| 🔐 | **Per-instance keys & .env** | Keys are injected per instance and never reach the UI; the credentials file stays `0600`. Local-first. |
| 🧩 | **Plugin Hub** | Browse, search and get tag-based recommendations — pick capabilities per instance, modpack-style. |
| 🌐 | **Launch to the web UI** | Web instances auto-start the server, grab a port and open the browser — you chat in dsh's own page. |
| 📜 | **Read-only log view** | Streaming stdout flows into a read-only log page for reviewing tool calls and reasoning. |
| 🌱 | **Clone from baseline** | Tune one baseline, then clone it into front-end / back-end / batch agents — just swap profile and model. |

### 🧩 Anatomy of an instance

One agent = one directory. No hidden global state — open the folder and you see everything that agent is:

```text
~/.dsh-launcher/instances/web-baseline/
├─ instance.json   # metadata: name · icon · group · model · temperature · thinking_budget
├─ AGENTS.md       # this instance's system prompt & rules
├─ mcp.json        # enabled MCP (Model Context Protocol) plugins
├─ .env            # this instance's API keys & env vars (isolated injection)
├─ skills/         # mounted skill packs
├─ workspace/      # safe file sandbox root (session history settles here too)
└─ logs/           # output & token-usage audit
```

### 🚀 Quick start

**Prereqs**: Rust toolchain · Node + pnpm · the [DeepSeek Harness (`dsh`) CLI](https://github.com/deepseek-ai) (on your `PATH`, with `DEEPSEEK_API_KEY` set).

```bash
pnpm install          # install frontend deps
pnpm tauri dev        # dev mode, opens the desktop window
pnpm tauri build      # bundle the desktop app
```

1. **Install dsh first** — the launcher is only a shell; DeepSeek Harness does the work.
2. **Create an instance** — hit *Add instance*, pick the web profile and a model; the folder is created under `~/.dsh-launcher/`.
3. **Launch & chat** — press *Launch*; the browser opens dsh's web page. History stays in the instance's `workspace/`.

> 📄 A GitHub-Pages-ready landing page ships in the repo: [`docs/landing.html`](docs/landing.html).

### 🏗 How it works

A light shell that runs the engine. **Tauri 2 (Rust)** manages subprocesses, the file sandbox and credentials; the **DeepSeek Harness (`dsh` CLI)** is the actual engine.

- **Run shape is derived from the profile**: if the profile's `bundles` include `@deepseek-ai/dsh-web-app`, the launcher starts a web server, grabs a port and opens the browser; otherwise it runs a one-shot task.
- **Front/back contract**: `src/types.ts` mirrors the Rust structs; `src/lib/api.ts` wraps every `invoke` command and the `dsh-log` / `dsh-status` events.

### 🙏 Acknowledgements

- **[Prism Launcher](https://prismlauncher.org/)** — inspiration for the instance-isolation philosophy and UI layout.
- **DeepSeek Harness (`dsh`)** — the underlying agent execution engine.

> ⚠️ The UI is heavily inspired by Prism Launcher, but agentLauncher is an **independent project**, not affiliated with Prism Launcher, Mojang, or DeepSeek.