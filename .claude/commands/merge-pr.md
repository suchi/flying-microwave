---
description: 承認済みPRのマージ・ブランチ削除・mainの同期
argument-hint: <PR番号> [バージョンタグ]
---

$ARGUMENTS の第1引数で指定されたPRを、最終チェックのうえマージする。

前提条件 — すべて確認し、1つでも満たさなければマージせず停止して報告する:

1. レビュー完了: 未解決スレッドがなく、`docs/development/review-process.md`の「レビュー完了の判定」を満たす(`git fetch origin`のあと、PRの情報から取得したhead SHAで確かめる。ローカルの`HEAD`は使わない)。
2. CIの全ジョブがグリーン。
3. ユーザー承認(ユーザーによるこのコマンドの実行は承認とみなす)。

手順:

1. `gh pr merge <PR番号> --merge --delete-branch`
2. `git checkout main && git pull`
3. バージョンタグ指定があれば: `git tag -a <バージョン> -m "<バージョン>"`のうえ`git push origin --tags`
4. ローカルのフィーチャーブランチが残っていれば削除する。
5. リモートのブランチは、リポジトリの設定(Automatically delete head branches。マージしたPRの元のブランチを自動で削除する)で消える。`git ls-remote --heads origin <ブランチ>`で残っていないことを確かめ、残っていればメンテナーに削除を依頼する(エージェントの環境からは、ブランチの削除が権限の確認で止められることがある)。
6. 日本語で完了報告する。
