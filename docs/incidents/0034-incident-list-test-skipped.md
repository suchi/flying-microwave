# 0034: incidentを記録したコミットで、手順が求める一覧のテストを実行しなかった

- 日付: 2026-11-19
- ステータス: 対策中(2026-11-20に、メンテナーが対策の(1)〜(3)を推奨どおりと判断した。計画0005のD-23)
- 関連: [0026](0026-incident-row-placed-in-procedure.md)(一覧のテストを手順に加えたincident)、[0033](0033-exit-time-near-limit-unnoticed.md)(この異常が起きた記録の作業)

## 起きたこと

2026-11-19に、incident 0033を記録したコミット(`3a9db20`)で、エージェントは、スクリプトのテスト(`python3 -I -m unittest discover -s scripts`)を実行しないままコミットした。`docs/README.md`の一覧に0033の行を加え忘れていて、一覧のテスト(`scripts/test_incident_lists.py`の`test_the_docs_readme_lists_every_record`)が失敗する状態だった。プッシュの前の軽いレビューで見つかった。

確かめた事実:

- `docs/incidents/README.md`の手順5は、「記録したあとで、`python3 -I -m unittest discover -s scripts`を実行し、一覧のテスト(`scripts/test_incident_lists.py`)が通ることを確かめる(文書だけの変更でも行う。[0026])」と定めている。
- AGENTS.mdの「検証コマンド」の節は、「コミット前に必ずテスト・リントを実行する。ドキュメントだけの変更(Rustのコード・Cargoの設定・ワークフローを変えないもの)では不要とする」と書いている。incidentの記録を例外とすることは、AGENTS.mdには書かれていない(`grep -n 'test_incident_lists' AGENTS.md`で0件)。
- エージェントは、記録を書く前に`docs/incidents/README.md`を読んだが、`sed -n 1,60p ... | cut -c1-300`で、各行を300文字で切って読んだ(このセッションのコマンド)。その行(手順5)は438文字で、テストの文の始まり(「記録したあとで」)は269文字目にある。GNU coreutilsの`cut -c`はバイト単位で切るため(この環境で`printf 'あいう\n' | cut -c1-3`は「あ」だけを出す)、読めたのは先頭の300バイト、和文で約100文字だけだった。テストの文は640バイト目付近から始まり、読んだ範囲に入っていなかった。
- コミットのときは、AGENTS.mdの「文書だけの変更では不要」に従い、テストを実行しなかった。
- CIは、スクリプトのテストを実行する(`.github/workflows/ci.yml`のWindowsのジョブの`Script tests`の手順)。プッシュしていれば、CIで失敗していた。

## なぜなぜ分析

1. なぜテストを実行しなかったか。
   - 文書だけの変更と考え、AGENTS.mdの「不要とする」に従ったため。
2. なぜ手順5の例外に従わなかったか。
   - 手順を、行を300文字で切るつもりで`cut -c1-300`で読んだが、`cut -c`はバイト単位で切るため、実際には約100文字しか読んでおらず、例外の文を読んでいなかった。
3. なぜ、切って読んでも例外に気づけなかったか。
   - 例外が`docs/incidents/README.md`の手順5の長い行の終わりにだけ書かれていて、エージェントがコミットのときに従うAGENTS.mdの検証の節には書かれていないため。2か所の書き方が違い、AGENTS.mdだけを見ると「不要」と読める。
4. なぜ仕組みで止まらなかったか。
   - コミットの前にテストを強制する仕組み(gitのフックやClaude Codeのフック)がない。CIは、プッシュしたあとでしか動かない。

## 真因

- 文書だけの変更でテストを行う例外が、手順の文書の中にだけあり、コミットのときに従う規則(AGENTS.md)には書かれていない。
- incidentの記録のコミットで、テストを実行させる仕組みがない。

## 対策

2026-11-20に、メンテナーが(1)〜(3)を推奨どおりと判断した(計画0005のD-23)。(1)と(3)は、2026-11-20の記録の変更で、AGENTS.md(検証の節と表、「情報源」の節)と`.claude/commands/commit.md`に加えた。`docs/development/onboarding.md`と`.claude/commands/babysit-pr.md`の同じ説明への書き添えと、(2)のフックは、計画0005の作業10の5で行う。

行ったこと(計画0005の作業10の5):

- (2): `block-dangerous-git.sh`は、`git commit`のときに、`docs/incidents/`か`docs/README.md`に変更があれば、一覧のテストを実行し、失敗したらコミットを止める。Pythonが動かない環境では止めない。`scripts/check_hooks.py`に、一覧にない記録があるときに止まり、ないときは通ることを試す入力を加えた。変更があるかどうかは、承認された案(ステージした変更)と違い、作業ツリー全体で見ている。この違いを確かめずに実装したため、[0040](0040-hook-implemented-against-reported-design.md)に記録し、どちらにするかをメンテナーに確かめている。
- `docs/development/onboarding.md`と`.claude/commands/babysit-pr.md`への書き添えは、作業10の5で行った。

案:

1. 規則: AGENTS.mdの「検証コマンド」の節の「ドキュメントだけの変更…では不要とする」に、「ただし、`docs/incidents/`と`docs/README.md`を変えるときは、スクリプトのテストを実行する(`docs/incidents/README.md`の手順5)」と書き添える。
2. 仕組み: `block-dangerous-git.sh`で、`git commit`のときに、ステージした変更に`docs/incidents/`か`docs/README.md`が含まれていれば、一覧のテスト(`python3 -I -m unittest scripts/test_incident_lists.py`の形)を実行し、失敗したらコミットを止める。フックの変更なので、承認が要る。フックが重くなりすぎないかは、試して確かめる。
3. 読み方: 規則と手順の文書は、行を切らずに読む(Readのツールで読む)。`cut -c`は和文をバイト単位で切り、読んだつもりの範囲より狭くなるため。エージェントの作業の仕方で、規則にするかどうかは判断していただくこと。

## 確かめ方

- 対策1: AGENTS.mdの検証の節に、例外の文があること。
- 対策2: `scripts/check_hooks.py`の試験の入力で、一覧のテストが失敗する状態の`git commit`が止まること。
- このあとのincidentの記録のコミットで、一覧のテストが失敗したまま入っていないこと(CIの`Script tests`の結果)。
