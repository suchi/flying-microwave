# ドキュメント一覧

計画・調査結果・設計・レビュー記録など、検討の経緯はすべてこのディレクトリに残す。各ドキュメントの冒頭にステータス(提案・採用・合意済みなど)を書く。リポジトリは将来公開するため、個人を特定できる情報や会話のやり取りは残さない(AGENTS.md)。

| ディレクトリ | 内容 |
| --- | --- |
| `plans/` | 計画(SOW)。番号付きで、合意してから実装する |
| `research/` | 調査結果。一次情報の出典と取得日を書き、確認済みの事実と推測を区別する |
| `assets/` | 文書で使う画像(READMEのバナーなど)。作者とライセンスは[assets/README.md](assets/README.md)、作り方は[ビジュアルデザイン方針](design/visual-design.md)の「リポジトリのバナー」 |
| `design/` | 設計。合意前は「提案」として書く |
| `adr/` | ADR(アーキテクチャ決定記録)。運用は[adr/README.md](adr/README.md)に従う |
| `development/` | 開発の進め方(オンボーディング、レビュー手順) |
| `ideas/` | 将来のアイデア。採用したものは計画・設計に移す |
| `reviews/` | 敵対的レビューの指摘と対応の記録 |
| `incidents/` | 開発の中で起きた異常の、なぜなぜ分析と対策の記録。手順は[incidents/README.md](incidents/README.md) |

リポジトリ内のほかの文書:

| ファイル | 対象 |
| --- | --- |
| [README.md](../README.md) | リポジトリを見る人 |
| [packaging/windows/README.md](../packaging/windows/README.md) | 配布物の利用者(リリースのzipに`README.md`として同梱する) |
| [AGENTS.md](../AGENTS.md) | 開発に参加する人とAIコーディングエージェント(指示とルールの単一ソース) |

## 計画

| 番号 | タイトル | ステータス |
| --- | --- | --- |
| [0001](plans/0001-initial-implementation-plan.md) | 空飛ぶ電子レンジ スクリーンセーバー 初期実装計画 | 合意済み |
| [0002](plans/0002-phase1-tooling-proposal.md) | Phase 1のビルド・配布用ツールの提案 | 合意済み |
| [0003](plans/0003-v0.1.1-smoke-test.md) | v0.1.1 スモークテストと実機での確認 | 合意済み |
| [0004](plans/0004-v0.2.0-settings.md) | v0.2.0 設定画面(Phase 2) | 合意済み |

## 調査

| ドキュメント | 内容 |
| --- | --- |
| [windows-screensaver.md](research/windows-screensaver.md) | Windowsスクリーンセーバーの仕様 |
| [tech-stack.md](research/tech-stack.md) | 言語・エコシステムの比較検討 |
| [prior-art-and-ip.md](research/prior-art-and-ip.md) | 先行作品と知的財産上の配慮 |
| [spike-results.md](research/spike-results.md) | 技術スパイクの結果(記録中。残りの項目は製品で確かめる) |
| [phase1-tooling.md](research/phase1-tooling.md) | Phase 1のビルド・配布用ツール(リソースの埋め込み、ライセンス表記の生成、リリースのワークフロー) |

## 設計

| ドキュメント | 内容 | ステータス |
| --- | --- | --- |
| [architecture.md](design/architecture.md) | アーキテクチャ | 採用 |
| [settings.md](design/settings.md) | 設定項目仕様 | 採用 |
| [visual-design.md](design/visual-design.md) | ビジュアルデザイン方針 | 採用 |
| [mockups/](design/mockups/README.md) | v0.2.0の見た目の案(電子レンジの絵、設定画面) | 合意済み |

## ADR

一覧は[adr/README.md](adr/README.md)にある。初期実装計画の合意時に、ADR-0001〜0013を起票した。

## 開発の進め方

| ドキュメント | 内容 |
| --- | --- |
| [onboarding.md](development/onboarding.md) | 開発参加者向けオンボーディング |
| [review-process.md](development/review-process.md) | PRのレビュー手順(敵対的レビュー) |
| [hardware-check-v0.1.1.md](development/hardware-check-v0.1.1.md) | v0.1.1の実機での確認の手順 |

## アイデア

| ドキュメント | 内容 |
| --- | --- |
| [future-options.md](ideas/future-options.md) | 時間的に変化する項目・隠し要素などのアイデア |

## レビュー記録

| 番号 | 対象 |
| --- | --- |
| [0001](reviews/0001-plan-adversarial-review.md) | 初期実装計画と関連ドキュメント |
| [0002](reviews/0002-decision-round-review.md) | 決定事項の反映(ADR、運用ルール、READMEなど) |
| [0003](reviews/0003-roadmap-concerns-to-v1.md) | v1.0.0までの懸念事項の確認(メンテナーの判断を含む) |

## 異常の記録

| 番号 | 異常 |
| --- | --- |
| [0001](incidents/0001-english-text-shown-to-maintainer.md) | メンテナーに英語の文章が表示された |
| [0002](incidents/0002-extra-ci-run-after-review.md) | レビューの指摘の修正で、不要なCIが走った |
| [0003](incidents/0003-cancelled-maintainer-spike-run.md) | メンテナーのスパイクの手動実行を取り消した |
| [0004](incidents/0004-spike-output-not-visible.md) | 実機の確認で、描画が目に見えなかった |
| [0005](incidents/0005-english-notes-between-tool-calls.md) | 対策のあとも、ツール呼び出しの合間に英語の文を表示した |
| [0006](incidents/0006-plan-delayed-first-runnable-version.md) | 計画の順番が、最初に動く版を遅らせた |
| [0007](incidents/0007-dependencies-inside-cooldown.md) | 公開から7日たっていない依存の版を使った |
| [0008](incidents/0008-push-cancelled-running-pr-ci.md) | PRのCIの実行中にプッシュし、終わりかけのビルドを取り消した |
| [0009](incidents/0009-guard-hooks-never-ran.md) | 防御のフックが、最初から一度も働いていなかった |
| [0010](incidents/0010-onboarding-missed-build-steps.md) | 開発参加者向けの文書に、ビルドの方法が抜けていた |
| [0011](incidents/0011-log-written-against-requirement.md) | 診断用のログが、要件に反していつも書かれていた |
| [0012](incidents/0012-hook-check-tests-failed-on-windows.md) | フックの検査のテストが、Windowsのスパイクのビルドで失敗した |
| [0013](incidents/0013-arm64-smoke-test-only-at-release.md) | arm64のスモークテストを、タグを付けるまで試せず、リリースが2回続けて止まった |
