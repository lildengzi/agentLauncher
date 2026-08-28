# 贡献指南

> English version: [CONTRIBUTING.md](CONTRIBUTING.md)

感谢你对 agentLauncher 感兴趣！

## 快速开始

```bash
pnpm install
pnpm tauri dev        # 开发模式
pnpm build            # 前端类型检查 + 构建
```

前置依赖：Rust stable · Node >=18 · pnpm >=8 · `dsh` CLI（`PATH` 可用且已设 `DEEPSEEK_API_KEY`）

## 提 Issue

- 先搜索是否已有相同问题
- 选用对应模板：Bug 反馈 / 功能建议
- 描述清晰：复现步骤、期望结果、实际结果、环境（OS / dsh 版本 / app 版本）

## 提 PR

1. Fork → 新建分支 `feat/xxx` 或 `fix/xxx`
2. 小步提交，commit 遵循 Conventional Commits
3. `pnpm build` 通过；改 Rust 需 `cargo fmt` + `cargo check`
4. 关联 Issue（如有），按 PR 模板填写
5. 等待 CI 与 Review

## 其他说明

- 保持 PR 聚焦，单一职责
- 修改 Rust 结构后同步更新 `src/types.ts`
- logo 母版是根目录 `app-icon.svg`（矢量），`app-icon.png` 与 `src-tauri/icons/*` 均由它派生，
  不要手改单张图标；重生成步骤见 [CONTRIBUTING.md](CONTRIBUTING.md#图标与品牌资产)
- 贡献代码即视为同意以 MIT 协议发布
