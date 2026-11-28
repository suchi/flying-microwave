# 0007: 外部から取り込むものすべてに、公開から7日の待ち(cooldown)をかける

- ステータス: 合意済み(2026-11-27。メンテナーが案のとおりと判断した)
- 日付: 2026-11-27
- 関連: AGENTS.mdの「セキュリティ」(公開から7日以上たった版だけを使う)、[incident 0007](../incidents/0007-dependencies-inside-cooldown.md)、[ADR-0001](../adr/0001-use-rust.md)、`.github/dependabot.yml`、`scripts/check_publish_age.py`

## 目的

リポジトリが外から取り込むもの(依存、道具、アクション、ツールチェーン)のすべてについて、公開から7日以上たった版だけを使う(サプライチェーン攻撃対策)。2026-11-27に、メンテナーが、追加したPythonのスクリプトを含めて点検し、基本的にすべてに7日の待ちを置くよう指示した。

## いまの状況(2026-11-27に確かめた)

| 取り込むもの | いまの対策 | 確かめた方法 |
| --- | --- | --- |
| Rustの依存(`Cargo.lock`) | Dependabotの`cooldown`(7日)と、CIとリリースの`check_publish_age.py` | `.github/dependabot.yml`の`cargo`の項目、`.github/workflows/ci.yml`の「Check dependency cooldown」 |
| CIで入れる`cargo-about`とその依存 | `check_publish_age.py`(`--crate`と、そのcrateの`Cargo.lock`) | 同じ段 |
| GitHub Actionsのアクション | 完全なコミットのSHAで固定。Dependabotの`cooldown`(7日)。手で加えるときは、人が版の公開日を確かめるだけ | `.github/dependabot.yml`の`github-actions`の項目、ワークフローの`uses:` |
| Pythonのスクリプト(`scripts/`の20本) | 標準ライブラリだけを使い、外部のパッケージを入れていない。ただし、それを保つ決まりと検査はない | すべての`.py`のimportを抜き出して確かめた。`requirements.txt`や`pyproject.toml`はない。ワークフローにpipなどはない |
| Rustのツールチェーン(`rust-toolchain.toml`の1.99.1) | 版を固定している。公開からの日数の検査はない | `rust-toolchain.toml`、ADR-0001 |
| GitHubのランナーに入っているもの(Python、jq、curl、rustup、ghなど) | 対策できない(GitHubが更新する) | ワークフローは`setup-python`などを使わず、ランナーの`python3`を使う |
| 手元で使う道具のうち、リポジトリが版を示すもの(resvg、actionlint) | 版を変えるときの手作業の確認だけ(resvgは`check_publish_age.py --crate`) | `scripts/icon/make_ico.py`の`RESVG_VERSION`、onboardingのactionlintの版(レビューの指摘L-3で加えた) |

## 判断していただいたこと

2026-11-27に、メンテナーが、次の3つを対策すると決めた。

- D-1: Pythonのスクリプトは標準ライブラリだけに限る。
- D-2: Rustのツールチェーンの版も、公開から7日以上たっていることをCIで確かめる。
- D-3: 対策できずに残るリスクを、ADRに記録する。

GitHub Actionsのワークフローのロックファイル(`gh actions-lock`の拡張)を採るかは、下の作業4で調べてから決める(E-1)。

同じ日に、実装を見て、メンテナーが次を決めた(どれも推奨の案)。

- D-4: AGENTS.mdの「セキュリティ」に、D-1の規則の1行を加える。AGENTS.mdの検証コマンドの表の「依存の公開からの日数」を、ツールチェーンも確かめる形(`--rust-toolchain rust-toolchain.toml`)にする。CIとリリースのワークフローの「Check dependency cooldown」の段に、同じ引数を加える。
- D-6: `scripts/check_stdlib_only.py`の範囲。Pythonのファイルは許可の一覧で確かめ、ワークフローとフックは、Pythonの起動の形だけを許可の一覧で確かめる。シェルのそのほかの中身は、それらのファイルの変更に要るメンテナーの承認と本格的なレビューに頼る(PRの前のレビューの指摘M-1を受けて決めた。[ADR-0030](../adr/0030-cooldown-on-everything-brought-in.md)の「決定」)。
- D-7: `scripts/check_stdlib_only.py`の目的は、外部のパッケージにうっかり頼ることを防ぐこととする。わざと書かれた抜け道(Pythonの名前の解決、許可したプログラムの引数、`unittest`がモジュールを名前で読むことなど)は、`scripts/`とワークフローの変更に要るPRとレビューに頼る(再レビューの指摘M-AとM-Bを受けて、2026-11-27にメンテナーが推奨の案で決めた。守らないものは[ADR-0030](../adr/0030-cooldown-on-everything-brought-in.md)と検査の説明に書いた)。
- D-5(E-1を決めた): ワークフローのロックファイルは、いまは採らず、調べた結果と見直す条件を[ADR-0030](../adr/0030-cooldown-on-everything-brought-in.md)に記録する。

## 実装の方式(エージェントの案)

- D-1: `scripts/`に、リポジトリのすべての`.py`のimportが標準ライブラリ(`sys.stdlib_module_names`)か`scripts/`の中のモジュールだけであることを確かめるテストを加える(TDD)。AGENTS.mdの「セキュリティ」に、Pythonの外部のパッケージを加えるときは、メンテナーの承認、ハッシュ付きの版の固定、Dependabotの`cooldown`、公開からの日数の検査をそろえる、と書く(規則のファイルなので承認が要る)。
- D-2: `check_publish_age.py`に、Rustのツールチェーンの版の公開日を確かめる機能を加える。公開日は、Rustの公式の配布の情報(`static.rust-lang.org`の`channel-rust-<版>.toml`の`date`)から取る。CIとリリースの「Check dependency cooldown」の段で、`rust-toolchain.toml`の版を渡す(ワークフローの変更なので承認が要る)。
- D-3: 新しいADRに、ランナーの中の道具、メンテナーとエージェントの手元の道具(Python、jq、Claude Code、ブラウザー)が、こちらで待ちをかけられないことと、その理由、どこまでを対策したかを書く。

## 作業

1. この計画の合意を得る。
2. D-1のテストと規則(TDD。規則は承認を得てから)。
3. D-2の検査とワークフローの変更(TDD。ワークフローは承認を得てから)。
4. ワークフローのロックファイルを調べる(E-1)。公式の文書とソースで、次を確かめて報告する。何を固定するか、Freeの個人のリポジトリで使えるか、Dependabotの`cooldown`との関係(v0.2.0のリリースノートは、cooldownにDependabotの設定を使うと書き、v0.3.0は、cooldownがあるとき新しいタグに警告を出すと書く)、`--fail-on-warning`(v0.5.0)で、手で加えた新しいアクションを止められるか、CIで検証するアクション(`github/gh-actions-lock`)と拡張の版の公開日。
5. D-3のADR。作業4の結果を含める。
6. 本格的なレビューを受け(ワークフローと規則のファイルを含むため)、PRを作る。

## あとで決めること

- E-1: GitHub Actionsのワークフローのロックファイルを採るか。2026-11-27にメンテナーが、いまは採らないと決めた(D-5)。決めた内容は次のとおりで、記録の場所は[ADR-0030](../adr/0030-cooldown-on-everything-brought-in.md)の「決定」。
  - 採るか: いまは採らない。見直す条件は、ADR-0030の「将来の見直し条件」。移行は、メンテナーの指示で、Issue #112に登録した(2026-11-27)。
  - どのワークフローに載せるか、CIで検証するか、リポジトリの設定(ロックファイルを必須にする方針)を使うか: 採らないため決めない。見直すときに改めて決める。
  - 調べた事実(公式の文書、一般提供のお知らせ、v0.5.1のREADMEとソース)は、ADR-0030に書いた。

## 作業の結果

- 作業2(D-1): `scripts/check_stdlib_only.py`とそのテストを加え、AGENTS.mdに規則を加えた(2026-11-27)。
- 作業3(D-2): `check_publish_age.py`に`--rust-toolchain`を加え、CIとリリースのワークフローと、AGENTS.mdの検証コマンドの表とonboardingを直した(2026-11-27)。`rust-toolchain.toml`の1.99.1は、配布の情報の日付が2026-10-15で、検査を通る。
- 作業4: 公式の文書、一般提供のお知らせ、`github/gh-actions-lock`のv0.5.1のREADMEとソースを読み、ADR-0030に書いた(2026-11-27)。
- 作業5(D-3): ADR-0030を書いた(2026-11-27)。
