#!/usr/bin/env bash
# sync-wiki.sh — 把 agentlauncher/docs/wiki 同步到 GitHub Wiki
# 用法: ./scripts/sync-wiki.sh [--dry-run]
# 依赖: gh (已登录), git
# 说明: 仅同步 agentlauncher/docs/wiki/，外层 docs/ 是离线设计稿，刻意不并入 Wiki。
set -euo pipefail

REPO="lildengzi/agentLauncher"
WIKI_SRC="docs/wiki"
DRY_RUN=false
if [[ "${1:-}" == "--dry-run" ]]; then DRY_RUN=true; fi

if ! command -v gh >/dev/null 2>&1; then echo "需要 gh-cli"; exit 1; fi
if ! gh auth status >/dev/null 2>&1; then echo "gh 未登录: gh auth login"; exit 1; fi

# 确认 Wiki 已启用
HAS_WIKI=$(gh api "repos/${REPO}" --jq .has_wiki 2>/dev/null || echo "false")
if [[ "$HAS_WIKI" != "true" ]]; then
  echo "Wiki 未启用，正在启用..."
  gh api --method PATCH "repos/${REPO}" --field has_wiki=true --jq .has_wiki >/dev/null
fi

# 检查是否在正确目录（脚本位于 agentlauncher/scripts/）
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
WIKI_SRC_ABS="${REPO_ROOT}/${WIKI_SRC}"

if [[ ! -d "$WIKI_SRC_ABS" ]]; then
  echo "找不到 Wiki 源目录: $WIKI_SRC_ABS"; exit 1
fi

echo "Wiki 源: $WIKI_SRC_ABS"
ls -1 "$WIKI_SRC_ABS" | sed 's/^/  - /'

if [[ "$DRY_RUN" == true ]]; then
  echo "[dry-run] 不推送"
  exit 0
fi

TOKEN="$(gh auth token)"
WIKI_URL="https://x-access-token:${TOKEN}@github.com/${REPO}.wiki.git"
TMPDIR="$(mktemp -d)"
trap 'rm -rf "$TMPDIR"' EXIT

echo "克隆 Wiki 到 $TMPDIR ..."
# Wiki 首次创建前 git clone 会 404，这里做兼容：失败则本机 init
if ! git clone "$WIKI_URL" "$TMPDIR" 2>&1; then
  echo "Wiki 远端尚未初始化（首次推送前会 404），本机初始化..."
  git init "$TMPDIR" >/dev/null
  git -C "$TMPDIR" remote add origin "$WIKI_URL"
  git -C "$TMPDIR" config user.email "lildengzi@users.noreply.github.com"
  git -C "$TMPDIR" config user.name "lildengzi"
else
  git -C "$TMPDIR" config user.email "lildengzi@users.noreply.github.com"
  git -C "$TMPDIR" config user.name "lildengzi"
fi

# 同步文件（保留 .git，清空其余后复制）
find "$TMPDIR" -mindepth 1 -maxdepth 1 ! -name '.git' -exec rm -rf {} +
cp -v "$WIKI_SRC_ABS"/*.md "$TMPDIR"/

echo "待提交内容:"
ls -1 "$TMPDIR"/*.md | xargs -I{} basename {}

git -C "$TMPDIR" add -A
if git -C "$TMPDIR" diff --cached --quiet; then
  echo "Wiki 无变化，无需推送"
  exit 0
fi

git -C "$TMPDIR" commit -m "docs(wiki): sync from ${WIKI_SRC} @ $(date -u +%Y-%m-%dT%H:%M:%SZ)"

# Wiki 默认分支是 master（旧仓）或 main，新版 GitHub 已切 main；都推一次
echo "推送到 Wiki..."
git -C "$TMPDIR" push -u origin HEAD:master 2>&1 || true
# 若 master 已推，再同步 main 以兼容
git -C "$TMPDIR" branch -M master 2>/dev/null || true
git -C "$TMPDIR" push origin master:main 2>&1 || true

echo "完成: https://github.com/${REPO}/wiki"
echo "本地验证: gh api repos/${REPO} --jq .has_wiki"
gh api "repos/${REPO}" --jq '{has_wiki, html_url}' 2>&1 | cat
