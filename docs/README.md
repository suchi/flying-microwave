# ドキュメント一覧

計画・調査結果・設計・レビュー記録など、検討の経緯はすべてこのディレクトリに残す。各ドキュメントの冒頭にステータス(提案・合意済みなど)を書く。

| ディレクトリ | 内容 |
| --- | --- |
| `plans/` | 計画(SOW)。番号付きで、合意してから実装する |
| `research/` | 調査結果。一次情報の出典と取得日を書き、確認済みの事実と推測を区別する |
| `design/` | 設計。合意前は「提案」として書く |
| `ideas/` | 将来のアイデア。採用したものは計画・設計に移す |
| `reviews/` | 敵対的レビューの指摘と対応の記録 |
| `adr/` | ADR(アーキテクチャ決定記録)。運用は[adr/README.md](adr/README.md)に従う |

## 計画

| 番号 | タイトル | ステータス |
| --- | --- | --- |
| [0001](plans/0001-initial-implementation-plan.md) | 空飛ぶ電子レンジ スクリーンセーバー 初期実装計画 | 提案 |

## 調査

| ドキュメント | 内容 |
| --- | --- |
| [windows-screensaver.md](research/windows-screensaver.md) | Windowsスクリーンセーバーの仕様 |
| [tech-stack.md](research/tech-stack.md) | 言語・エコシステムの比較検討 |
| [prior-art-and-ip.md](research/prior-art-and-ip.md) | 先行作品と知的財産上の配慮 |

## 設計

| ドキュメント | 内容 | ステータス |
| --- | --- | --- |
| [architecture.md](design/architecture.md) | アーキテクチャ | 提案 |
| [settings.md](design/settings.md) | 設定項目仕様 | 提案 |
| [visual-design.md](design/visual-design.md) | ビジュアルデザイン方針 | 提案 |

## アイデア

| ドキュメント | 内容 |
| --- | --- |
| [future-options.md](ideas/future-options.md) | 時間的に変化する項目・隠し要素などのアイデア |

## レビュー記録

| 番号 | 対象 |
| --- | --- |
| [0001](reviews/0001-plan-adversarial-review.md) | 初期実装計画と関連ドキュメント |
