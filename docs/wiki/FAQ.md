# FAQ · 常见问题

### 它和直接用某个引擎的 CLI 有什么区别？

某个引擎的 CLI（如 `dsh` / `pi` / `claude`）是单体执行器，`agentLauncher` 是**图形化管理器**：帮你隔离、组织、一键启动多个 Agent 实例，并管理各自的引擎、模型、插件与密钥。以 `dsh` 的 web 实例为例，对话仍在 dsh 自己的网页里发生。

### 为什么启动器里没有内置聊天界面？

这是刻意的设计。启动器**只做管理，不做交互 UI** —— `dsh` web 实例的交互在 dsh 的网页，`pi`/`omp`/`claude`/`codex`/`opencode` 以 headless 任务运行、输出进只读日志页。内置聊天会与引擎自身 UI 重复，也违背“不做 UI”的初衷。

### 启动 `dsh` web 实例后浏览器没自动打开？

仅 `dsh` 有 web 模式。启动器执行 `dsh --profile web --no-open --port 0`，并从 stdout 抓 `http://127.0.0.1:<port>` 再调用系统 opener 打开。若没打开：
1. 确认该实例的 `runtime.engine` 为 `dsh` 且 profile 的 bundles 含 `@deepseek-ai/dsh-web-app`（否则会被当作 headless）。
2. 看**只读日志页**里子进程是否真的打印了 URL。
3. 手动复制日志里的 URL 到浏览器。

### 其余引擎（`pi`/`claude` 等）为什么没有网页？

本轮仅 `dsh` 接了 web 服务 (`is_serve`)；`pi`/`omp`/`claude`/`codex`/`opencode` 5 个引擎当前只接 headless 一次性任务（见 [Instance Anatomy](Instance-Anatomy#自由组合--框架--llm) 与 [Architecture](Architecture)）。

### 报 `ERR_MODULE_NOT_FOUND`？

通常是 `dsh` 某个插件的版本比本机 `dsh` 新，依赖了缺失的 `@deepseek-ai/dsh-environment`。用 `dsh plugin --profile <p> remove <plugin>` 移除该插件即可恢复（可再 `add` 回来）。非 `dsh` 引擎报类似错请检查对应 CLI 的插件/依赖版本。

### 我在磁盘上手动建了实例，为什么启动器里看不到？

实例列表在窗口挂载时读取一次。**重载一次窗口**即可看到新实例。

### 会话历史 / 记忆存在哪？会丢吗？

存在该实例的 `workspace/` 里。因为启动器给每个实例一个**稳定路径** `instances/<id>/workspace`，历史跨次启动继承，不会丢。删除实例目录才会清除。

### 密钥安全吗？

- 每实例的 `.env` 只在启动子进程时注入，**永不回流到前端**。
- `dsh` 的全局 `~/.dsh/.credentials.yaml` 保持 `0600`（仅 `dsh`）。
- 请勿把 `.env` / 凭据提交进 git。

### 支持 Windows / macOS 吗？

基于 Tauri 2，理论跨平台。目前主要在 Linux 上开发验证；其它平台需自行 `pnpm tauri build` 并测试。

### headless / 批处理什么时候能用？

已可用：除 `dsh` web 外，其余引擎均以 headless 一次性任务运行，stdout/stderr 流式进日志页。更复杂的无人值守编排见 README 路线图。
