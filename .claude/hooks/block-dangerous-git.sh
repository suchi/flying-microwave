#!/bin/sh
# PreToolUse hook for the Bash tool (registered in .claude/settings.json).
#
# Denies git operations that must never run under this repository's workflow
# (AGENTS.md) and asks for user confirmation on destructive ones, so the
# guard also works in auto-accept permission modes.
#
# Requires jq to parse the hook payload (docs/development/onboarding.md,
# section 6.3). If jq is missing or the payload does not parse, the hook asks
# instead of deciding, so the guard fails safe rather than silently
# allowing everything.

input=$(cat)

emit() {
  # $1: permission decision (deny|ask), $2: reason (escaped by jq)
  jq -cn --arg decision "$1" --arg reason "$2" \
    '{hookSpecificOutput: {hookEventName: "PreToolUse", permissionDecision: $decision, permissionDecisionReason: $reason}}'
  exit 0
}

if ! command -v jq >/dev/null 2>&1; then
  printf '{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"ask","permissionDecisionReason":"このフックにはjqが必要ですが、見つかりません。jqを導入してください(docs/development/onboarding.mdの6.3)。"}}\n'
  exit 0
fi

cmd=$(printf '%s' "$input" | jq -r '.tool_input.command // empty' 2>/dev/null) ||
  emit ask "フックの入力をJSONとして解析できませんでした。先にユーザーに確認してください。"
# Why tr: a native jq.exe started from Git Bash on Windows turns every LF of
# its output into CRLF (jq manual, --binary). Every CR is removed: a CR must not
# decide whether a pattern matches.
# bash does not split words at a CR, so removing them hides nothing that
# would run (docs/incidents/0019).
cmd=$(printf '%s' "$cmd" | tr -d '\r')

[ -n "$cmd" ] || exit 0

has() {
  printf '%s' "$cmd" | grep -Eq "$1"
}

is_push=false
if has 'git[[:space:]]+push'; then is_push=true; fi

force_push=false
if [ "$is_push" = true ]; then
  if has 'git[[:space:]]+push[^|;&]*--force' || has 'git[[:space:]]+push[^|;&]*[[:space:]]-f([[:space:]]|$)'; then
    force_push=true
  fi
fi

touches_main=false
if has '[[:space:]:](main|master)([^[:alnum:]_-]|$)'; then touches_main=true; fi

# --- Never allowed ------------------------------------------------------

if [ "$force_push" = true ] && [ "$touches_main" = true ]; then
  emit deny "main・masterへのforce pushは禁止です(AGENTS.mdの開発フロー)。"
fi

if has 'git[[:space:]]+push[^|;&]*[[:space:]](HEAD:)?(main|master)([^[:alnum:]_/-]|$)'; then
  emit deny "main・masterへの直接のプッシュは禁止です。フィーチャーブランチとPRを使ってください(AGENTS.md)。"
fi

if has 'git[[:space:]]+(commit|push)[^|;&]*--no-verify'; then
  emit deny "--no-verifyはフックと検査を飛ばすため、禁止です。"
fi

if has '(^|[;&|[:space:]"])rm[[:space:]]+-[[:alnum:]]*r[[:alnum:]]*[[:space:]]+(/|~/?)([[:space:]"]|$)'; then
  emit deny "ファイルシステムのルートやホームディレクトリの削除は拒否します。"
fi

# --- Ask the user first ---------------------------------------------------

if [ "$force_push" = true ]; then
  emit ask "force pushです。先にユーザーに確認してください。フィーチャーブランチでは--force-with-leaseを使ってください(AGENTS.md)。"
fi

if has 'git[[:space:]]+reset[^|;&]*--hard'; then
  emit ask "git reset --hardは手元の変更を捨てます。先にユーザーに確認してください(AGENTS.md)。"
fi

if has 'git[[:space:]]+clean[^|;&]*[[:space:]]-[[:alnum:]]*f'; then
  emit ask "git clean -fは追跡していないファイルを削除します。先にユーザーに確認してください(AGENTS.md)。"
fi

if has 'git[[:space:]]+branch[^|;&]*[[:space:]]-D([[:space:]]|$)'; then
  emit ask "ブランチを強制的に削除します。先にユーザーに確認してください(AGENTS.md)。"
fi

if has 'git[[:space:]]+commit'; then
  current_branch=$(git branch --show-current 2>/dev/null || true)
  if [ "$current_branch" = "main" ] || [ "$current_branch" = "master" ]; then
    emit ask "$current_branchに直接コミットしようとしています。開発フローではフィーチャーブランチが必要です(AGENTS.md)。先にユーザーに確認してください。"
  fi
fi

if has '(curl|wget)[^|;&]*\|[[:space:]]*(sudo[[:space:]]+)?[a-z]*sh([[:space:]]|$)'; then
  emit ask "ダウンロードしたスクリプトをシェルに直接流そうとしています。内容を確かめ、先にユーザーに確認してください(AGENTS.mdのセキュリティ)。"
fi

if has 'git[[:space:]]+add[^|;&]*\.env' && ! has '\.env\.(example|sample|template)'; then
  emit ask ".envファイルをステージしようとしています。シークレットはコミットしない決まりです(AGENTS.mdのセキュリティ)。先にユーザーに確認してください。"
fi

exit 0
