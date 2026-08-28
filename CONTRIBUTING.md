# Contributing to agentLauncher

感谢你考虑为 agentLauncher 做贡献！本文档帮助你快速上手开发流程。

[中文版](CONTRIBUTING_zh.md) | English

## 开发环境

| 依赖 | 版本要求 |
|------|----------|
| Rust | stable (via rustup) |
| Node.js | >= 18 |
| pnpm | >= 8 |
| dsh CLI | 已加入 `PATH`，并配置 `DEEPSEEK_API_KEY` |

```bash
pnpm install          # 安装前端依赖
pnpm tauri dev        # 启动开发窗口（热重载）
pnpm build            # 前端构建检查 (vue-tsc + vite)
pnpm tauri build      # 打包桌面应用
```

## 项目结构

```
src/                  # Vue 3 前端
src-tauri/            # Tauri 2 / Rust 后端 (instance_manager, executor, runtime)
src/types.ts          # 前后端共享类型（镜像 Rust 结构体）
src/lib/api.ts        # 所有 Tauri invoke 封装 + 事件监听
docs/wiki/            # Wiki 源文件，同步到 GitHub Wiki
```

## 图标与品牌资产

`app-icon.svg`（仓库根）是 logo 的**唯一母版**，矢量、任意分辨率。`app-icon.png`（1024×1024）
只是它的渲染产物，也是 `tauri icon` 的默认输入；`src-tauri/icons/` 下的整套图标全部由它派生，
**不要手改**。换 logo 的流程是：

```bash
# 1. 替换母版 app-icon.svg，2. 重渲 1024 PNG（保持 83.8% 画面占比、居中）
rsvg-convert -w 3432 app-icon.svg -o /tmp/logo-4x.png
magick /tmp/logo-4x.png -trim +repage -resize 858x \
  -background none -gravity center -extent 1024x1024 PNG32:app-icon.png
# 3. 生成整套桌面图标，并删掉本项目不发布的移动端产物
pnpm exec tauri icon app-icon.png
rm -rf src-tauri/icons/{android,ios} src-tauri/icons/64x64.png
```

`tauri.conf.json` 的 `bundle.icon` 只引用 `32x32 / 128x128 / 128x128@2x / icon.icns / icon.ico`，
其余 `Square*Logo.png` 供 Windows Appx 使用。README 开头的 logo 直接引用
`src-tauri/icons/128x128@2x.png`，不另存副本——换 logo 后 README 自动跟着变。

## 分支与提交规范

- 分支命名：`feat/<scope>` / `fix/<scope>` / `docs/<scope>` / `chore/<scope>`
- 提交信息遵循 [Conventional Commits](https://www.conventionalcommits.org/)：
  ```
  feat(hub): add tag-based recommendation
  fix(executor): handle dsh non-zero exit
  docs(readme): update quick start
  ```
- 每个 PR 尽量聚焦单一改动，保持可 review。

## Pull Request 流程

1. Fork 仓库并创建特性分支。
2. 本地完成开发与自测（`pnpm build` 通过）。
3. 提交前确保 `cargo fmt` / `cargo check` 无明显告警（如涉及 Rust）。
4. 发起 PR，填写模板中的必要信息，关联相关 Issue（如有）。
5. 等待 CI 通过与 Review，通过后由维护者 squash merge。

## Issue 指南

- 提交 Bug 前请先搜索已有 Issue，避免重复。
- 使用对应的 Issue 模板（Bug Report / Feature Request），提供复现步骤、期望行为、环境信息。
- 功能建议请说明使用场景与价值，欢迎附上草图或参考实现。

## 代码风格

- 前端：Vue 3 `<script setup>` + TypeScript 严格模式，Tailwind CSS。
- 后端：`rustfmt` 默认风格，避免 `unwrap()` 在生产路径上，优先返回 `Result`。
- 前后端契约以 `src/types.ts` 为准，修改 Rust 结构体后同步更新。

## 行为准则

请尊重每一位贡献者，保持友善、专业的交流氛围。 harassment 或不友好的行为将不会被容忍。

## License

提交 PR 即表示你同意你的贡献将以 [MIT License](LICENSE) 同协议发布。
