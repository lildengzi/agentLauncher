# Instance Anatomy · 实例解剖

> One agent = one directory. No hidden global state.

每个 Agent 就是 `~/.dsh-launcher/instances/<id>/` 下的一个目录。打开它，你看到的就是这个 Agent 的全部。

```text
~/.dsh-launcher/instances/web-baseline/
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
  "id": "web-baseline",
  "name": "Web 基线",
  "icon": "globe",
  "group": "Web 交互",
  "profile": "web",
  "model": "deepseek-v4-flash",
  "temperature": 0.2,
  "thinking_budget": 4096,
  "default_task": "",
  "created_at": "2026-08-27T08:41:05+00:00"
}
```

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
