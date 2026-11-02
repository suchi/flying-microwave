# Phase 1のビルド・配布用ツールの調査

- 調査日: 2026-11-01
- ステータス: 調査結果。採否は[Phase 1のビルド・配布用ツールの提案](../plans/0002-phase1-tooling-proposal.md)で決める
- 関連: [実装計画](../plans/0001-initial-implementation-plan.md)のPhase 1の作業10〜12と9章、[ADR-0011](../adr/0011-license-notices-with-cargo-about.md)、[ADR-0015](../adr/0015-ci-build-cache-and-spike-targets.md)

Phase 1の作業10(Windowsリソースの埋め込み)、作業11(依存crateのライセンス表記の生成)、作業12(リリースのワークフロー)で使う道具を調べた。

調べ方:

- 出典は、各プロジェクトのリポジトリのソース(タグ・コミットを固定)、crates.ioのAPI、`github/docs`リポジトリの原文である。
- 調査環境からは`docs.rs`、`doc.rust-lang.org`、`docs.github.com`、`api.github.com`、`github.blog`に接続できなかった。そのため、GitHubの公式ドキュメントは、同じ内容のソースである`github/docs`(mainの`33b1f4a`、2026-11-01取得)で読んだ。
- 各項目の「確認済み」はソースの読解または実行で確かめたもの、「推測」は確かめていないものを示す。

## 1. Windowsリソースの埋め込み(作業10)

埋め込むもの: アイコン、バージョン情報(VERSIONINFO)、文字列表(STRINGTABLE)の序数1(スクリーンセーバーの説明。[仕様調査](windows-screensaver.md))、必要ならアプリケーションマニフェスト。

### 候補の比較

| 項目 | embed-resource | winresource |
| --- | --- | --- |
| 最新版と公開日 | 3.0.12(2026-10-06) | 0.1.34(2026-10-12) |
| ライセンス | MIT | MIT |
| リポジトリ | `nabijaczleweli/rust-embed-resource`。リリースのタグあり(`v3.0.12`は`b4778a2`) | `BenjaminRi/winresource`(保守が止まった`winres`のフォーク)。リリースのタグなし。0.1.21・0.1.24・0.1.25は公開が取り消された |
| ビルド時の依存crate(配布物には入らない) | Linuxで13個(`cc`、`toml`など)。WindowsのMSVCでは約19個で、`vswhom-sys`がC++のコードをビルドする | 7個(`toml`と`version_check`) |
| 入力の形 | 自分で書いたリソース定義ファイル(`.rc`)をそのまま使う | APIから`.rc`を生成する。追記には`append_rc_content`を使う |
| 文字列表の序数1 | `.rc`にそのまま書ける | 専用のAPIがなく、`append_rc_content`で追記する |
| バージョン情報 | `.rc`に書く。Cargoのバージョンは、ビルドスクリプトからマクロ(`-D`)で渡す | Cargoの環境変数から自動で作る |
| Linuxでの型チェック(Windowsターゲット) | リソースのコンパイラ(`llvm-rc`)がない場合、`manifest_optional()`なら何もせずに成功する | 呼び出し側で`CARGO_CFG_TARGET_OS`を見て場合分けしないと、ビルドが失敗する。`llvm-rc`のほかに`clang`も必要 |
| Windowsでコンパイラが見つからない場合 | 失敗にする(`manifest_optional()`でも`Failed`は誤りとして返る) | 失敗にする |
| リソースを結び付ける対象 | 実行ファイルだけ(`rustc-link-arg-bins`) | テスト用の実行ファイルを含むすべて(`rustc-link-arg`) |
| 再ビルドの判定 | どちらも`cargo:rerun-if-changed`を出さない。ビルドスクリプトで自分で出す必要がある | 同左 |

確認済みの事実:

- embed-resourceの`manifest_optional()`は、`NotWindows`・`Ok`・`NotAttempted`を成功とし、`Failed`だけを誤りとして返す(`src/lib.rs`の341〜348行、`v3.0.12`)。
- embed-resourceは、リソースを実行ファイルにだけ結び付ける(`src/lib.rs`の443行、`cargo:rustc-link-arg-bins`)。
- `cargo:rerun-if-changed`を出さないことは、embed-resource自身の文書にも書かれている(`src/lib.rs`の69〜79行)。Linuxでは、ビルドスクリプトで出さないと、`.rc`を変えても古いリソースのままになることを実行して確かめた。
- `llvm-rc`(LLVM 18)で両方の候補を試し、アイコン・バージョン情報・マニフェスト・文字列表(序数1を含むブロック)が生成されることを確かめた。
- winresourceの文書は`FileDescription`に`package.description`を使うと書いているが、コードは`CARGO_PKG_NAME`を使っていた(`lib.rs`の143〜148行と198〜203行)。

推測:

- GitHubの`ubuntu-24.04`のx64ランナーには、`llvm-rc`が入っていない。導入スクリプトが`llvm-N`を入れるのはarm64のイメージだけで(`actions/runner-images`の`install-clang.sh`)、`llvm-rc`は`llvm-18`のパッケージに含まれるため。
- arm64向けのビルドでも、x64のホストの`rc.exe`を使う。`.res`はマシンに依存しないため問題はない。(2026-11-02の注: これはx64のホストでクロスビルドする場合の推測である。リリースのワークフローはarm64のホストでビルドするため、embed-resource 3.0.12は、Windows SDKのarm64の`rc.exe`を使う。[計画0002](../plans/0002-phase1-tooling-proposal.md)のT-3の実装の記録を参照)

crateを使わない方法:

- 自分でビルドスクリプトから`rc.exe`・`llvm-rc`を呼ぶ: `rc.exe`の場所の探索(embed-resourceでは約100行)を自分で書く必要がある。
- コンパイル済みの`.res`をリポジトリに置く: ビルド時に道具は要らないが、バージョン情報が固定になり、リリースのたびに作り直す二重管理になる(計画の7.2の方針と合わない)。

## 2. 依存crateのライセンス表記の生成(作業11)

cargo-aboutは[ADR-0011](../adr/0011-license-notices-with-cargo-about.md)で採用済み。

### 版と保守

- 最新版は0.9.3(2026-10-26)。ライセンスはMIT OR Apache-2.0、最小のRustのバージョンは1.88.0(`Cargo.toml`の10・12行、タグ`0.9.3`、コミット`8dd595a`)。
- 0.9.0〜0.9.3が2026-04〜10に公開されている。

### 導入方法

| 方法 | 内容 | 注意 |
| --- | --- | --- |
| crates.ioからソースでビルド | `cargo install --locked --features cli --version 0.9.3 cargo-about` | `--features cli`を付けないと、本体が入らないのに終了コード0で終わる(`Cargo.toml`の23〜29行の`required-features = ["cli"]`)。Linux(4コア)で約1分40秒、73個のcrateをビルドした。Windowsのランナーでの時間は測っていない |
| 配布されているバイナリ | `cargo-about-<タグ>-<ターゲット>.tar.gz`と`.sha256` | 署名や証明はなく、チェックサムは同じリリースに置かれているため、改ざんは防げない(推測) |
| 公式のGitHub Action | ない | - |

- crates.ioの`.crate`のSHA-256と、Cargoが取得したファイルのハッシュは一致した。`.crate`にはロックファイルが含まれ、`--locked`が効く。

### 設定と出力

- 設定ファイル`about.toml`の`accepted`(受け入れるライセンスのSPDX ID)は必須。`targets`で対象を`x86_64-pc-windows-msvc`と`aarch64-pc-windows-msvc`に絞ると、`cfg(unix)`だけで使うcrateは一覧から外れる(実行して確認)。
- `private = { ignore = true }`で、公開しないワークスペースのcrateを一覧から外せる。
- テキスト形式の見本はリポジトリにない。テンプレート(handlebars)は既定でHTML用のエスケープをするため、テキストでは`{{{text}}}`のように3重の波括弧が必要(実行して確認)。
- Windowsのランナーの既定のシェル(PowerShell)では、標準出力へのリダイレクトを拒むため、`-o`で出力ファイルを指定する(`generate.rs`の249〜259行)。

### 失敗の検出(実行して確認)

| 状況 | 結果 |
| --- | --- |
| `accepted`にないライセンス | 誤りとして終了コード1。出力ファイルは作られない |
| ライセンスの記載がないcrate | `--fail`を付けると終了コード1 |
| ライセンスのファイルが見つからない | 黙ってSPDXの定型文に置き換え、終了コード0 |
| `clarify`(ファイルの明示)のチェックサムの不一致 | 警告だけで、終了コード0 |

- `windows-sys`と`windows-link`は、ライセンスのファイル名が`license-mit`のため読み込まれない。その結果、MITの定型文の「Copyright (c) <year> <copyright holders>」がそのまま出力され、本来の「Copyright (c) Microsoft Corporation.」が抜けた。
- `clarify`でそのファイルとSHA-256を明示すると、正しい著作権者の行が出力された。
- 定型文への置き換えは、終了コードでは検出できない。出力に定型文の空欄(`<copyright holders>`など)が残っていないかの検査が別に必要である。

### ネットワーク

- ライセンスの本文は本体に埋め込まれており、生成時に取得しない。
- `cargo fetch`のあとに`--frozen`で実行すれば、ネットワークは使わない。`git`を使う`clarify`や回避策(workarounds)を使わない場合に限る。

## 3. リリースのワークフロー(作業12)

### 下書きのリリースの作成

- GitHubのWindowsランナー(`windows-2025`、イメージ20261021.299.1)には`gh`(2.102.1)が入っている(`actions/runner-images`の`Windows2025-Readme.md`の99行)。
- `gh release create <タグ> --draft --verify-tag <ファイル>...`で、第三者のActionを使わずに、下書きのリリースを作って配布物を載せられる(`cli/cli`の`v2.102.1`、`pkg/cmd/release/create/create.go`)。
- トークンには`contents: write`が必要(`github/docs`の`github-token-scope-descriptions.md`の18行)。
- `--draft`では、同じタグのリリースがあるかを確かめない。そのため、作る前に`gh release view <タグ>`で確かめる必要がある。アップロードに失敗した場合、作りかけの下書きは残る。

### ジョブの分け方

- x64・arm64のビルドのジョブ(読み取りの権限だけ)の成果物を、`actions/upload-artifact`と`actions/download-artifact`で、リリースのジョブに渡す形が公式の例にある(`store-and-share-data.md`の136〜147行)。
- 書き込みの権限を、依存crateのビルド(ビルドスクリプトや手続きマクロで任意のコードが動く)を行うジョブに持たせない方がよい。トークンを持つ工程が乗っ取られると、リポジトリに書き込めるため(`secure-use.md`の133行)。

### アーティファクト証明(ビルドの出所の証明)

- 「Free・Pro・Teamの契約では、公開リポジトリだけで使える。非公開・内部リポジトリでは、Enterprise Cloudの契約が必要」(`data/reusables/gated-features/attestations.md`)。
- 現在の公式の手順は`actions/attest`(v4.2.2、`1e69f48`)を使う。`actions/attest-build-provenance`は、それを包むだけのものになった。

### キャッシュ

- `cache-mode`(ジョブのキャッシュの権限)は、github.comで使える(`data/features/actions-cache-mode.yml`)。値は`read`・`write`・`write-only`・`none`。
- 省略した場合、`push`・`workflow_dispatch`で起動した実行は、キャッシュに書き込める。
- 公開されているワークフローの検査ツール(actionlintなど)は、まだ`cache-mode`を知らない(推測。ソースに該当がない)。

### タグの保護

- タグの保護は、ルールセット(rulesets)に置き換わった。作成・更新・削除を制限できる(`available-rules-for-rulesets.md`の25〜35行)。
- 非公開リポジトリでルールセットを使えるのは、Pro・Team・Enterprise Cloudの契約である(`data/reusables/gated-features/repo-rules.md`)。

### タグとバージョンの照合

- `cargo metadata --no-deps --locked --offline --format-version 1`で、ネットワークなしでワークスペースのバージョンを取得できた(実行して確認)。
- タグの名前はシェルの特殊文字を含みうる。環境変数で渡し、`^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$`のような厳密な正規表現で検査してから使う(`secure-use.md`の67〜80行)。
