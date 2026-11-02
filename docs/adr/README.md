# ADR(アーキテクチャ決定記録)

技術選択・設計判断を記録する。1決定=1ファイルで`NNNN-title.md`(4桁ゼロ埋め連番・タイトルは英語kebab-case)、本文は日本語。テンプレート: [0000-template.md](0000-template.md)。

## 運用ルール

- 技術選択・設計判断をしたら必ずADRを残す。番号は既存の最大値+1。作成したら下の一覧に追加する。
- 例外: `main`にまだマージされていないADR(起票したPRの中にあるもの)は、レビューの指摘などを受けて同じPRの中で直接修正してよい。修正の経緯はPRとレビュー記録(`docs/reviews/`)に残し、決定内容を変える場合はメンテナーの承認を得る。
- 既存ADR(`main`にマージ済みのもの)の決定内容の書き換えは禁止。許される編集は以下の非推奨化手順(ステータス変更と冒頭への追記)のみ。決定が変わったら:
  1. 旧ADRのステータスを`採用`から`非推奨`に変え、冒頭に追記する:

     ```markdown
     - ステータス: ~~採用~~ 非推奨 → [ADR-NNNN: new-title](NNNN-new-title.md) に置き換え

     > 非推奨理由: ...
     ```

  2. 次の番号で新ADRを作成し、置き換えを明記する:

     ```markdown
     - 置き換え: [ADR-NNNN: old-title](NNNN-old-title.md) を非推奨とし、本ADRが最新の決定となる
     ```

  3. 下の一覧を更新し、非推奨を明示する。
- レビュー指摘を受けても技術的・環境的理由で現状を維持する場合は、該当コードに1〜3行の`Why:`コメント(ADR番号を参照)を書き、ADRにも記録する。

## 一覧

| 番号 | タイトル | ステータス | 日付 |
| --- | --- | --- | --- |
| [0000](0000-template.md) | テンプレート | - | - |
| [0001](0001-use-rust.md) | 言語・エコシステムにRustを採用する | 採用 | 2026-10-31 |
| [0002](0002-render-with-wgpu.md) | 描画にwgpu(GPU)を採用する | 採用 | 2026-10-31 |
| [0003](0003-settings-ui-with-eframe.md) | 設定画面にeframe/eguiを採用する | 採用 | 2026-10-31 |
| [0004](0004-store-settings-as-toml.md) | 設定をTOMLファイルで保存する | 採用 | 2026-10-31 |
| [0005](0005-license-mit.md) | ライセンスにMITを採用する | 採用 | 2026-10-31 |
| [0006](0006-target-windows-11-x64-arm64.md) | 対象環境をWindows 11のx64とarm64にする | 非推奨(ADR-0016に置き換え) | 2026-10-31 |
| [0007](0007-adversarial-review-without-copilot.md) | 敵対的レビューを別コンテキストのエージェントで行い、GitHub Copilotのレビューは使わない | 非推奨(ADR-0014に置き換え) | 2026-10-31 |
| [0008](0008-svg-assets-with-resvg.md) | 素材をSVGで作り、resvgでラスタライズする | 採用 | 2026-10-31 |
| [0009](0009-cargo-workspace-core-and-app.md) | Cargoワークスペースをcoreとappの2つのcrateに分ける | 採用 | 2026-10-31 |
| [0010](0010-deterministic-core-simulation.md) | シミュレーションを決定的にする | 採用 | 2026-10-31 |
| [0011](0011-license-notices-with-cargo-about.md) | 依存crateのライセンス表記をcargo-aboutで生成する | 採用 | 2026-10-31 |
| [0012](0012-standalone-app-mode.md) | 単独アプリとしてウィンドウ・全画面でも実行できる設計にする | 採用 | 2026-10-31 |
| [0013](0013-no-code-signing-for-v0.md) | v0.x系はコード署名せずに配布し、v1.0.0までに再判断する | 採用 | 2026-10-31 |
| [0014](0014-risk-based-review-depth.md) | 敵対的レビューの深さを変更の種類で決める | 採用 | 2026-10-31 |
| [0015](0015-ci-build-cache-and-spike-targets.md) | CIでビルドをキャッシュし、PRでのスパイクのビルドをx64に限る | 非推奨(ADR-0017に置き換え) | 2026-10-31 |
| [0016](0016-arm64-without-guarantee.md) | arm64版は動作を保証せずに配布し、CIではx64だけをビルドする | 採用 | 2026-11-01 |
| [0017](0017-ci-build-cache-and-x64-spike-builds.md) | CIでビルドをキャッシュし、スパイクのビルドをx64に限る | 採用 | 2026-11-01 |
| [0018](0018-review-before-opening-pr.md) | 敵対的レビューをPRを作る前に行う | 採用 | 2026-11-01 |
| [0019](0019-screensaver-windows-without-winit-fullscreen.md) | `/s`のウィンドウを、winitの全画面の指定を使わずに作る | 採用 | 2026-11-01 |
| [0020](0020-windows-resources-with-embed-resource.md) | Windowsのリソースを、embed-resourceで自分で書いた`.rc`から埋め込む | 採用 | 2026-11-02 |
| [0021](0021-spdx-sbom-with-own-script.md) | リリースのSBOMを、SPDX 2.3の形式で、自前のスクリプトで作る | 採用 | 2026-11-02 |
