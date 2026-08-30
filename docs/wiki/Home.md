# agentLauncher Wiki

> A universal launcher for isolated AI agent instances.

欢迎来到 **agentLauncher** 的文档。这里是把 [README](https://github.com/lildengzi/agentLauncher#readme) 之外的细节讲透的地方。

> 🔗 **仓库 Wiki 地址**：`https://github.com/lildengzi/agentLauncher/wiki` · 本地源文件在 [`agentlauncher/docs/wiki`](https://github.com/lildengzi/agentLauncher/tree/master/agentlauncher/docs/wiki) ，通过 `gh` 同步（外层 `docs/` 为离线设计稿，刻意不并入 Wiki）。

## 📚 导航 · Contents

| 页面 Page | 内容 |
|---|---|
| **[Getting Started](Getting-Started)** | 环境准备、构建、创建并启动第一个实例 |
| **[Instance Anatomy](Instance-Anatomy)** | 一个实例目录里的每个文件到底是什么 |
| **[Configuration](Configuration)** | profile · 模型 · 插件 · 凭据 · `.env` 怎么配 |
| **[Architecture](Architecture)** | Tauri 外壳 + 6 引擎 (`dsh`/`pi`/`omp`/`claude`/`codex`/`opencode`) + 前后端契约 |
| **[FAQ](FAQ)** | 常见问题与排障 |

## 🧭 一分钟理解 · TL;DR

- **它是什么**：一个 AI Agent 的图形化**启动器 / 管理器**（不是聊天客户端）。
- **它管什么**：实例（引擎 · profile / 模型 / 插件 / 凭据 / `.env`）、插件 Hub、设置。
- **交互在哪**：**`dsh` 的 web 实例**在 dsh 自己的网页里（启动器抓端口并开浏览器）；其余 5 个引擎以 headless 一次性任务运行，日志流式回显。
- **技术栈**：Tauri 2 (Rust) + Vue 3 + Vite + TailwindCSS；多引擎 `AgentRuntime` 可扩展（当前已适配 `dsh` 等）。

## 备注

实例隔离的交互灵感来自 [Prism Launcher](https://prismlauncher.org/)，本项目为独立实现，与 Prism Launcher / Mojang / DeepSeek 无隶属关系。