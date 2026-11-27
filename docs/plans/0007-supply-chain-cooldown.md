# 0007: 外部から取り込むものすべてに、公開から7日の待ち(cooldown)をかける

- ステータス: 案(2026-11-27)
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

## 判断していただいたこと

2026-11-27に、メンテナーが、次の3つを対策すると決めた。

- D-1: Pythonのスクリプトは標準ライブラリだけに限る。
- D-2: Rustのツールチェーンの版も、公開から7日以上たっていることをCIで確かめる。
- D-3: 対策できずに残るリスクを、ADRに記録する。

あわせて、GitHub Actionsは、GitHubのワークフローのロックファイル(`gh actions-lock`の拡張)を使うほうがよさそうという考えが示された。採るかどうかは、下の作業4で調べてから決める(E-1)。

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

- E-1: GitHub Actionsのワークフローのロックファイルを採るか。採るときは、どのワークフローに載せるか、CIで検証するか、リポジトリの設定(ロックファイルを必須にする方針など)を使うか。作業4の結果を示して、メンテナーが決める。2026-11-27時点で分かっていること(`github/gh-actions-lock`のREADMEとリリースの一覧):
  - ロックファイルは`.github/workflows/actions.lock`で、直接と推移的なアクションを、確かめたコミットに固定する。
  - 拡張の最新版はv0.5.1(2026-11-19 22:15 UTCの公開)で、2026-11-27には公開から7日をすぎている。
  - 2026-10-28に一般提供になったと、GitHubのお知らせ(`github.blog`)にある。そのページはエージェントの環境から接続できず、中身(対象のプラン、設定)は確かめていない。
