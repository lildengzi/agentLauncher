# Launcher Anatomy · 启动器解剖

> The launcher has a contract too — and it lives on disk, next to the instances.

[实例契约](Instance-Anatomy)管的是「单个 Agent」。**启动器契约**管的是启动器自身：界面偏好、全局默认值、会话状态、侧栏分组。它们同样**落盘、带版本、由后端拥有、在 `src/types.ts` 镜像**——不再散落在 `localStorage` 或内存 `ref` 里。

```text
~/.agentlauncher/
├─ config.json       # 启动器配置（有版本）
├─ instgroups.json   # 侧栏分组的表现层覆盖（有版本）
└─ instances/        # 各实例目录（见 Instance Anatomy）
```

两个文件都由 `src-tauri/src/launcher_config.rs` 拥有，命令 `get/set_launcher_config`、`get/set_inst_groups` 收口读写。**缺失或损坏时回退内置默认值**，绝不因为一个坏文件把启动器卡死。

## `config.json` — 启动器配置

```jsonc
{
  "format_version": 1,
  "ui":       { "theme": "catppuccin-mocha", "locale": "zh" },
  "defaults": { "profile": "headless", "provider": "deepseek",
                "base_url": "https://api.deepseek.com",
                "model": "deepseek-reasoner" },
  "session":  { "selected_instance": "web-baseline", "last_used_group": "Web" }
}
```

- **`ui`**：主题与语言。原先分别存在 `localStorage` 的 `agentlauncher.theme` / `agentlauncher.locale`，现收口于此；`localStorage` 仅保留一份极小缓存供首屏秒画，真相是本文件。
- **`defaults`**：新建实例对话框的预填值（**非密钥**）。原 `agentlauncher.modelConfig` 的非密钥部分迁移至此。
- **`session`**：跨启动恢复的瞬态 UX——上次选中的实例、上次使用的分组。

> 🔐 **密钥永不进入本文件。** API Key 仍归运行时所有，写在 `~/.dsh/.credentials.yaml`（权限 `0600`，见 [Configuration](Configuration#凭据--credentials)）。启动器契约只*引用*这条边界，不复制账号库。

## `instgroups.json` — 分组表现层（覆盖，非真相）

```jsonc
{
  "format_version": 1,
  "order": ["未分类", "Web"],
  "groups": {
    "Web": { "collapsed": false, "instances": ["web-baseline", "test-agent"] }
  }
}
```

- **`order`**：侧栏分组的上→下顺序。
- **`groups[name].collapsed`**：该分组是否折叠（现在**跨重启保留**，不再是内存态）。
- **`groups[name].instances`**：组内的**手动排序**覆盖。

**混合模型 · 谁拥有什么：** 成员归属的**唯一真相**始终是每个 `instance.json` 的 `group` 字段。本文件只是一层表现覆盖（顺序 / 折叠 / 组内排序），实例因此保持自描述，迁移代价最小。

**健壮性铁律：绝不让过期文件把真实实例藏起来。**

- 覆盖里引用了已删除的实例 / 分组 → **静默忽略**。
- 某个实例 / 分组在覆盖里缺席 → **回退按名排序并追加**到末尾。
- 渲染永远以「当前真实实例列表」为基准做重排，而非以覆盖文件为基准做筛选（见 `src/lib/instGroups.ts::applyOverlay`）。

## 通用铁律 · Invariants

- **万物带版本**：`config.json` / `instgroups.json` 有 `format_version`，`instance.json` 有 `schema_version`（缺失视为 `1`，向后兼容）。
- **各域分文件**：UI 偏好、分组表现、实例数据、密钥各自独立。
- **后端拥有 + 前端镜像**：结构体定义在 Rust，`src/types.ts` 一一对应，改一边同步另一边。
- **密钥独立于配置**：凭据只属运行时 `~/.dsh`。

## Roadmap · 契约演进

北极星是「通用 Agent 启动器」，但遵循**渐进泛化**：**契约字段与它的消费者同时落地**，绝不预留今天没人读的空字段。以下要素**刻意推迟**，各自等到功能真要做时连同实现一起补：

- **`runtime.permissions`（软墙）**：`{ mode, allowed_commands, denied_commands }` 只有在存在**强制点**时才有意义；命令由 dsh 子进程内部执行，启动器目前无法拦截，需 dsh 侧提供可消费的权限入口后再定契约。
- **`custom_python`**：dsh 是 Node 运行时，暂无消费者；待某引擎确实需要独立 Python/venv 路径时再加。
- **共享 `cache/`**：MCP 插件 / Skill 的下载目前由 dsh / npx 负责，启动器不自行下载，无缓存对象。
- **`model{}` 嵌套**：把扁平的 `model`/`temperature`/`thinking_budget` 收进对象是纯重构，需 `schema_version` 1→2 迁移，收益低，暂不动。

> 记住：这些不是「缺失」，而是**待消费者**。加字段的门槛始终是「今天有谁读它」。

### 已落地 · 曾在路线图上

- **`runtime.engine`（多引擎）**：已落地。`runtime/mod.rs::for_instance` 按 `runtime.engine` 分发到 6 个 `AgentRuntime` 实现（`dsh`/`pi`/`omp`/`claude`/`codex`/`opencode`，见 [Instance Anatomy](Instance-Anatomy#自由组合--框架--llm) 的调用矩阵）。字段与 6 个消费者同时到位，正是「渐进泛化」的落地示范。
- **宿主引擎探测**：原计划的 **`engines.json`**（引擎路径缓存）**有意不落盘**——改由 `detect_engines` 命令**每次实时探测** PATH（复用 `runtime/env.rs` 的登录 shell 逻辑），与「PATH 现探不缓存」同一铁律：缓存会过期指向已删二进制。
