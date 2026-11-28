# ドキュメント一覧

計画・調査結果・設計・レビュー記録など、検討の経緯はすべてこのディレクトリに残す。各ドキュメントの冒頭にステータス(提案・採用・合意済みなど)を書く。個人を特定できる情報や会話のやり取り、作成者個人の事情(意志、希望、好み、人柄、私的な行動や活動)、リポジトリを公開する時期、意向、理由は残さない。決定の理由は残す(AGENTS.mdの「ドキュメント」の節)。

| ディレクトリ | 内容 |
| --- | --- |
| `plans/` | 計画(SOW)。番号付きで、合意してから実装する |
| `research/` | 調査結果。一次情報の出典と取得日を書き、確認済みの事実と推測を区別する |
| `assets/` | 文書で使う画像(READMEのバナーなど)。作者とライセンスは[assets/README.md](assets/README.md)、作り方は[ビジュアルデザイン方針](design/visual-design.md)の「リポジトリのバナーとアイコン」 |
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
| [SECURITY.md](../SECURITY.md) | 脆弱性を報告する人(対象の版と、非公開の報告の方法) |
| [CONTRIBUTING.md](../CONTRIBUTING.md) | 貢献する人(ライセンス、素材の出所の条件、PRの流れ) |
| [AGENTS.md](../AGENTS.md) | 開発に参加する人とAIコーディングエージェント(指示とルールの単一ソース) |

## 計画

| 番号 | タイトル | ステータス |
| --- | --- | --- |
| [0001](plans/0001-initial-implementation-plan.md) | 空飛ぶ電子レンジ スクリーンセーバー 初期実装計画 | 合意済み |
| [0002](plans/0002-phase1-tooling-proposal.md) | Phase 1のビルド・配布用ツールの提案 | 合意済み |
| [0003](plans/0003-v0.1.1-smoke-test.md) | v0.1.1 スモークテストと実機での確認 | 完了(v0.1.1) |
| [0004](plans/0004-v0.2.0-settings.md) | v0.2.0 設定画面(Phase 2) | 完了(v0.2.0) |
| [0005](plans/0005-v0.3.0-visuals-and-clock.md) | v0.3.0 見た目と時計(Phase 3) | 完了(v0.3.0) |
| [0006](plans/0006-landing-page.md) | 利用者向けのランディングページ(GitHub Pages) | 合意済み |
| [0007](plans/0007-supply-chain-cooldown.md) | 外部から取り込むものすべてに、公開から7日の待ち(cooldown)をかける | 完了 |
| [0008](plans/0008-agents-md-reorganization.md) | AGENTS.mdを原則だけにし、手順を文書へ移す | 合意済み |

## 調査

| ドキュメント | 内容 |
| --- | --- |
| [windows-screensaver.md](research/windows-screensaver.md) | Windowsスクリーンセーバーの仕様 |
| [tech-stack.md](research/tech-stack.md) | 言語・エコシステムの比較検討 |
| [prior-art-and-ip.md](research/prior-art-and-ip.md) | 先行作品と知的財産上の配慮 |
| [spike-results.md](research/spike-results.md) | 技術スパイクの結果(記録中。残りの項目は製品で確かめる) |
| [phase1-tooling.md](research/phase1-tooling.md) | Phase 1のビルド・配布用ツール(リソースの埋め込み、ライセンス表記の生成、リリースのワークフロー) |
| [ci-second-monitor.md](research/ci-second-monitor.md) | CIで2台目のモニターを用意する方法(計画0005の作業13。CIでの確認は見送りと決めた。D-45) |

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
| [publication-checklist.md](development/publication-checklist.md) | 公開の手順の一覧(メンテナーが画面で行う作業。計画0005の作業15の2、D-47) |
| [public-repo-migration.md](development/public-repo-migration.md) | 公開用のリポジトリへの移行の手順(計画0005の作業15の2。移行はしないと決めたため不採用。D-46) |
| [hardware-check-v0.1.1.md](development/hardware-check-v0.1.1.md) | v0.1.1の実機での確認の手順 |
| [hardware-check-v0.2.0-alpha.1.md](development/hardware-check-v0.2.0-alpha.1.md) | v0.2.0-alpha.1(試作版)の実機での確認の手順 |
| [hardware-check-v0.3.0-alpha.3.md](development/hardware-check-v0.3.0-alpha.3.md) | v0.3.0-alpha.3(試作版)の実機での確認の手順。確認は行わず(計画0005のD-21)、v0.3.0-alpha.4の手順書の元にする |
| [hardware-check-v0.3.0-alpha.4.md](development/hardware-check-v0.3.0-alpha.4.md) | v0.3.0-alpha.4(試作版)の実機での確認の手順。v0.3.0-alpha.3の確認の項目と、キーの遅れの直し方の確認(計画0005の作業10の3) |
| [hardware-check-v0.3.0-alpha.5.md](development/hardware-check-v0.3.0-alpha.5.md) | v0.3.0-alpha.5(試作版)の実機での確認の手順。v0.3.0-alpha.4で確かめられなかった項目と、終了の速さの直しの確認(計画0005の作業10の6) |

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
| [0004](reviews/0004-pre-publication-audit.md) | 公開の前の点検(Issue #3) |
| [0005](reviews/0005-pre-publication-threat-review.md) | 公開の前の脅威の点検(計画0005の作業15。2026-11-19に判断を受け取った。残りは作業15の2) |
| [0006](reviews/0006-pre-publication-threat-recheck.md) | 公開の前の脅威の再点検(計画0005のD-47。2026-11-25に判断を受け取った) |

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
| [0014](incidents/0014-second-monitor-not-drawn.md) | 2台目のモニタに電子レンジが描かれないまま、リリースした |
| [0015](incidents/0015-english-reports-after-stop-hook.md) | メンテナーへの報告を英語で書き、一部はフックでも止まらなかった |
| [0016](incidents/0016-tag-before-merge.md) | 試作版のタグが、マージの前のコミットに付いた |
| [0017](incidents/0017-gitignore-entry-removed-with-spikes.md) | spikesの削除で、.gitignoreのビルドの生成物のエントリも消した |
| [0018](incidents/0018-english-progress-note-again.md) | 0015の対策のあとも、合間の報告を英語で書いた |
| [0019](incidents/0019-hooks-miss-crlf-from-native-jq.md) | Windowsのネイティブのjqでは、設定の保護のフックが確認を求めないおそれがある |
| [0020](incidents/0020-incident-status-not-updated.md) | incidentのステータスが、対策のマージのあとも古いままだった |
| [0021](incidents/0021-settings-init-timeout-missing.md) | 設定画面のGPUの初期化に、要件にある上限がないまま出した |
| [0022](incidents/0022-note-hook-reads-transcript-too-early.md) | 合間の文を確かめるフックが、記録の書き込みより先に動き、何も止めていなかった |
| [0023](incidents/0023-tag-check-command-failed-in-cmd.md) | タグの前に件名を確かめるコマンドが、Windowsのコマンドプロンプトで動かなかった |
| [0024](incidents/0024-unfounded-guess-as-premise.md) | 記録にない過去の事実を、根拠のない推測で補い、異常の記録の前提にしようとした |
| [0025](incidents/0025-implemented-against-recorded-decision.md) | 記録した決定と違う方式を、確認を取らずに実装した |
| [0026](incidents/0026-incident-row-placed-in-procedure.md) | incidentの一覧の行を、手順の箇条の間に入れてマージした |
| [0027](incidents/0027-log-contents-stated-without-reading-code.md) | ログに何が書かれるかを、ログを書くコードを読まずに文書に書いた |
| [0028](incidents/0028-smoke-failure-merged-on-a-guess.md) | スモークテストの失敗を、原因を確かめないまま再実行で通してマージし、再発した |
| [0029](incidents/0029-incident-status-stale-again.md) | 0020の対策のあとも、incidentのステータスがマージのあとで古いままだった |
| [0030](incidents/0030-push-checked-check-runs-not-workflow-runs.md) | プッシュの前にPRのチェックだけを見て、待ち行列のCIの実行に気づかずにプッシュした |
| [0031](incidents/0031-requested-publishing-an-already-published-release.md) | 公開済みのリリースの公開を依頼し続け、確かめていない前提で判断を仰いだ |
| [0032](incidents/0032-committed-after-a-failing-check.md) | 検証のコマンドの失敗を見ずに、同じ呼び出しの中でコミットした |
| [0033](incidents/0033-exit-time-near-limit-unnoticed.md) | スモークテストの終了の時間が上限の近くまで延びたことに8日間気づかず、1回の実行の失敗を、値を比べずに案の不採用の根拠にした |
| [0034](incidents/0034-incident-list-test-skipped.md) | incidentを記録したコミットで、手順が求める一覧のテストを実行しなかった |
| [0035](incidents/0035-key-to-screen-gone-failed-after-adr-0028.md) | ADR-0028の直し方のあとも、F-01のキーから画面が消えるまでが上限を超えた |
| [0036](incidents/0036-session-link-in-pr-body.md) | PRの本文に、セッションのリンクが入ったまま、マージの直前まで気づかなかった |
| [0037](incidents/0037-english-note-missing-from-transcript.md) | 合間の英語の文が会話の記録に残らず、日本語を求めるフックが止めなかった |
| [0038](incidents/0038-release-notes-not-updated-after-hardware-check.md) | 公開したプレリリースの実機での確認の結果を、CHANGELOGとリリースノートに反映しなかった |
| [0039](incidents/0039-main-ci-failed-unnoticed-after-merge.md) | マージのあとのmainのCIが2回続けて失敗し、気づかなかった |
| [0040](incidents/0040-hook-implemented-against-reported-design.md) | 承認された設計と違う方式で、フックを確認なしに実装した |
| [0041](incidents/0041-dates-written-in-utc.md) | 記録の日付を、JSTに直さずにUTCの日付で書いた |
| [0042](incidents/0042-rule-change-not-reflected-in-other-docs.md) | 規則を変えたときに、同じ説明を書いたほかの文書を直し漏らした(再発) |
| [0043](incidents/0043-delegated-implementation-skipped-tdd-red.md) | 実装を任せたサブエージェントが、先に失敗するテストを書かなかった |
| [0044](incidents/0044-site-check-fix-left-the-same-holes.md) | ページの読み込みの検査の抜け道を直したあとも、同じ種類の抜け道が残った |
| [0045](incidents/0045-actions-allow-list-entered-without-separators.md) | 公開の手順の一覧が許可するアクションの書き方を示さず、CIが始まらなくなった |
| [0046](incidents/0046-release-premise-without-diffing-the-last-tag.md) | 前の版のタグからの変更を確かめずに、正式なリリースの中身を「試作版と同じ機能」と書いた |
| [0047](incidents/0047-exchange-written-into-decision-records.md) | 決定の記録に、会話のやり取りを書いた |
