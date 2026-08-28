# agentLauncher Wiki

> *Manage AI agents the way Prism Launcher manages Minecraft instances.*
> 像 Prism Launcher 管理 Minecraft 实例一样，管理你的 AI Agent。

欢迎来到 **agentLauncher** 的文档。这里是把 [README](https://github.com/lildengzi/agentLauncher#readme) 之外的细节讲透的地方。

> 🔗 **仓库 Wiki 地址**：`https://github.com/lildengzi/agentLauncher/wiki` · 本地源文件在 [`dsh-launcher/docs/wiki`](https://github.com/lildengzi/agentLauncher/tree/master/dsh-launcher/docs/wiki) ，通过 `gh` 同步（外层 `docs/` 为离线设计稿，刻意不并入 Wiki）。

## 📚 导航 · Contents

| 页面 Page | 内容 |
|---|---|
| **[Getting Started](Getting-Started)** | 环境准备、构建、创建并启动第一个实例 |
| **[Instance Anatomy](Instance-Anatomy)** | 一个实例目录里的每个文件到底是什么 |
| **[Configuration](Configuration)** | profile · 模型 · 插件 · 凭据 · `.env` 怎么配 |
| **[Architecture](Architecture)** | Tauri 外壳 + dsh 引擎 + 前后端契约 |
| **[FAQ](FAQ)** | 常见问题与排障 |

## 🧭 一分钟理解 · TL;DR

- **它是什么**：一个 AI Agent 的图形化**启动器 / 管理器**（不是聊天客户端）。
- **它管什么**：实例（profile / 模型 / 插件 / 凭据 / `.env`）、插件 Hub、设置。
- **交互在哪**：在 **dsh 自己的网页** 里 —— 启动器只负责把它跑起来并打开浏览器。
- **技术栈**：Tauri 2 (Rust) + Vue 3 + Vite + TailwindCSS；引擎是 DeepSeek Harness (`dsh`)。

## ⚠️ 关于灵感来源 · A note on inspiration

界面与「实例隔离」理念深受 [Prism Launcher](https://prismlauncher.org/) 启发。agentLauncher 是一个**独立项目**，与 Prism Launcher、Mojang、DeepSeek 官方均无隶属关系。