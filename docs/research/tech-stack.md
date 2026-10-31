# 言語・エコシステムの比較検討

- 調査日: 2026-10-31
- ステータス: 提案(ユーザーの承認待ち)。承認後、決定内容をADRとして`docs/adr/`に記録する。
- 関連: [実装計画](../plans/0001-initial-implementation-plan.md)、[Windowsスクリーンセーバーの仕様調査](windows-screensaver.md)

## 評価基準

AGENTS.mdのプロジェクト概要と、[Windowsスクリーンセーバーの仕様調査](windows-screensaver.md)で整理した要件から、次の基準で比較する。

| ID | 基準 | 由来 |
| --- | --- | --- |
| C1 | 2026年10月時点で一般的・信頼でき、オープンソースであること | AGENTS.md |
| C2 | 配布・インストールが容易であること(単一ファイル、ランタイム不要、サイズ) | AGENTS.md |
| C3 | スクリーンセーバー固有の要件を満たせること(`/p`の子ウィンドウ描画、全画面・マルチモニタ、起動の速さ) | 仕様調査 W-1〜W-4 |
| C4 | 将来のLinux版・macOS版を見据えられること | AGENTS.md |
| C5 | TDDで進めやすいこと(描画以外のロジックを単体テストできる) | AGENTS.md(開発フロー) |
| C6 | 安全性(メモリ安全、依存関係の管理しやすさ) | AGENTS.md(セキュリティ) |

## 候補

バージョンは2026-10-31に各パッケージレジストリで確認した最新安定版。

| 候補 | 構成 | 確認したバージョン | ライセンス |
| --- | --- | --- | --- |
| A. Rust | winit(ウィンドウ)+ wgpu(GPU描画)+ eframe/egui(設定画面) | Rust 1.99.0(2026-10-01リリース)[R-1]、winit 0.30.13、wgpu 30.0.1、eframe 0.37.0 [R-2] | winit: Apache-2.0、wgpu・eframe: MIT OR Apache-2.0 [R-2] |
| B. C#/.NET | .NET 10(LTS)+ WinForms/WPF(Windowsのみ)またはAvalonia(クロスプラットフォーム) | .NET 10 LTS(サポート終了 2028-11-14)[R-3]、Avalonia 12.1.4 [R-4] | MIT |
| C. C++ | SDL3 + Dear ImGui(設定画面)またはWin32ダイアログ | SDL 3.4系(Rustバインディング`sdl3-sys` 0.7.2のバージョン表記「+SDL-3.4.18」による間接確認。SDL公式リリースページは本セッションで未取得)[R-2] | SDL: zlib |
| D. Go | Ebitengine | v2.10.5(2026-10-06)[R-5] | Apache-2.0 |
| E. Web技術 | Tauri 2またはElectron | 未確認(早期に対象外と判断したため) | MIT等 |

## 比較

| 基準 | A. Rust | B. C#/.NET | C. C++/SDL3 | D. Go/Ebitengine | E. Web技術 |
| --- | --- | --- | --- | --- | --- |
| C1 普及・OSS | 適合 | 適合 | 適合 | 適合 | 適合 |
| C2 配布 | 適合: ランタイム不要の単一exe | 課題あり: ランタイムが必要、または同梱でサイズ増(未計測)。WinForms/WPFのNative AOT対応状況は要確認 | 適合: 静的リンクで単一exe可 | 適合: 単一バイナリ | 課題あり: WebView2ランタイム依存またはChromium同梱 |
| C3 `/p`子ウィンドウ | 適合見込み: winitがWindowsで`WS_CHILD`の子ウィンドウ作成をサポート(ソースで確認)[R-6] | 適合見込み: WinFormsならWin32 APIの呼び出しで可能(一般的な手法。一次情報未確認)。Avaloniaは要確認 | 要確認: 既存のネイティブウィンドウを扱うプロパティがある(要確認) | 課題あり: 外部HWNDの子として描画するAPIは確認できた範囲では見当たらない(未確認) | 課題あり |
| C4 将来のLinux/macOS | 適合: winit・wgpu・eframe/eguiはWindows/macOS/Linux対応 | 一部適合: Avaloniaなら可。WinForms/WPFはWindowsのみ | 適合 | 適合 | 適合 |
| C5 TDD | 適合: `cargo test`が標準、ロジックをcrateとして分離しやすい | 適合 | 一部適合: テスト基盤の選定・構築が必要 | 適合 | 一部適合 |
| C6 安全性 | 適合: メモリ安全、`Cargo.lock`で依存を固定 | 適合: メモリ安全 | 課題あり: メモリ安全でない、ビルド構成(CMake・vcpkg等)が複雑 | 適合: メモリ安全 | 一部適合: 依存が大きい |

## 推奨: A. Rust

理由:

- 配布の容易さ: ランタイム不要の単一実行ファイルになり、`.scr`へのリネームだけで配布できる(C2)。
- `/p`プレビューへの適合: winit 0.30.13の`WindowAttributes::with_parent_window`は、Windowsで`WS_CHILD`スタイルの子ウィンドウを作り親のクライアント領域に収める(ソースのドキュメントコメントで確認)[R-6]。プレビュー枠への描画を全画面と同じコードで実現できる見込みがある(スパイクS1で確認)。
- 依存の整合性: eframe 0.37.0は`winit ^0.30.13`と`wgpu ^30.0`に依存しており[R-2]、スクリーンセーバー本体と設定画面で同じwinit・wgpuを共有できる。
- 将来の移植性: winit・wgpuはWindows/macOS/Linuxをサポートし、プラットフォーム非依存のロジックを独立したcrateにしてLinux上でもテストできる(C4・C5)。
- Win32 APIへのアクセス: 必要な箇所(HWNDの検証など)はMicrosoft公式の`windows-sys`(windows-rs)で呼べる。

デメリット・受け入れるトレードオフ:

- ビルド時間が長く、wgpuを含むとバイナリサイズが大きくなる(目標値は計画の非機能要件で設定し、Phase 1で計測する)。
- eguiの設定画面はWindowsネイティブの見た目ではない。
- winitは0.31のベータ版(0.31.0-beta.4、2026-10-24)が出ており[R-2]、将来APIの破壊的変更が予想される。eframeが0.30系に依存している間は0.30系に固定し、eframeの対応に合わせて移行する。
- `x86_64-pc-windows-msvc`ターゲットのリンクにはMicrosoftのビルドツール(MSVCリンカとWindows SDK)が必要で、これらはOSSではない。言語とライブラリはOSSで、CIのWindowsランナーで利用できる見込み(Phase 0で確認)。完全にOSSのツールチェーンが必要なら`*-windows-gnullvm`ターゲットを検討する(Rustでのサポート状況は要確認)。

## 描画方式の比較

| 方式 | 構成 | 長所 | 短所 |
| --- | --- | --- | --- |
| GPU(推奨) | wgpu 30.0.1 | 4K・複数モニタでもCPU負荷が低い。拡大縮小・回転・半透明がほぼ無償。将来の演出(パーティクル、背景のグラデーション)に余裕がある | シェーダ(WGSL)とスプライトバッチの実装が必要。GPUが使えない環境への対応が必要(スパイクS6) |
| CPU | softbuffer 0.4.8 + tiny-skia 0.12.0 | 実装が単純。描画結果をテストで画素比較しやすい | 4K(3840x2160)の32bitフレームは約33MBで、60fpsで全面を描き直すと毎秒約2GBのメモリ書き込みになる(概算・未計測)。省電力性に不利 |

推奨: GPU(wgpu)。スクリーンセーバーはPCの放置中に長時間動くため、CPU負荷と消費電力を抑えることを優先する。描画以外のロジックはcoreに分離して単体テストし、描画結果の自動テストは後回しにする(必要になればwgpuのオフスクリーン描画で行う)。

## 設定画面の比較

| 方式 | 確認したバージョン・ライセンス [R-2] | 長所 | 短所 |
| --- | --- | --- | --- |
| eframe/egui(推奨) | 0.37.0、MIT OR Apache-2.0 | クロスプラットフォーム。本体とwinit・wgpuを共有できる。実装量が少ない | ネイティブの見た目ではない。`/c`の親ウィンドウに対するモーダル化は要確認(スパイクS2) |
| Win32ネイティブダイアログ | `windows-sys` 0.61.2、MIT OR Apache-2.0 | 見た目がWindows標準。モーダル化が容易 | Windows専用。将来OSごとに作り直しが必要。コード量が多い |
| Slint | 1.18.1、GPL-3.0-only OR ロイヤリティフリー OR 商用 | 宣言的UI。見た目の自由度が高い | ライセンス体系が複雑で、本プロジェクトのライセンス選択に影響する |
| iced | 0.14.0、MIT | クロスプラットフォーム | eframeと比べて本体とのwinit/wgpuバージョン整合を別途管理する必要がある(要確認) |

## 依存crateの候補(Rust採用時)

依存の追加はAGENTS.mdにより事前承認が必要なため、候補を一覧にする。各フェーズの着手時に改めて承認を得る。バージョン・ライセンスは2026-10-31にcrates.ioで確認した。[R-2]

| crate | バージョン | ライセンス | 用途 | 導入フェーズ |
| --- | --- | --- | --- | --- |
| winit | 0.30.13 | Apache-2.0 | ウィンドウ・入力・モニタ列挙 | Phase 1 |
| wgpu | 30.0.1 | MIT OR Apache-2.0 | GPU描画 | Phase 1 |
| pollster | 1.0.1 | Apache-2.0/MIT | wgpuの非同期初期化の待機 | Phase 1 |
| bytemuck | 1.25.2 | Zlib OR Apache-2.0 OR MIT | GPUバッファへのデータ変換 | Phase 1 |
| windows-sys | 0.61.2 | MIT OR Apache-2.0 | Win32 API(HWNDの検証など) | Phase 1 |
| fastrand | 2.5.0 | Apache-2.0 OR MIT | シード指定可能な乱数(シミュレーションの再現性) | Phase 1 |
| embed-resource または winresource | 3.0.12 / 0.1.34 | MIT | アイコン・バージョン情報・String Tableの埋め込み(ビルド時のみ) | Phase 1 |
| resvg | 0.49.0 | Apache-2.0 OR MIT | SVG素材のラスタライズ | Phase 1または3 |
| eframe | 0.37.0(MSRV 1.95) | MIT OR Apache-2.0 | 設定画面 | Phase 2 |
| serde | 1.0.229 | MIT OR Apache-2.0 | 設定のシリアライズ | Phase 2 |
| toml | 1.1.8 | MIT OR Apache-2.0 | 設定ファイル形式 | Phase 2 |
| directories | 6.0.0 | MIT OR Apache-2.0 | OSごとの設定保存場所の解決 | Phase 2 |
| jiff または chrono | 0.2.38 / 0.4.45 | Unlicense OR MIT / MIT OR Apache-2.0 | ローカル時刻(時計表示・季節テーマ) | Phase 3 |
| rodio | 0.23.0 | MIT OR Apache-2.0 | 効果音(将来・任意) | 未定 |

引数解析は対象が4種類と少ないため、crateを使わず自前で実装する(YAGNI)。

## 対象外とした選択肢

- Python(pygame等): 配布にPyInstaller等の同梱が必要で、起動も遅くなりやすい(C2・C3)。
- ゲームエンジン(Godot、Bevy等): スクリーンセーバーの規模に対して大きく、`/p`の子ウィンドウ描画への対応が難しい(C2・C3)。

## 参考資料

- [R-1] Rust Blog「Announcing Rust 1.99.0」(2026-10-01)
- [R-2] crates.io API(各crateのメタデータ・依存関係・ライセンス、2026-10-31取得)
- [R-3] .NET and .NET Core official support policy(2026-10-31取得)
- [R-4] NuGet Gallery API: Avalonia(2026-10-31取得)
- [R-5] Go module proxy: github.com/hajimehoshi/ebiten/v2(2026-10-31取得)
- [R-6] winit 0.30.13のソースコード`src/window.rs`(`WindowAttributes::with_parent_window`のドキュメントコメント。crates.ioから取得したソースパッケージで確認)

[R-1]: https://blog.rust-lang.org/2026/10/01/Rust-1.99.0/
[R-2]: https://crates.io/
[R-3]: https://dotnet.microsoft.com/en-us/platform/support/policy/dotnet-core
[R-4]: https://www.nuget.org/packages/Avalonia
[R-5]: https://proxy.golang.org/github.com/hajimehoshi/ebiten/v2/@latest
[R-6]: https://github.com/rust-windowing/winit/blob/v0.30.13/src/window.rs
