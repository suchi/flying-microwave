# 0001: 初期実装計画の敵対的レビュー記録

- 実施日: 2026-10-31
- 対象: [初期実装計画](../plans/0001-initial-implementation-plan.md)と関連ドキュメント(`docs/research/`、`docs/design/`、`docs/ideas/`、`docs/README.md`、`README.md`、AGENTS.mdの変更)の初版(コミット`bc07719`〜`7738f13`)
- 方法: 計画の作成者とは別のコンテキストを持つエージェント(Claude Codeのサブエージェント)に、AGENTS.mdと対象ファイルの一覧だけを渡し、作成者の結論を渡さずに欠陥を探させた。レビュアーは読み取り専用で、ファイルを変更していない。
- 指摘の扱い: 指摘は検証すべき主張として扱い、作成者が一次情報(公式ドキュメント、crateのソース、レジストリ、GitHub API)で確認できたものは「確認済み」、確認できなかったが内容が妥当と判断したものは「未確認・妥当」とした。

## 結果の概要

- 指摘: 26件(high 4件、medium 9件、low-medium 3件、low 10件)
- 対応: 26件すべてを計画・設計に反映した。一部は判断をメンテナーに委ねる確認事項(D-13〜D-17)として追加した。
- 不採用: なし(部分的に採用したものは「対応」欄に記載)

## 指摘と対応

| No. | 重要度 | 指摘の要約 | 検証 | 対応 | 反映先 |
| --- | --- | --- | --- | --- | --- |
| 1 | high | リポジトリが非公開で、Releaseの配布・署名・証明などの前提が成り立たない | 確認済み: GitHub APIで`"private": true` | 不整合I-6、リスクR-12、確認事項D-13(公開の時期)、Phase 0に公開前の確認を追加 | plan 2章・7章・9章・11章・12章 |
| 2 | high | 複数モニタでウィンドウを作るたびにフォーカス喪失が起き、起動直後に終了する。合成キーイベント、キーの解放、カーソル座標の基準、閉じる要求、タッチの扱いが未定義 | 確認済み: winit 0.30.13のソースで`active`の既定値true、`is_synthetic`、`RegisterTouchWindow`を確認 | 終了条件を「アプリ全体の非アクティブ化」に変更し、合成イベント・キーの解放の無視、起動直後の猶予時間、スクリーン座標での判定、タッチと閉じる要求を追加。テストケースとして明記 | architecture「入力と終了判定」、plan F-02・Phase 1・R-15 |
| 3 | high | Smart App Controlが署名なしの`.scr`をブロックする | 確認済み: Microsoft Learn「Smart App Control」に「unknown, unsigned code are blocked by default」 | 仕様調査に保護機能の節を追加、R-03に追記、確認事項D-14(v0.x系の署名方針)、READMEに既知の制限を書く作業と手動テスト項目を追加 | windows-screensaver、plan 9章・11章・12章・8.2 |
| 4 | high | msvcターゲットは既定でVC++ランタイムに動的リンクするため「ランタイム不要」は成り立たない | 未確認・妥当: Rustの既知の挙動だが、調査時には公式ドキュメントを取得できなかった | `+crt-static`での静的リンク、CIでの依存DLLの確認、クリーンな環境での手動テストを追加。記述を「静的リンクすれば」に修正 | tech-stack、architecture、plan N-09・Phase 0・Phase 1・8.2・R-16 |
| 5 | medium | DLL検索順序の乗っ取りへの対策がない。wgpu-halはDLLをパスなしの名前で読み込み、DX12のコンパイラの既定値は`dxcompiler.dll`を先に試す | 確認済み: wgpu-hal 30.0.1とwgpu-types 30.0.1のソースで、`d3d12.dll`・`dxgi.dll`・`dxcompiler.dll`の読み込みと、`Dx12Compiler`の既定値`Auto`を確認 | 要件W-11、セキュリティ対策SEC-1・SEC-2、非機能要件N-13、リスクR-13を追加。APIとリンカ設定の詳細はPhase 1で公式ドキュメントを確認してから適用する | windows-screensaver、architecture「セキュリティ」、plan |
| 6 | medium | 検証コマンドがLinux環境で実行できる保証がない。`rustup show`による自動インストールに頼るのは不確実 | 未確認・妥当: rustupのCHANGELOGは調査時に未取得 | 原則A-7(`app`もLinuxでビルドできる状態を保つ)を追加。セットアップを`rustup toolchain install`に、ビルド系に`--locked`を追加。CIのLinuxジョブをワークスペース全体に変更 | architecture、tech-stack、plan Phase 0 |
| 7 | medium | eframeの既定フォントのライセンスに、OSIの承認ライセンスでないUbuntu Font Licenceが含まれる。D-6の「主要な依存と同じ」は不正確 | 確認済み: crates.ioで`epaint_default_fonts` 0.37.0のライセンスが`(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0`、eframeの既定機能に`default_fonts`を確認。OSIの一覧は未確認 | eframeの既定機能を無効にしてフォントを差し替える方針を追加。N-12・R-14を追加。D-6の記述を修正 | tech-stack、plan |
| 8 | medium | `/c`はMS-1では前面のウィンドウに対するモーダルだが「親なし」としていた。eframeでのモーダル化の見通しが楽観的 | 確認済み: MS-1の原文、egui-winit 0.37.0のソースにオーナー指定の手段がなく、`CreationContext`がウィンドウハンドルを提供することを確認 | `/c`は前面のウィンドウをオーナーにする規則に変更。W-2の根拠を修正。S2の確認方法(生成後にWin32 APIでオーナーを設定)と代替案(Win32ダイアログ)を明記 | windows-screensaver、architecture |
| 9 | medium | Windowsの起動・終了の振る舞いを自動テストしていない | 未確認・妥当 | WARPを使うWindowsランナーでのスモークテストを追加。ホストランナーで実行できるかはPhase 0で確認する(部分採用) | plan 8.1・Phase 0・Phase 1 |
| 10 | medium | スパイクに必要な依存の承認時期とスパイクの置き場所が不明。resvgはPhase 3でよい。Phase 1の仮の設定画面の方式が未定 | 未確認・妥当(計画の内部の矛盾) | 依存の承認を「最初に使う時点の前」に変更。スパイクは`spikes/`に置きワークスペースに含めない。resvgをPhase 3へ。Phase 1の`/c`は`MessageBoxW` | tech-stack、architecture、settings、plan |
| 11 | medium | Dependabotの既存グループのままだと、0.x系crateの破壊的な更新がまとめて来る。R-05にwgpuがない | 未確認・妥当: Dependabotの更新種別の判定は調査時に未確認 | egui系・winit・wgpuを一緒に更新するグループ分けと、CIでの重複検出を追加。R-05にwgpuを追加 | plan Phase 0・R-05 |
| 12 | medium | 手動テストのチェックリストに漏れがある。arm64は未検証のまま出荷される | 未確認・妥当 | 「設定」ボタン、ダブルクリック・右クリックメニュー、インターネットから取得したファイル、クリーンな環境、空白・日本語のパス、スリープ復帰、タッチ、Smart App Control、arm64(実機がなければ未検証と明記)を追加。arm64のホストランナーの利用は、利用条件を確認できていないため採用していない(部分採用) | plan 8.2、windows-screensaver S3・S5 |
| 13 | medium | Phase 4〜6に完了条件がない。バージョンを上げる手順が未定義 | 未確認・妥当(計画の内部の不足) | 「全フェーズ共通のリリース条件」と「リリース手順」を追加し、各フェーズから参照 | plan 7.1・7.2 |
| 14 | low-medium | winitは1プロセスにつきイベントループを1つしか作れない制約が書かれていない | 確認済み: winit 0.30.13のソースで`EVENT_LOOP_CREATED`と`RecreationAttempt`を確認 | モード判定をイベントループの生成前に行う制約を追加 | architecture「起動モードの判定」、tech-stack |
| 15 | low-medium | 設定仕様の穴(NaN、`max_fps`の型、機数に食べ物を含める問題、閾値の単位、アトミックな保存、整数と小数、タブの配置) | 未確認・妥当 | 有限でない値は既定値、小数項目は整数を受け付ける、`max_fps`は整数(0はリフレッシュレートに合わせる)、食べ物は`flight.food_count`として分離、閾値をスクリーン座標の物理ピクセルで定義(暫定値)、一時ファイルを使った保存、タブ列を追加 | settings、architecture |
| 16 | low-medium | スクリーンセーバーの目的の1つ「機密情報を隠す」から来る要件が抜けている | 確認済み: MS-2に「To conceal sensitive information left on a screen」 | 仕様調査の要点に2つの目的を追記。W-9(起動直後から黒で覆う)・W-10(モニタの追加に対応)を追加し、ガイドラインG-4の根拠を修正 | windows-screensaver、architecture、visual-design、plan F-01・F-07 |
| 17 | low | GPU・電力・動きの滑らかさの考慮不足(WARP、初期化失敗時に静かに終了すると直後にロックされる、ディスプレイ消灯中の描画、省電力GPU、補間) | 未確認・妥当 | S6にWARPを追加。`/s`で描画の初期化に失敗したら黒い画面のまま入力を待つ方針に変更。省電力GPUの優先、ディスプレイ消灯中の描画停止(N-14)、描画時の補間(N-10)を追加 | windows-screensaver、architecture、plan |
| 18 | low | 参考資料のIDが本文中の要件・リスク・ガイドラインのIDと紛らわしい | 確認済み(文書の内部) | 参考資料のIDに出典の接頭辞を付けた(`APL-1`、`XSS-1`、`GH-1`、`SP-1`、`RUST-1`、`CIO-1`など) | architecture、tech-stack、plan |
| 19 | low | Rustの最新安定版は1.99.0ではなく1.99.1 | 確認済み: `channel-rust-stable.toml`に`version = "1.99.1 (58cf8f9c3 2026-10-13)"` | 1.99.1に修正し、出典をチャンネルのマニフェストに変更 | tech-stack、plan 5章 |
| 20 | low | MS-5のURLが取得できない形式。2つの調査文書にステータスの行がない | 確認済み: Microsoft Learnのライフサイクルのページで「Windows 10 will reach end of support on October 14, 2025」を取得 | MS-5を差し替え。windows-screensaverとprior-art-and-ipにステータスの行を追加 | plan、research |
| 21 | low | SignPathの条件の要約に漏れ(実績の要件、リリース済みであること、アンインストールの提供、すべての構成要素) | 確認済み: signpath.orgの条件のページで原文を確認 | 9章の要約に追記。D-14で署名できない場合の扱いを判断事項にした | plan 9章・12章 |
| 22 | low | IP-1(READMEでの言及のみ)と設定画面のAboutの内容が矛盾。仮称の商標の確認がない | 確認済み(文書の内部) | IP-1をREADMEとAboutでの説明に揃えた。製品名の決定前にUSPTOとJ-PlatPatで確認する作業を追加 | prior-art-and-ip、settings、plan D-5 |
| 23 | low | `/s`の多重起動と、余分な引数の扱いが未定義 | 未確認・妥当 | `/s`の単一起動(名前付きミューテックス)と余分な引数を無視する規則を追加(N-15) | architecture、plan |
| 24 | low | リリースワークフローのサプライチェーン対策(ActionsのSHA固定、最小権限、タグの保護、`--locked`)が足りない | 未確認・妥当 | R-08と9章、Phase 0のCIの作業に追加 | plan |
| 25 | low | D-5の期限が遅い(Phase 0でcrate名を作る)。D項目にpros/consがない。素材形式・ワークスペース構成・ライセンス表記ツールがD項目になっていない | 確認済み(文書の内部) | D-5の期限をPhase 0の開始前に変更。全D項目に主な長所・短所を追加。D-15〜D-17を追加 | plan 12章 |
| 26 | low | 乱数crateは出力列の安定性を保証しておらず、固定シードのテストが依存の更新で壊れうる | 未確認・妥当(fastrandの方針は調査時に未確認) | 小さな乱数生成器を`core`に自前で実装し、既知の出力列でテストする方針に変更。依存候補からfastrandを削除 | tech-stack、architecture、plan Phase 1 |

## レビュアーが問題なしとした項目(要約)

- crates.ioのバージョン・ライセンス・依存関係の値(winit、wgpu、eframe、windows-sysなど)
- winitの`with_parent_window`のドキュメントコメントとソースの一致、モニタごとのDPI対応の設定
- Microsoft Learnの記述(MS-1〜MS-4)の原文との一致
- .NET 10のサポート終了日、Windows 10のサポート終了日、Ebitengine・Avaloniaのバージョン
- 相対リンクと日本語見出しのアンカーの解決、絵文字・太字を使っていないこと、図がすべてMermaidであること

## 次回への教訓

- 配布の前提(リポジトリの公開状態、Windowsの保護機能、署名)は、技術選定より先に確認する。
- ライブラリの既定値(フォーカス、DLLの読み込み、既定機能のライセンス)は、ドキュメントの要約ではなくソースとメタデータで確認する。
