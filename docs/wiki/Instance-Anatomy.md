# Instance Anatomy · 实例解剖

> One agent = one directory. No hidden global state.

每个 Agent 就是 `~/.agentlauncher/instances/<id>/` 下的一个目录。打开它，你看到的就是这个 Agent 的全部。

```text
~/.agentlauncher/instances/web-baseline/
├─ instance.json   # 元数据
├─ AGENTS.md       # System Prompt 与行为守则
├─ mcp.json        # MCP 插件配置
├─ .env            # 密钥与环境变量
├─ skills/         # 技能工具包
├─ workspace/      # 文件读写沙箱根
└─ logs/           # 输出与 Token 审计
```

## 文件逐个看 · File by file

### `instance.json`
实例元数据；前端网格卡片就是读它渲染的。示例：

```json
{
  "schema_version": 1,
  "id": "web-baseline",
  "name": "Web 基线",
  "icon": "globe",
  "group": "Web 交互",
  "profile": "web",
  "provider": "",
  "model": "deepseek-v4-flash",
  "runtime": { "engine": "dsh", "env_policy": "autodetect", "custom_bin": "" },
  "default_task": "",
  "created_at": "2026-08-27T08:41:05+00:00"
}
```

> `schema_version` 标记该文件的契约版本（缺失视为 `1`，向后兼容）。启动器级的契约见 [Launcher Anatomy](Launcher-Anatomy)。
>
> **已退役字段**：`temperature` / `thinking_budget` 曾被收集并落盘，但**没有任何框架 adapter 读过它们**——违反本项目「字段与消费者同时落地」的规矩，故删除而非补线。旧 `instance.json` 仍带着它们也能正常读取（未知键被忽略），因此**无需 `schema_version` 升版**。

#### 自由组合 · 框架 × LLM
一个实例 = 任选一个**框架**（`runtime.engine`，即哪个 Agent CLI）× 任选一个 **LLM**（顶层 `provider` + `model`）。二者正交、自由搭配：

- **`runtime.engine`** = 框架/CLI（缺失或空 → `dsh`，向后兼容）。当前已适配（如 `dsh` · `pi` · `omp` · `claude` · `codex` · `opencode` 等，持续扩展）：

  | engine | 程序 | headless 调用契约 | provider 注入 | model 注入 |
  |---|---|---|---|---|
  | `dsh` | `dsh` | 默认（非 web profile） | `model.patch.yml` 内 `provider`（空回退 `deepseek-official`） | `--patch model.patch.yml` |
  | `pi` | `pi` | `-p` | `--provider <p>` | `--model <m>` |
  | `omp` | `omp` | `-p` | `--provider <p>` | `--model <m>` |
  | `claude` | `claude` | `-p` | 环境变量（`ANTHROPIC_*`，走实例 `.env`），无 flag | `--model <m>` |
  | `codex` | `codex` | `exec` | `-c model_provider="<p>"` | `-c model="<m>"` |
  | `opencode` | `opencode` | `run` | 并入 `-m <p>/<m>` | `-m <p>/<m>`（无 provider 则 `-m <m>`） |

- **`provider` + `model`** = LLM 身份。**空值即省略对应 flag**——让所选框架用它自己的默认，不臆测。provider 命名空间**因框架而异**（`pi` 的 `google` ≠ `dsh` 的 `deepseek-official`），启动器只透传字符串、不做跨框架归一，UI 给每个框架一句格式提示。
- **各框架的 API Key 走各自的 env**（`pi`/`omp` 各自 env、`claude` 的 `ANTHROPIC_API_KEY`、`codex`/`opencode` 各自 env、`dsh` 走 `~/.dsh`）——统一落点即实例 `.env`（启动时注入子进程）。**密钥边界不变，不进任何契约文件。**
- **web（交互式）当前仅部分引擎支持**（如 `dsh`），其余以 headless 一次性任务运行。

> 这张矩阵不是口头承诺：`src-tauri/src/runtime/model_test.rs` 是「框架 × LLM」测试总表，`cargo test` 逐行断言已适配引擎真正 exec 出的 program + argv（含「空值即省略」与 `custom_bin` 覆盖），再真实建实例落盘、按 UI 读路径回读、由 `for_instance` 组命令行——全链路自动化；最后对宿主已装引擎跑只读 `--version` 存活探测（不消耗额度，未装自动跳过）。

#### `runtime` — 宿主适配（运行时/环境 Override）
启动器是从桌面环境启动的 GUI 进程，子进程默认继承它那份**被裁剪过的 PATH**——终端里能跑的 `dsh`（及其依赖的 node）从图标启动却可能 `无法启动`。`runtime` 就是每个实例对宿主环境的覆盖：

- **`engine`**：见上「自由组合」——决定 spawn 哪个 Agent CLI（多个 `AgentRuntime` 实现并列在 `src-tauri/src/runtime/model.rs` 一个文件里，可扩展；测试总表在 `runtime/model_test.rs`）。
- **`env_policy`**：
  - `autodetect`（默认）——启动前从**登录 shell** 探测 PATH 并并入子进程，让它看到与终端一致的工具链。**PATH 每次启动现探，绝不缓存落盘**（缓存会过期指向已删二进制）。
  - `isolated`——只给最小的确定性系统 PATH，不泄漏宿主整套工具链，面向可复现沙箱。
- **`custom_bin`**：非空时用它作为 Agent CLI 的绝对路径（覆盖 PATH 查找），其所在目录也会并入 PATH。

> 解析逻辑是**与 Agent 无关**的宿主职责，落在 `src-tauri/src/runtime/env.rs`，由 `executor` 在 spawn 前设置；`.env` 里显式的 `PATH` 仍最终覆盖（最显式者胜）。`runtime` 不含任何密钥。软墙权限（`runtime.permissions`）等尚未落地项见 [Launcher Anatomy](Launcher-Anatomy#roadmap--契约演进) 的路线图。

### `AGENTS.md`
该实例专属的 System Prompt 与行为守则（如「只在 `workspace/` 内读写」「高危命令先说明意图」）。

### `mcp.json`
启用的 MCP (Model Context Protocol) 插件。默认 `{"servers": {}}`，通过 **插件 Hub** 挑选后写入。

### `.env`
该实例专属的 API Keys / 环境变量，例如 `DEEPSEEK_API_KEY=`。**启动时注入子进程，永不回流到界面。**

### `skills/`
挂载的独立 Skill 工具包目录。

### `workspace/`
Agent 读写文件的**安全沙箱根**。路径稳定 → 会话历史与记忆天然沉淀于此，跨次启动继承。

### `logs/`
历史输出与 Token 消耗审计日志；也是**只读日志页**的数据来源。

## 隔离先于沙箱 · Isolation first

密钥、提示词、插件、工作目录全部**按实例隔离**。删除一个实例目录，就是干净地移除这个 Agent，不影响其它任何实例。
