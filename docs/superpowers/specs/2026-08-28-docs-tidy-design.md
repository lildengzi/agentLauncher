# Docs Tidy — 根目录按语言分区整理 (方案 A) Design

> Date: 2026-08-28 | Author: agentLauncher | Status: Draft (pending review)

## 1. 背景与问题

根目录当前 `*.md` 4 个 + `LICENSE` 并列：
`README.md:1` / `README_zh.md:1` / `CONTRIBUTING.md:1` / `CONTRIBUTING_zh.md:1` / `LICENSE:1`，加上 `index.html:1`、`docs/landing.html:1` 等，审计视角“顶层太乱”。

GitHub 约束：
- `README.md` 必须根（首页展示）。
- `LICENSE` 必须根（license 检测）。
- `CONTRIBUTING.md` 可在 `/` / `/.github/` / `/docs/` 均被识别，但 `/.github/` 为最原生、Issue/PR 侧边自动提示。

目标：**仅治根、按语言分区、零破环**。后续 `docs/` 内可自然扩展 `en/`/`zh/`，本次只动 `CONTRIBUTING`。

非目标：不改 `docs/wiki/`、`docs/images/`、`docs/landing.html`；不改 `.github/workflows/`；不引入中英文全文重排。

## 2. 设计方案 (方案 A — 已选)

### 2.1 最终目录

```
./README.md                      # 保留，EN 主场
./README_zh.md                   # 保留，ZH 入口
./LICENSE                        # 保留，MIT
./.github/CONTRIBUTING.md         # EN canonical（GitHub 自动发现）
./.github/CONTRIBUTING_zh.md      # ZH（或 docs/zh/CONTRIBUTING.md 二选一，本设计选前者，理由见 §2.2）
./.github/ISSUE_TEMPLATE/*        # 已存在，保持
./.github/pull_request_template.md # 已存在，保持
./docs/superpowers/specs/...      # 设计文档（本次新增）
```

**根 `*.md` 从 4 → 2**（README×2），若计入 LICENSE 则 5→3，视觉极简。

### 2.2 为什么 `.github/` 承载双语而非 `docs/zh/`

- 选择 `.github/CONTRIBUTING_zh.md`：与 `CONTRIBUTING.md` 同级，`README_zh.md:120` 的相对链接改动最小（`CONTRIBUTING_zh.md` → `.github/CONTRIBUTING_zh.md`），且贡献指南本属 GitHub 协作域，放在 `.github/` 语义正确。
- 备选 `docs/zh/CONTRIBUTING.md` 语义也对，但会让 `docs/` 提前承担语言分区骨架（需同步建 `docs/en/` 占位），本次“仅治根”不做；未来若 `docs/` 全面语言分区，可再将 `.github/CONTRIBUTING_zh.md` 迁移并在 `.github/` 留 2 行跳转文件，零破环。

### 2.3 文件移动清单

| 操作 | 源 | 目标 | 备注 |
|------|----|------|------|
| move | `CONTRIBUTING.md:1` | `.github/CONTRIBUTING.md` | GitHub 自动发现，无需跳转 |
| move | `CONTRIBUTING_zh.md:1` | `.github/CONTRIBUTING_zh.md` | 中文版同级 |
| keep | `README.md:98` | — | 链更新见 §2.4 |
| keep | `README_zh.md:120` | — | 链更新 |
| keep | `CONTRIBUTING*.md:3` 内互链 | — | 互链更新 |

> `git mv` 保留历史；若用 `mv` 需 `git add -A` 识别 rename。

### 2.4 链接更新

需改 4 处（grep 已验证 `CONTRIBUTING` 7 命中）：

1. `README.md:16` Badge `PRs Welcome`：`CONTRIBUTING.md` → `.github/CONTRIBUTING.md`
2. `README.md:98` Contributing 小节：`[CONTRIBUTING.md](CONTRIBUTING.md)` → `[CONTRIBUTING.md](.github/CONTRIBUTING.md)`
3. `README_zh.md:16` Badge：`CONTRIBUTING_zh.md` → `.github/CONTRIBUTING_zh.md`
4. `README_zh.md:120`：`[CONTRIBUTING_zh.md](CONTRIBUTING_zh.md) / [CONTRIBUTING.md](CONTRIBUTING.md)` → `[CONTRIBUTING_zh.md](.github/CONTRIBUTING_zh.md) / [CONTRIBUTING.md](.github/CONTRIBUTING.md)`
5. `CONTRIBUTING.md:5` 头部 `[中文版](CONTRIBUTING_zh.md)` → `[中文版](CONTRIBUTING_zh.md)` （同目录无需改）或显式 `./CONTRIBUTING_zh.md`；保持相对即可
6. `CONTRIBUTING_zh.md:3` `> English version: [CONTRIBUTING.md](CONTRIBUTING.md)` 同理保持
7. 若未来 `docs/zh/` 方案，则需改 `CONTRIBUTING*.md` 互链为 `../`，本设计不改。

### 2.5 GitHub 发现验证

- 验证点：新建 Issue 时侧边是否提示 “Contributing guidelines” 指向 `.github/CONTRIBUTING.md`。
- 本地校验脚本（可放入 `scripts/check-docs-links.sh` 或一次性 `bash -c`）：
  ```bash
  rg -n "CONTRIBUTING" README* .github/CONTRIBUTING* --no-heading
  test -f .github/CONTRIBUTING.md && test -f .github/CONTRIBUTING_zh.md && echo "ok"
  test ! -f ./CONTRIBUTING.md && test ! -f ./CONTRIBUTING_zh.md && echo "root clean"
  ```

### 2.6 迁移步骤 (执行顺序)

1. `git mv CONTRIBUTING.md .github/CONTRIBUTING.md`
2. `git mv CONTRIBUTING_zh.md .github/CONTRIBUTING_zh.md`
3. 编辑 `README.md:16,98` / `README_zh.md:16,120` 4 处链接
4. `pnpm build` 无关但跑一遍确保 README 链不断；`rg CONTRIBUTING` 复核无残留 `](CONTRIBUTING` 指向旧根
5. `git status` 应显示 `R CONTRIBUTING.md -> .github/CONTRIBUTING.md` 的 rename 检测
6. 本地提交：`docs: tidy root — move CONTRIBUTING* into .github per language partition (A)`

### 2.7 风险与回滚

- **风险1**：旧外链（外部博客直接链 `.../blob/main/CONTRIBUTING.md`）404。缓解：GitHub 会对 moved file 显示 “moved” 提示；可在根留 2 行跳转文件（本设计不留，因外链概率低；若审计要求可加 `CONTRIBUTING.md` 跳转：`> 已迁移至 [.github/CONTRIBUTING.md](.github/CONTRIBUTING.md)`）。
- **风险2**：中文 CONTRIBUTING 在 `.github/` 非 GitHub 默认识别，侧边仅显示英文。接受：中文用户通过 `README_zh` 入口进入，符合“按语言分区”预期。
- **回滚**：`git mv .github/CONTRIBUTING* ./ &&` 还原 README 4 处链接，一次提交即回滚。

## 3. 成功标准

- `ls *.md` 根仅 `README.md` `README_zh.md`；`ls .github/*.md` 含双语 CONTRIBUTING。
- `rg CONTRIBUTING` 无指向根旧路径的断链。
- GitHub 新建 Issue 侧边可点开 Contributing。

## 4. 不做事项

- 不动 `LICENSE`、`index.html`、`docs/` 其他。
- 不建 `docs/en`/`docs/zh` 全量分区（留给后续 `docs` 全面国际化时再做）。
