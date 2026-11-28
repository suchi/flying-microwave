---
description: 検証と整合性チェックのうえConventional Commitsでコミットする
---

AGENTS.mdに従って現在の変更をコミットする。

1. 変更を確認する: `git status`と`git diff`(ステージ済みがあれば`git diff --staged`も)。
2. 簡易整合性チェック(自明な変更では省略可): ドキュメントと実態の一致、設計判断を伴う変更なら`docs/adr/`にADRがあるか。問題があれば修正してから進む。
3. AGENTS.mdの検証コマンド(テスト・リント・型チェック)を実行する。ドキュメントだけの変更(Rustのコード・Cargoの設定・ワークフローを変えないもの)では省略してよい(AGENTS.mdの検証コマンド節)。ただし、`docs/incidents/`、`docs/README.md`、`CHANGELOG.md`、`docs/development/hardware-check-*.md`、AGENTS.mdの「規則の写し」の表の文書のどれかを変えたときは、文書だけの変更でも`python3 -I -m unittest discover -s scripts`を実行する(docs/incidents/0034、0038、0042)。シェルスクリプトを変更した場合は、`sh -n`による構文チェックとサンプル入力での動作確認を行う。フック(`.claude/hooks/`と`.claude/settings.json`)を変えた場合は、`python3 -I scripts/check_hooks.py`で、登録したとおりの起動で確かめる(docs/incidents/0009。詳細は`docs/development/onboarding.md`の6.3の「フックとシェルスクリプトを変えるとき」)。実行した検証(文書だけの変更で行う検証を含む)は、すべて通ったことを確かめてからコミットする。検証とコミットを1回の呼び出しでつなぐときは、検証の失敗でコミットが止まる形にする(`&&`でつなぎ、検証の終了の状態をパイプなどで失わない。docs/incidents/0032)。検証の出力を短くして見るときは、出力をファイルに書いてから読むか、先頭に`set -o pipefail`を付ける。`git commit`より前にパイプがあり、`pipefail`の語がないコマンドは、フック(`block-dangerous-git.sh`)が止める(止める形は網羅ではない。`docs/development/onboarding.md`の6.3。0032の「その後の再発」、計画0005のD-42)。
4. 関連ファイルを明示的にステージし(無関係な変更があるときは一括`git add -A`をしない)、`git diff --staged`を確認する。追加した行に日付(`2026-11-21`の形)があれば、`TZ=Asia/Tokyo date`のJSTの日付と、出来事の時刻(gitのコミットの日時、GitHubの表示、会話の記録をJSTに直したもの)と比べる。`/commit`のコマンドを使わずに`git commit`を直接実行するときも、この段を行う(AGENTS.mdの「検証コマンド」)。セッションに示される日付はUTCに沿い、JSTの0時から9時の間は前の日になるため(docs/incidents/0041)。過去の出来事の日付は、覚えから書かず、その出来事の時刻(GitHubの表示、gitのコミットの日時)をJSTに直して書く。すでにある文の日付を写したり書き直したりするときも、その出来事の時刻で確かめる。計画やincidentなど、すでにある文書は出典にしない。過去の出来事の日付を書いたコミットでは、確かめたコマンド(`git log`など)をコミットメッセージに書く。GitHubの画面で確かめたときは、画面の項目名(リリースの`published_at`など)でよい(0041の「その後の再発(2回目)」、計画0005のD-41)。「YYYY-MM-DDにPR #NNでマージ」などの形の日付は、`docs/incidents/`、`docs/README.md`、`CHANGELOG.md`、`docs/development/hardware-check-*.md`にあれば、スクリプトのテスト(`scripts/test_merge_dates.py`)が履歴と比べる。計画やADRの日付は比べない。履歴が浅いときは飛ばすので、`git fetch --unshallow`で履歴を取ってから実行する(0041の「その後の再発」、計画0005のD-36とD-37)。文書(`*.md`)に加えた行は、AGENTS.mdの「ドキュメント」に照らして読み直し、会話のやり取り(誰が何と言ったか、最初に何を尋ねたかなど)や作成者個人の事情を書いていれば、決定・要件・ルールとして非人称の形に直す(docs/incidents/0047)。
5. AGENTS.mdの「コミットメッセージ」に従ってメッセージを書く(1行目: 英語・命令形・50文字以内、本文: 日本語)。
6. メッセージをユーザーに提示し、承認後にコミットする。
