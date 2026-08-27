# Configuration · 配置

> Profiles, models, plugins, credentials and `.env`.

## Profile · 运行档位

Profile 决定 Agent 装载哪些插件包，**也决定它的运行形态**。dsh 的 profile 存放在 `$DSH_HOME`（默认 `~/.dsh`）下的 `profiles/<name>/`，核心是 `package.json` 的 `dsh.profile.bundles`：

| profile | bundles（简化） | 运行形态 |
|---|---|---|
| `web` | `dsh-base` + `dsh-web-app` | 长驻的浏览器 UI 服务 |
| `headless` | `dsh-base` + `dsh-headless` | 一次性任务（批处理 / 无人值守） |

> **运行形态从 profile 派生**：启动器检查 bundles 是否含 `@deepseek-ai/dsh-web-app`（见 `src-tauri/src/dsh_config.rs::profile_is_web_capable`）。含 → 起 web 服务并开浏览器；否则跑一次性任务。**没有单独的 `run_mode` 字段。**

## 模型 · Model routing

模型路由由 `agent-default-model` 插件的配置 `{ provider, model }` 决定，可在运行时用 `--patch <file>` 覆盖。启动器把实例 `instance.json` 里的 `model` 通过 patch 文件传给 dsh：

```yaml
# model.patch.yml（示例）
agent-default-model:
  provider: deepseek-official
  model: deepseek-v4-flash
```

## 凭据 · Credentials

- **全局**：`~/.dsh/.credentials.yaml`（扁平 KV，权限 `0600`）。
- **按实例**：实例目录下的 `.env`（如 `DEEPSEEK_API_KEY=...`），启动时注入子进程。

> 🔐 密钥值**永不返回给前端 UI**；`.credentials.yaml` 保持 `0600`。切勿把密钥提交进 git。

## 插件 · Plugins (Hub)

- 插件 Hub 消费 `dsh.market` 的插件目录，提供**浏览 / 搜索 / 标签推荐**（移植在 `src/lib/market/`）。
- 选中的插件写入实例 `mcp.json` 的 `servers`。
- 版本要与本机 dsh 匹配 —— 某些插件依赖较新的 `@deepseek-ai/dsh-environment`，旧版 dsh 会 `ERR_MODULE_NOT_FOUND`，移除该插件即可恢复。

## 分组与图标 · Groups & icons

`instance.json` 的 `group` 决定网格分组标题，`icon` 决定卡片图标 —— 纯展示，不影响运行。
