# Configuration · 配置

> Engine, profile, model, plugins, credentials and `.env`.

## 引擎 · Engine

每个实例的 `runtime.engine` 任选其一：`dsh` · `pi` · `omp` · `claude` · `codex` · `opencode`（见 [Instance Anatomy](Instance-Anatomy#自由组合--框架--llm) 矩阵）。`设置` 面板通过 `detect_engines` 实时探测宿主已装 CLI；`custom_bin` 可覆盖为绝对路径。

- **仅 `dsh` 支持 web 服务**，其余 5 个引擎当前只跑 headless 一次性任务。
- `provider`/`model` 为空即省略对应 flag，让引擎用自身默认值。

## Profile · 运行档位（仅 `dsh`）

Profile 决定 `dsh` Agent 装载哪些插件包，**也决定它的运行形态**。`dsh` 的 profile 存放在 `$DSH_HOME`（默认 `~/.dsh`）下的 `profiles/<name>/`，核心是 `package.json` 的 `dsh.profile.bundles`：

| profile | bundles（简化） | 运行形态 |
|---|---|---|
| `web` | `dsh-base` + `dsh-web-app` | 长驻的浏览器 UI 服务 |
| `headless` | `dsh-base` + `dsh-headless` | 一次性任务（批处理 / 无人值守） |

> **运行形态从 引擎 + profile 派生**：仅 `dsh` 检查 bundles 是否含 `@deepseek-ai/dsh-web-app`（见 `src-tauri/src/runtime/dsh_home.rs::profile_is_web_capable`）。含 → 起 web 服务并开浏览器；其余情况（含非 `dsh` 引擎）均为一次性任务。**没有单独的 `run_mode` 字段。**

## 模型 · Model routing

- **`dsh`**：由 `agent-default-model` 插件的配置 `{ provider, model }` 决定，可在运行时用 `--patch <file>` 覆盖。启动器把实例 `instance.json` 里的 `model` 通过 `model.patch.yml` 传给 `dsh`：

```yaml
# model.patch.yml（示例，仅 dsh）
agent-default-model:
  provider: deepseek-official
  model: deepseek-v4-flash
```

- **其余引擎**：`pi`/`omp` 用 `--provider`/`--model`，`claude` 用 `--model`（provider 走 `ANTHROPIC_*` env），`codex` 用 `-c model[_provider]`，`opencode` 用 `-m [provider/]model`。详见 [Instance Anatomy](Instance-Anatomy#自由组合--框架--llm) 矩阵，空值即省略 flag。

## 凭据 · Credentials

- **按实例（通用）**：实例目录下的 `.env`（如 `DEEPSEEK_API_KEY=` / `ANTHROPIC_API_KEY=` 等），启动时注入子进程——所有 6 个引擎统一落点。
- **全局（仅 `dsh`）**：`~/.dsh/.credentials.yaml`（扁平 KV，权限 `0600`），`dsh` 专用。

> 🔐 密钥值**永不返回给前端 UI**；凭据文件保持 `0600`。切勿把密钥提交进 git。

## 插件 · Plugins (Hub)

- 插件 Hub 消费插件市场目录，提供**浏览 / 搜索 / 标签推荐**（移植在 `src/lib/market/`）。
- 选中的插件写入实例 `mcp.json` 的 `servers`。
- `dsh` 插件版本要与本机 `dsh` 匹配 —— 某些插件依赖较新的 `@deepseek-ai/dsh-environment`，旧版 `dsh` 会 `ERR_MODULE_NOT_FOUND`，移除该插件即可恢复；其余引擎的插件机制待接入。

## 分组与图标 · Groups & icons

`instance.json` 的 `group` 决定网格分组标题，`icon` 决定卡片图标 —— 纯展示，不影响运行。

**成员归属的唯一真相是 `instance.json.group`**；分组的**表现层**（组顺序、折叠状态、组内手动排序）另落在 `~/.agentlauncher/instgroups.json`，是一层覆盖而非真相——引用了过期实例会被静默忽略，缺席的实例按名追加，绝不因此把真实实例藏起来。详见 [Launcher Anatomy](Launcher-Anatomy)。
