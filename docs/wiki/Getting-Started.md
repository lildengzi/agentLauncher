# Getting Started · 快速上手

> Prereqs, build, and launching your first instance.

## 1. 前置条件 · Prerequisites

| 依赖 | 说明 |
|---|---|
| **Rust** (stable) | Tauri 后端编译所需。`rustup` 安装即可。 |
| **Node + pnpm** | 前端构建。仓库使用 `pnpm`（见 `pnpm-lock.yaml`）。 |
| **引擎 CLI（任选其一）** | `dsh` · `pi` · `omp` · `claude` · `codex` · `opencode` 中至少一个在 `PATH` 中（`设置` → `引擎` 可实时探测）。 |
| **对应 API Key** | 按引擎填入实例 `.env`（如 `DEEPSEEK_API_KEY` / `ANTHROPIC_API_KEY` 等），`dsh` 另支持全局 `~/.dsh/.credentials.yaml`。 |

> 启动器只是外壳，**没有已装引擎就无法跑起 Agent**。先确认 `dsh --help` / `pi --help` / `claude --help` 等任一可用。

## 2. 构建与运行 · Build & run

```bash
pnpm install          # 安装前端依赖
pnpm tauri dev        # 开发模式，拉起桌面窗口（热更新）
pnpm tauri build      # 打包为本机桌面应用
```

后端单独检查：

```bash
cd src-tauri && cargo build
```

## 3. 创建第一个实例 · Your first instance

1. 点顶部 **添加实例 (Add instance)**。
2. 填名称、图标、分组；选 **profile**（如 `web`）与**模型**（如 `deepseek-v4-flash`）。
3. 保存 —— 启动器会在 `~/.agentlauncher/instances/<id>/` 下生成目录结构（`instance.json`、`AGENTS.md`、`.env`、`mcp.json`、`workspace/`、`skills/`、`logs/`）。

## 4. 启动 · Launch

- 选中实例，点右侧 **启动 (Launch)**。
- 若是 **`dsh` 的 web** 实例：启动器执行 `dsh --profile web --no-open --port 0`，从 stdout 抓到 `http://127.0.0.1:<port>` 并自动用系统浏览器打开，你在 dsh 自己的网页里对话。
- 若是 **其余引擎（`pi`/`omp`/`claude`/`codex`/`opencode`）**：以 headless 一次性任务运行（分别 ` -p` / `exec` / `run`），stdout/stderr 全程流式进只读日志页。
- 会话历史沉淀在该实例的 `workspace/` 中，跨次启动继承；点 **结束进程 (Stop)** 关闭。

> 💡 想要多个专精 Agent？调好一个基线实例，用 **复制 (Clone)** 派生出 `frontend-agent`、`backend-agent`，只改 profile 与模型即可。
