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
# Why tr: a native jq.exe started from Git Bash on Windows turns every LF
# of its output into CRLF (jq manual, --binary), so every CR is removed: a
# CR must not decide whether a pattern matches. bash does not split words
# at a CR, so removing them only lets more patterns match. The one
# exception is the .env.example exclusion below, which can be passed
# without a CR as well (docs/incidents/0019).
cmd=$(printf '%s' "$cmd" | tr -d '\r')

[ -n "$cmd" ] || exit 0

has() {
  printf '%s' "$cmd" | grep -Eq "$1"
}

# "git <subcommand>", also with git options before it (git -C dir push,
# git -c key=value commit; docs/reviews/0005, K-1; plan 0005, D-24). Only
# words that start with "-", each with an optional argument, may come
# between git and the subcommand, so that "git commit -m '... push ...
# main'" is not taken for a push.
git_opts='git([[:space:]]+-[^[:space:]|;&]+([[:space:]]+[^-[:space:]|;&][^[:space:]|;&]*)?)*[[:space:]]+'
git_push="${git_opts}push"
git_commit="${git_opts}commit"

is_push=false
if has "$git_push"'([[:space:]]|$)'; then is_push=true; fi

force_push=false
if [ "$is_push" = true ]; then
  if has "$git_push"'[^|;&]*--force' || has "$git_push"'[^|;&]*[[:space:]]-f([[:space:]]|$)'; then
    force_push=true
  fi
fi

touches_main=false
if has '[[:space:]:+](main|master)([^[:alnum:]_-]|$)'; then touches_main=true; fi

# --- Never allowed ------------------------------------------------------

if [ "$force_push" = true ] && [ "$touches_main" = true ]; then
  emit deny "main・masterへのforce pushは禁止です(AGENTS.mdの開発フロー)。"
fi

# The destination main in these refspec forms: main, +main, HEAD:main,
# <branch>:main, refs/heads/main (docs/reviews/0005, K-1). A branch whose
# name only contains main (fix/main, main-fix) is not matched. Quoted names,
# line continuations, option values with spaces and git.exe are not matched
# (docs/development/onboarding.md, section 6.3); the ruleset of main is the
# real defense.
if has "$git_push"'[^|;&]*[[:space:]]\+?([^[:space:]|;&:]*:)?(refs/heads/)?(main|master)([^[:alnum:]_/.-]|$)'; then
  emit deny "main・masterへの直接のプッシュは禁止です。フィーチャーブランチとPRを使ってください(AGENTS.md)。"
fi

# Also with git options before the subcommand, and the short form -n of
# git commit (a cluster such as -an too; plan 0005, D-24). git push -n is
# --dry-run, so only commit is checked for -n.
if has "${git_opts}"'(commit|push)[^|;&]*--no-verify' ||
  has "$git_commit"'[^|;&]*[[:space:]]-[[:alnum:]]*n[[:alnum:]]*([[:space:]]|$)'; then
  emit deny "--no-verify(git commitの-nを含む)はフックと検査を飛ばすため、禁止です。"
fi

# docs/incidents/0034: a commit that changes the incident lists runs their
# test first. The working tree is looked at, not only the index, because
# "git add ... && git commit" stages the files after this hook has run. A
# missing Python stops nothing.
if has "$git_commit"; then
  root=${CLAUDE_PROJECT_DIR:-.}
  if git -C "$root" status --porcelain -- docs/incidents docs/README.md 2>/dev/null | grep -q .; then
    # The first Python that runs. Why -c '': on Windows, an app execution
    # alias named python3 can be found without Python installed, and it
    # fails when run, while python may work (not checked on Windows).
    py=
    for c in python3 python; do
      if command -v "$c" >/dev/null 2>&1 && "$c" -c '' >/dev/null 2>&1; then
        py=$c
        break
      fi
    done
    if [ -n "$py" ] && [ -f "$root/scripts/test_incident_lists.py" ] &&
      ! (cd "$root" && "$py" -I -m unittest discover -s scripts -p test_incident_lists.py >/dev/null 2>&1); then
      emit deny "incidentの一覧のテスト(scripts/test_incident_lists.py)が通りません。docs/incidents/README.mdとdocs/README.mdの一覧を直してから、コミットしてください(docs/incidents/0034)。"
    fi
  fi
fi

# Plan 0005, D-42 (docs/incidents/0032): a pipe before git commit makes the
# exit status that of the last command of the pipe, so a failing check
# ("check | tail -1 && git commit") does not stop the commit. Any single "|"
# before the first commit stops it unless pipefail is mentioned; "||" is not
# a pipe. A "|" inside quotes or a here-document stops it as well, and so
# does a command that only searches for the words "git commit" after a pipe;
# "set -o pipefail" lets both through (the maintainer accepted both; plan
# 0005, D-42).
if has "$git_commit"; then
  before=$(printf '%s' "$cmd" | tr '\n' ' ' | sed -E "s/${git_commit}.*//")
  if printf '%s' "$before" | grep -Eq '(^|[^|])\|([^|]|$)' &&
    ! printf '%s' "$before" | grep -q 'pipefail'; then
    emit deny "git commitより前にパイプ(|)があります。検証の出力をパイプで渡すと、終了の状態はパイプの最後のコマンドのものになり、検証が失敗してもコミットが止まりません。検証とコミットを別の呼び出しにするか、出力をファイルに書くか、先頭にset -o pipefailを付けてください。コミットをしないコマンド(git commitという語を検索するだけのものなど)や、引用符やヒアドキュメントの中の|で止まったときも、先頭にset -o pipefailを付ければ通ります(docs/incidents/0032、計画0005のD-42)。"
  fi
fi

# Plan 0005, D-43 (docs/incidents/0034): rule documents are read whole.
# cut -c cuts Japanese text by bytes, so far less than intended was read. A
# pipe into cut -c in a command that names a rule document stops it. cut -c
# given the file directly (no pipe) is not checked.
if has '\|[[:space:]]*cut[[:space:]]+-c' &&
  has '(AGENTS\.md|CLAUDE\.md|\.agents/skills/|\.claude/commands/|\.claude/skills/|docs/incidents/README\.md|docs/adr/README\.md|docs/development/)'; then
  emit deny "規則の文書(AGENTS.md、.claude/commands/、docs/development/など)の行をcut -cで切っています。cut -cは和文をバイト単位で切るため、読んだつもりの範囲よりずっと狭くなります。行の場所や有無はgrep -c、grep -l、grep -o(一致した部分だけ)で探し、中身はReadのツールで切らずに読んでください(docs/incidents/0034、計画0005のD-43)。"
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
