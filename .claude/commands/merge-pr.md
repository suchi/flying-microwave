---
description: 承認済みPRのマージ・ブランチ削除・mainの同期
argument-hint: <PR番号> [バージョンタグ]
---

$ARGUMENTS の第1引数で指定されたPRを、最終チェックのうえマージする。

前提条件 — すべて確認し、1つでも満たさなければマージせず停止して報告する:

1. レビュー完了: 未解決スレッドがなく、最新の敵対的レビューのまとめコメントに「新規指摘: 0件」とレビューしたhead SHAが記録され、そのSHAが現在のPRのhead(`git fetch origin`のあと、PRの情報から取得する。ローカルの`HEAD`は使わない)と一致する。一致しない場合は、`docs/development/review-process.md`の「最終回の記録」の条件と確認方法(`git diff --no-renames --name-only <レビューしたSHA> <PRのhead SHA>`)で、その後のコミットが最終回の記録だけであることを確かめる。最新のまとめコメントが「すべて軽微。修正済み」の場合は、同じ文書の「軽微な指摘だけが残った場合」の判定に従う。
2. CIの全ジョブがグリーン。
3. ユーザー承認(ユーザーによるこのコマンドの実行は承認とみなす)。

手順:

1. `gh pr merge <PR番号> --merge --delete-branch`
2. `git checkout main && git pull`
3. バージョンタグ指定があれば: `git tag -a <バージョン> -m "<バージョン>"`のうえ`git push origin --tags`
4. ローカルのフィーチャーブランチが残っていれば削除し、日本語で完了報告する。
