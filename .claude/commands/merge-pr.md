---
description: 承認済みPRのマージ・ブランチ削除・mainの同期
argument-hint: <PR番号> [バージョンタグ]
---

$ARGUMENTS の第1引数で指定されたPRを、最終チェックのうえマージする。

前提条件 — すべて確認し、1つでも満たさなければマージせず停止して報告する:

1. レビュー完了: 未解決スレッドがなく、`docs/development/review-process.md`の「レビュー完了の判定」を満たす(`git fetch origin`のあと、PRの情報から取得したhead SHAで確かめる。ローカルの`HEAD`は使わない)。
2. CIの全ジョブがグリーン。
2の2. PRの本文に`claude.ai/code/session_`のリンクがない(PRの情報を読んで確かめる。docs/incidents/0036)。
2の3. mainの最新のCIの実行が成功している(`ci.yml`のワークフローの実行の一覧を、ブランチ`main`とイベント`push`で絞り、最新のものを確かめる。手動実行は含めない)。失敗していれば、そのPRがその失敗の原因を直すものでない限り、マージしない(docs/incidents/0039)。
3. ユーザー承認(ユーザーによるこのコマンドの実行は承認とみなす)。

手順:

1. `gh pr merge <PR番号> --merge --delete-branch`。GitHubのツールで直接マージするときは、この手順を同等のマージの操作(マージのコミットを作る方法)に読み替え、手順2以降を行う。そのときの前提条件3の承認は、ユーザーがマージを明示して承認したことを指す。
2. `git checkout main && git pull`
3. バージョンタグ指定があれば: エージェントはタグを付けない(タグはメンテナーだけが付けられる。ルールセット`release-tags`)。マージのコミットのSHAと件名を`git log -1 --no-decorate --oneline <SHA>`で確かめ、期待する件名(例: `Merge pull request #56 ...`)と一致することを確かめる。一致しないときは、依頼を書かずにメンテナーに知らせる。一致したら、最後の報告に、メンテナーに依頼するコマンドを書く(AGENTS.mdの「進め方」、[docs/incidents/0016](../../docs/incidents/0016-tag-before-merge.md))。コマンドは次の形にする。
   - 件名の確認: `git log -1 --no-decorate --oneline <SHA>`(期待する表示: `<短いSHA> Merge pull request #<番号> ...`。`--no-decorate`は、端末でブランチ名やタグの括弧が付かないようにするため。一致しないときはタグを付けずに知らせてもらう)。引用符の要らない形にする(メンテナーはcmdで実行する。AGENTS.mdの「進め方」、[docs/incidents/0023](../../docs/incidents/0023-tag-check-command-failed-in-cmd.md))
   - タグ付け: `git tag -a <タグ> <SHA> -m "<タグ>"`(対象のコミットを必ず指定する)
   - 送信: `git push origin <タグ>`(`--tags`は使わない)
4. `python3 -I scripts/list_merge_incidents.py <マージのコミット>`で、マージしたPRが変えたincidentの記録(`changed`)と、PRで加えた行が参照するincident(`referenced`。同じファイルの消した行にも参照があったものは`also in a removed line`と示す。書き換えた行に対策が入ることもあるので、ほかと同じく記録ごとに判断する)を、ステータスの行と合わせて一覧にする(浅い履歴では`<マージのコミット>^1`がないので、先に`git fetch --unshallow`で履歴を取る)。一覧の記録ごとに、ステータスを直したか、直さない理由を、完了報告に書く([docs/incidents/0029](../../docs/incidents/0029-incident-status-stale-again.md)とその「その後の再発」。記録のファイルを変えずに対策だけを含むPRでは、差分の記録の一覧に出ないため)。マージしたPRが`docs/incidents/`の記録の対策を含むときは、その記録のステータスを直す変更を、次の文書の変更で行う(マージしたPRの中では直せないため)。記録の対策がすべてマージされたら「対策済み」にする。一部だけなら「対策中」のまま、残りの対策と、それを登録した場所(計画の作業など)を書く。完了報告に、直す予定の記録を書く([docs/incidents/0020](../../docs/incidents/0020-incident-status-not-updated.md))。
4の2. マージのコミットのmainのCIの実行を、`ci.yml`のワークフローの実行の一覧(ブランチ`main`、イベント`push`)から`head_sha`で見つけ、終わるのを待って結果を確かめ、完了報告に書く。待つ間は、ポーリングせずに、確認の予約(数分後のリマインダーなど)を使う。失敗していたら異常として扱い、原因を調べてメンテナーに知らせる(docs/incidents/0039)。
5. ローカルのフィーチャーブランチが残っていれば削除する。
6. リモートのブランチは、リポジトリの設定(Automatically delete head branches。マージしたPRの元のブランチを自動で削除する)で消える。`git ls-remote --heads origin <ブランチ>`で残っていないことを確かめ、残っていればメンテナーに削除を依頼する(エージェントの環境からは、ブランチの削除が権限の確認で止められることがある)。
7. 日本語で完了報告する。
