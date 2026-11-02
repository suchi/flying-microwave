# ADR-0020: Windowsのリソースを、embed-resourceで自分で書いた`.rc`から埋め込む

- ステータス: 採用
- 日付: 2026-11-02

## 背景

実行ファイル(`.scr`)には、次のWindowsのリソースを入れる(計画0001のPhase 1の作業10)。

- アイコン: 配布物を見分けやすくするため。スクリーンセーバーのアイコンのIDは`ID_APP`(100)である([Windowsスクリーンセーバーの仕様調査](../research/windows-screensaver.md))。
- バージョン情報(VERSIONINFO): エクスプローラーのプロパティで版を確かめられるようにするため。
- 文字列表の序数1: スクリーンセーバーの説明。

Rustには、リソースを入れる手段が標準ではない。候補は[Phase 1のビルド・配布用ツールの調査](../research/phase1-tooling.md)で比べ、[計画0002](../plans/0002-phase1-tooling-proposal.md)のT-1で承認を得た。

## 決定

embed-resource 3.0.12を、ビルド時だけの依存(`build-dependencies`)として使う。

- 自分で書いたリソース定義ファイル`crates/app/res/app.rc`を、`crates/app/build.rs`からコンパイルする。
- バージョン情報は、ビルドスクリプトが`Cargo.toml`のバージョンからヘッダー(`fm_version.h`)を作って`.rc`に渡す。版を二重に管理しない。プレリリースの版(`-alpha.N`など)では、VERSIONINFOの`FILEFLAGS`にプレリリースの印を付ける。数値の版の4つめは常に0にする。
- `.rc`はシステムのヘッダー(`windows.h`・`winver.h`)を読み込まず、使う定数だけを自分で定義する。Linuxで、Windowsのターゲットを型チェックするときに`llvm-rc`があれば、ヘッダーがなくてもコンパイルできる。
- Windowsのホストでは、リソースコンパイラが見つからないときにビルドを失敗にする(`manifest_required`)。リソースのない実行ファイルを、気づかずに配布しないため。ほかのホストでは任意にする(`manifest_optional`)。
- embed-resourceは、入力のファイルをCargoに知らせない。ビルドスクリプトが`.rc`とアイコンについて`cargo:rerun-if-changed`を出す。
- WindowsのCIで、実行ファイルのバージョン情報が`Cargo.toml`の版と一致することを確かめる。
- アイコンは、SVG(`crates/app/res/icon.svg`)から、各サイズを別々に描画して`.ico`にまとめる(`scripts/icon/make_ico.py`)。`.ico`もリポジトリに置き、ビルドの時にはSVGの描画の道具を要らないようにする。今のアイコンは仮のもので、Phase 3で本番の絵にする。

### 検討した選択肢と採用しなかった理由

- winresource: 文字列表を直接扱うAPIがない。Windows以外のホストでは、呼び出し側で場合分けしないと失敗する。リソースが、テスト用の実行ファイルにも結び付けられる。リリースのタグがない。
- コンパイル済みの`.res`をリポジトリに置く: ビルド時に道具は要らないが、バージョン情報が固定になり、リリースのたびに作り直す二重管理になる。
- ビルドスクリプトから`rc.exe`を自分で呼ぶ: `rc.exe`の場所の探索(embed-resourceでは約100行)を自分で書くことになる。

## 結果

- メリット: 版は`Cargo.toml`だけで決まる。リソースの内容は、`.rc`を読めば分かる。Linuxでも、Windowsのターゲットの型チェックが壊れない。
- デメリット・受け入れたトレードオフ: ビルド時の依存crateが増える(embed-resourceを含めて7個。`toml`・`vswhom`・`winreg`など。配布物には入らない)。Linuxで`llvm-rc`があるとき、型チェックの途中でccの「GNU compiler is not supported for this target」という警告が出る(前処理にCのコンパイラを探すため。結果には影響しない)。
- 将来の見直し条件: Linux版・Mac版を作るとき(リソースはWindowsだけのもの)。embed-resourceの保守が止まったとき。アプリケーションマニフェストが必要になったとき(今はDPIの扱いをwinitが実行時に設定するため、入れていない)。
