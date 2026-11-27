# ADR-0030: 外から取り込むものに公開から7日の待ちをかける範囲と、残るリスク

- ステータス: 採用
- 日付: 2026-11-27

## 背景

AGENTS.mdの「セキュリティ」は、依存(間接の依存とCIで入れるツールを含む)に、公開から7日以上たった版だけを使うと定めている(サプライチェーン攻撃対策。[incident 0007](../incidents/0007-dependencies-inside-cooldown.md))。2026-11-27に、メンテナーが、追加したPythonのスクリプトを含めて点検し、基本的にすべてに7日の待ちを置くよう指示した([計画0007](../plans/0007-supply-chain-cooldown.md))。

点検で、取り込むものを次のように分けた(2026-11-27に、リポジトリのファイルとワークフローで確かめた)。

| 取り込むもの | 待ちをかける仕組み |
| --- | --- |
| Rustの依存(`Cargo.lock`) | Dependabotの`cooldown`(7日)と、`scripts/check_publish_age.py`(CIとリリース) |
| CIで入れる`cargo-about`とその依存 | `check_publish_age.py`の`--crate`とそのcrateの`Cargo.lock` |
| Rustのツールチェーン(`rust-toolchain.toml`) | `check_publish_age.py`の`--rust-toolchain`(計画0007のD-2で加えた) |
| GitHub Actionsのアクション | Dependabotの`cooldown`(7日)と、完全なコミットのSHAでの固定 |
| Pythonのパッケージ | 使わない。`scripts/check_stdlib_only.py`が確かめる(計画0007のD-1) |
| resvg(`.ico`を作るときに手元で使う。`scripts/icon/make_ico.py`の`RESVG_VERSION`で版を固定) | 版を変えるときに、手で`check_publish_age.py --crate`を実行する(onboardingの手順) |
| actionlint(ワークフローの検査に手元で使う。onboardingで版を示す) | 版を変えるときに、人が公開日を確かめる |
| GitHubのランナーの中の道具(Python、jq、curl、rustup、ghなど) | なし(下の「決定」) |
| 手元の道具(メンテナーとエージェントの環境のPython、jq、Claude Code、ブラウザーなど) | なし(下の「決定」) |

## 決定

- 上の表の仕組みで、リポジトリが選ぶ版のすべてに7日の待ちをかける。Pythonは標準ライブラリだけに限り、外部のパッケージを加えるときは、メンテナーの承認、ハッシュ付きの固定、Dependabotの`cooldown`、公開からの日数の検査をそろえる(AGENTS.mdの「セキュリティ」)。
- `scripts/check_stdlib_only.py`は、許可の一覧で確かめる(2026-11-27のメンテナーの判断。計画0007のD-6)。Pythonのファイル(`.py`、`.pyw`、シェバンでPythonのもの)では、モジュール、組み込みの名前、`os`、`subprocess`、`shutil`、`sys`の属性、起動するプログラム、子のプロセスに足す環境変数を、すべて許可の一覧にする。Pythonを起動するファイル(ワークフロー、`.github/actions/`、Claude Codeのフックと`.claude/settings.json`のフックの登録)では、Pythonの起動の形(`-I scripts/<ファイル>.py`か`-I -m <一覧のモジュール>`)だけを許可の一覧にする。シェルのそのほかの中身(パッケージを入れる命令、環境変数の`PYTHONPATH`など)は、標準ライブラリでは文法を正しく読めず、命令の許可の一覧は誤りが多くなるため、この検査では見ない。それらのファイルは、変えるのにメンテナーの承認と本格的なレビューが要り(AGENTS.mdの「セキュリティ」)、この部分はそれに頼る。
- この検査が守る相手は、うっかりである(2026-11-27のメンテナーの判断。計画0007のD-7)。外部のパッケージに、誰も意図しないまま頼ることを防ぐ。わざと書かれた抜け道は、`scripts/`とワークフローの変更に要るPRとレビューに頼る。次のものは、検査では見ない。
  - Pythonの名前の解決(ほかのモジュールの属性を経て`os`に届く形、1つの場所で組み込みの名前を束縛し直す形)と、`ROOT`と`__file__`が指す場所。
  - 許可したプログラムの引数(`git -c alias`、`cargo install`、`check_hooks.py`の`sh -c`)と、`unittest`がモジュールを名前で読むこと。
  - `.claude/hooks/`の下のフォルダ、`.claude/settings.json`のフックの登録のほかのコマンド(`apiKeyHelper`など)、`.mcp.json`、`.claude/commands/`の手順の文。2026-11-27時点で、フックの下のフォルダ、`.mcp.json`、`apiKeyHelper`はない(`git ls-files`と`grep`で確かめた)。`.claude/commands/`の手順の文が示すPythonの起動は、どれも検査が許す形(`-I scripts/…`、`-I -m unittest discover -s scripts`)である。これらも、変えるのにメンテナーの承認が要る(AGENTS.mdの「セキュリティ」)。
- GitHubのランナーの中の道具には、待ちをかけない。版はGitHubがランナーのイメージを更新するときに決まり、リポジトリでは選べないため。ワークフローは、ランナーの`python3`で標準ライブラリだけを動かし、パッケージを入れない(Pythonの起動の形は`check_stdlib_only.py`が確かめ、そのほかは上のとおり承認とレビューに頼る)。
- 手元の道具も、このリポジトリの対策の外とする。リポジトリに版の記録がなく、メンテナーとエージェントの環境で決まるため。
- GitHub Actionsのワークフローのロックファイル(`gh actions-lock`。2026-10-28に一般提供)は、いまは採らない(2026-11-27のメンテナーの判断。計画0007のE-1)。移行は、メンテナーの指示で、Issue #112で追う。

ロックファイルを調べた結果(2026-11-27。GitHubの公式の文書「Workflow dependency lockfiles」、一般提供のお知らせ、`github/gh-actions-lock`のv0.5.1のREADMEとソース):

- 推移的な依存も固定し、元のリポジトリの同一性(数字のID)を確かめ、フォークだけにあるコミットを拒む。Freeのプランで使え、Dependabotの更新のPRがロックファイルも直す。
- 7日の待ちは強制しない。新しいタグは、固定するときに「情報」の重さの知らせになるだけで(`cmd/gh-actions-lock/fresh_tag.go`の`freshTagFinding`は`SeverityInfo`)、`--fail-on-warning`でも失敗にならない。同じファイルのコメントにも、cooldownは新しい版を止めないとある。
- 2026-11-27時点で、ワークフローが使うアクションはすべてSHAで固定し、中で別のアクションを呼ぶのは`actions/upload-pages-artifact`だけで、それもSHAで固定している(各アクションの`action.yml`で確かめた)。ロックファイルで得られる差は小さい。
- 使うには、`uses:`を変えるたびに、認証した`gh`で生成し直す必要がある。エージェントの環境の`gh`は検索にしか使えないため、メンテナーの作業が増える。

検討した選択肢と採用しなかった理由:

- ロックファイルをいま採る: 上のとおり、いまの差が小さく、待ちも強制しないのに、メンテナーの作業と、許可するアクション(`github/gh-actions-lock`)が増える。
- 手で加えるアクションの公開日をCIで確かめる: Dependabotの`cooldown`は更新にしか効かないため、手で加えたアクションの版は人の確認だけになる。GitHubのAPIでタグやコミットの日付を調べる検査は作れるが、メンテナーが計画0007で対象にしなかった。
- ランナーの道具の版を固定する(`actions/setup-python`などで入れる): 入れるためのアクションと、その配布物が新しく増え、取り込むものが減らない。

## 結果

- メリット: CIとリリースで使うもの(Rustの依存、cargo-about、ツールチェーン)は、7日の待ちを機械で確かめる。GitHub Actionsは、Dependabotの更新に待ちがかかる。Pythonは外部のパッケージを使わない。
- デメリット・受け入れたトレードオフ:
  - ランナーと手元の道具の版は、7日の待ちの外にある。
  - 手で加えるアクションの版と、手元で使う道具(resvg、actionlint)の版は、人の確認だけになる。
  - `check_publish_age.py`は`tomllib`を使うため、Python 3.11以上が要る。
- 将来の見直し条件: タグで中のアクションを呼ぶアクションを加えるとき。`gh actions-lock`が1.0になるときか、待ちを強制できるようになるとき。手で加えたアクションの版の誤りが起きたとき。Pythonの外部のパッケージが必要になったとき。
