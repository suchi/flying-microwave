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

1. `gh pr merge <PR番号> --merge --delete-branch`。GitHubのツールで直接マージするときは、この手順を同等のマージの操作(マージのコミットを作る方法)に読み替え、手順2以降を行う。そのときの前提条件3の承認は、ユーザーがマージを明示して承認したことを指す。
2. `git checkout main && git pull`
3. バージョンタグ指定があれば: エージェントはタグを付けない(タグはメンテナーだけが付けられる。ルールセット`release-tags`)。マージのコミットのSHAと件名を`git log -1 --no-decorate --oneline <SHA>`で確かめ、期待する件名(例: `Merge pull request #56 ...`)と一致することを確かめる。一致しないときは、依頼を書かずにメンテナーに知らせる。一致したら、最後の報告に、メンテナーに依頼するコマンドを書く(AGENTS.mdの「進め方」、[docs/incidents/0016](../../docs/incidents/0016-tag-before-merge.md))。コマンドは次の形にする。
   - 件名の確認: `git log -1 --no-decorate --oneline <SHA>`(期待する表示: `<短いSHA> Merge pull request #<番号> ...`。`--no-decorate`は、端末でブランチ名やタグの括弧が付かないようにするため。一致しないときはタグを付けずに知らせてもらう)。引用符の要らない形にする(メンテナーはcmdで実行する。AGENTS.mdの「進め方」、[docs/incidents/0023](../../docs/incidents/0023-tag-check-command-failed-in-cmd.md))
   - タグ付け: `git tag -a <タグ> <SHA> -m "<タグ>"`(対象のコミットを必ず指定する)
   - 送信: `git push origin <タグ>`(`--tags`は使わない)
4. マージしたPRに含まれたincidentの記録を、`git diff --name-only <マージのコミット>^1 <マージのコミット> -- docs/incidents/`で一覧にし(`NNNN-*.md`の記録だけを見る。`README.md`は記録ではない)、完了報告に示す([docs/incidents/0029](../../docs/incidents/0029-incident-status-stale-again.md))。マージしたPRが`docs/incidents/`の記録の対策を含むときは、その記録のステータスを直す変更を、次の文書の変更で行う(マージしたPRの中では直せないため)。記録の対策がすべてマージされたら「対策済み」にする。一部だけなら「対策中」のまま、残りの対策と、それを登録した場所(計画の作業など)を書く。完了報告に、直す予定の記録を書く([docs/incidents/0020](../../docs/incidents/0020-incident-status-not-updated.md))。
5. ローカルのフィーチャーブランチが残っていれば削除する。
6. リモートのブランチは、リポジトリの設定(Automatically delete head branches。マージしたPRの元のブランチを自動で削除する)で消える。`git ls-remote --heads origin <ブランチ>`で残っていないことを確かめ、残っていればメンテナーに削除を依頼する(エージェントの環境からは、ブランチの削除が権限の確認で止められることがある)。
7. 日本語で完了報告する。
