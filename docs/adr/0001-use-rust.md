# ADR-0001: 言語・エコシステムにRustを採用する

- ステータス: 採用
- 日付: 2026-10-31

## 背景

[AGENTS.md](../../AGENTS.md)は、利用する言語・エコシステムに次を求めている。

- 2026年10月時点で一般的で信頼でき、オープンソースであること。
- 配布やインストールが容易であること。
- Windows版で実装し、将来のLinux版・macOS版を考慮して設計すること。

加えて、スクリーンセーバー固有の要件(`/p`のプレビュー枠への子ウィンドウ描画、全画面・マルチモニタ、起動の速さ)を満たす必要がある([Windowsスクリーンセーバーの仕様調査](../research/windows-screensaver.md)のW-1〜W-4)。

C#/.NET、Rust、C++、Go、Web技術を、普及・OSS、配布、`/p`への適合、将来のLinux/macOS、TDD、安全性の6基準で比較した。経緯と根拠は[言語・エコシステムの比較検討](../research/tech-stack.md)に記録している。実装計画の確認事項D-1として提示され、メンテナーが2026-10-31に決定した。

## 決定

実装言語にRustを採用する。ウィンドウ・入力にwinit、GPU描画にwgpu([ADR-0002](0002-render-with-wgpu.md))、設定画面にeframe/egui([ADR-0003](0003-settings-ui-with-eframe.md))を組み合わせる。

### ツールチェーンとビルドの方針

- ツールチェーン: Rustのstableチャンネルを使い、`rust-toolchain.toml`でバージョンを固定する。導入は`rustup toolchain install`で明示的に行い、自動インストールには頼らない。rustup 1.28.0で有効なツールチェーンの自動インストールが廃止され、1.28.1(2025-03-05)で既定では復活したが、環境変数`RUSTUP_AUTO_INSTALL`を`0`にすると無効になる(rustupのCHANGELOGで確認。[rustup CHANGELOG][RUSTUP-1])。調査時点(2026-10-31)の最新安定版は1.99.1(2026-10-13)である([channel-rust-stable.toml][RUST-1])。固定するバージョンは、eframe 0.37.0のMSRV(1.95)以上を満たす範囲で、雛形の作成時に決める。
- Windowsターゲット: `x86_64-pc-windows-msvc`と`aarch64-pc-windows-msvc`([ADR-0006](0006-target-windows-11-x64-arm64.md))。`.cargo/config.toml`で`-C target-feature=+crt-static`を指定し、VC++ランタイムを静的リンクする。`*-pc-windows-msvc`ターゲットは既定でCランタイムを動的リンクし`VCRUNTIME140.dll`に依存するとされる(Rustの既知の挙動だが、公式ドキュメントは調査時点で取得できず未確認)。CIで依存DLLを確認し、システムDLLだけに依存していることを確かめる。
- MSVCビルドツール: `*-pc-windows-msvc`のリンクにはMicrosoftのビルドツール(MSVCリンカとWindows SDK)が必要で、これらはOSSではない。言語とライブラリはOSSであるため、このトレードオフを受け入れる。完全にOSSのツールチェーンが必要になった場合のフォールバックとして、`*-windows-gnullvm`ターゲットを調査する(Rustでのサポート状況は要確認)。
- winit: 0.30系に固定し、eframeと同期させる。eframe 0.37.0は`winit ^0.30.13`に直接依存し、既定のwgpuバックエンドでは`egui-wgpu` 0.37.0を経由して`wgpu ^30.0`に依存する([crates.io][CIO-1])。winitには0.31.0-beta.4(2026-10-24)が出ているが、eframeが対応するまでは移行しない。

### 検討した選択肢と採用しなかった理由

| 選択肢 | 構成 | 採用しなかった理由 |
| --- | --- | --- |
| C#/.NET | .NET 10(LTS、サポート終了 2028-11-14。[.NETのサポートポリシー][NET-1]) + WinForms/WPF(Windowsのみ)またはAvalonia 12.1.4 | ランタイムが必要、または同梱でサイズが増える(未計測)。WinForms/WPFはWindows専用で将来の移植に向かない。Avaloniaの`/p`対応は要確認 |
| C++ | SDL3 + Dear ImGuiまたはWin32ダイアログ | メモリ安全でなく、ビルド構成(CMake・vcpkg等)が複雑。テスト基盤の選定・構築が必要。`/p`のための外部ウィンドウの扱いは要確認 |
| Go | Ebitengine v2.10.5([Go module proxy][GO-1]) | 単一バイナリにはなるが、外部HWNDの子として描画するAPIは確認できた範囲では見当たらない(未確認) |
| Web技術 | TauriまたはElectron | WebView2ランタイム依存またはChromium同梱で配布が重い。`/p`への対応も課題。早期に対象外と判断したため、バージョンは未確認 |

## 結果

- メリット:
  - 配布: VC++ランタイムを静的リンクすれば、追加のランタイムなしで動く単一実行ファイルになり、`.scr`へのリネームだけで配布できる。
  - `/p`への適合: winit 0.30.13の`WindowAttributes::with_parent_window`は、Windowsで`WS_CHILD`の子ウィンドウを作り親のクライアント領域に収める(ソースのドキュメントコメントで確認。[winit 0.30.13 `src/window.rs`][WINIT-1])。実機での動作はスパイクS1で確認する。
  - 依存の整合: スクリーンセーバー本体と設定画面で、同じwinit・wgpuを共有できる。
  - 移植性とテスト: winit・wgpu・eframe/eguiはWindows/macOS/Linuxに対応し、プラットフォーム非依存のロジックを独立したcrateにしてLinux上でも`cargo test`できる。
  - 安全性: メモリ安全で、`Cargo.lock`で依存を固定できる。
- デメリット・受け入れたトレードオフ:
  - ビルド時間が長く、wgpuを含むとバイナリサイズが大きくなる。目標値は実装計画の非機能要件で設定し、Phase 1で計測する。
  - eguiの設定画面はWindowsネイティブの見た目ではない([ADR-0003](0003-settings-ui-with-eframe.md))。
  - winit 0.31への移行で将来APIの破壊的変更が予想される。
  - winit 0.30.13は1プロセスにつきイベントループを1つしか作れない(2回目の生成は`EventLoopError::RecreationAttempt`になる。ソースで確認。[winit 0.30.13][WINIT-1])。起動モードの判定はイベントループの生成前に行う必要がある([アーキテクチャ](../design/architecture.md#起動モードの判定))。
  - MSVCビルドツール(非OSS)に依存する。
- 将来の見直し条件:
  - スパイクS1が不成立で、プレビュー(`/p`)の要件を満たせないとき。
  - `+crt-static`で静的リンクしても、クリーンな環境で起動しないとき。
  - 配布サイズが実装計画の目標を満たせないとき。
  - eframeがwinit 0.31系に対応したとき(言語の見直しではなく、バージョン移行として扱う)。
  - 完全にOSSのツールチェーンが必要になったとき(`*-windows-gnullvm`を評価する)。

[RUST-1]: https://static.rust-lang.org/dist/channel-rust-stable.toml
[CIO-1]: https://crates.io/
[RUSTUP-1]: https://github.com/rust-lang/rustup/blob/master/CHANGELOG.md
[NET-1]: https://dotnet.microsoft.com/en-us/platform/support/policy/dotnet-core
[GO-1]: https://proxy.golang.org/github.com/hajimehoshi/ebiten/v2/@latest
[WINIT-1]: https://github.com/rust-windowing/winit/blob/v0.30.13/src/window.rs
