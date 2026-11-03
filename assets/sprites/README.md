# スプライトの素材

画面を飛ぶ電子レンジの素材。SVGで描き、起動時にresvgでラスタライズしてテクスチャにする([ADR-0008](../../docs/adr/0008-svg-assets-with-resvg.md)、[ビジュアルデザイン方針](../../docs/design/visual-design.md#素材の作り方))。素材は実行ファイルに埋め込む(`crates/app/src/sprite.rs`の`include_str!`)。

## 一覧

| ファイル | 内容 | 作者 | ライセンス | 出典 |
| --- | --- | --- | --- | --- |
| `microwave-body-silver.svg` | 本体(シルバー)。天面・側面・前面、通気口、操作パネルとLEDの時計、ドアの開口部と灯りのついた庫内 | Flying Microwave contributors | MIT | 本リポジトリで新たに作成 |
| `microwave-body-graphite.svg` | 本体(グラファイト)。色のほかは上と同じ | Flying Microwave contributors | MIT | 本リポジトリで新たに作成 |
| `microwave-body-white.svg` | 本体(ホワイト)。色のほかは上と同じ | Flying Microwave contributors | MIT | 本リポジトリで新たに作成 |
| `microwave-body-mint.svg` | 本体(ミント)。色のほかは上と同じ | Flying Microwave contributors | MIT | 本リポジトリで新たに作成 |
| `microwave-door.svg` | ドア。半透明の窓と取っ手 | Flying Microwave contributors | MIT | 本リポジトリで新たに作成 |

外部の素材(画像・フォント・クリップアート)は使っていない。

## 描き方の決まり

- 座標: 本体は320×320の正方形で、絵をその中央に置く。描画では、この正方形をシミュレーションの飛行物体の正方形(`SpriteInstance::size`)に合わせ、-8°傾ける。傾けても正方形からはみ出さないよう、余白を残している。
- ドア: 本体の座標で左上が(24, 100)、大きさが156×140の長方形に収まる。描画のとき、左の縦の辺(ヒンジ)を固定した平行四辺形に変形して開閉させる(`crates/app/src/sprite.rs`)。ドアの座標を変えたときは、そのファイルの定数も合わせる。
- 文字: `<text>`は使わない。resvgの文字の機能を切っているため(LEDの時計は四角で描いた7セグメント風の表示)。
- 外部の参照: `<image>`や外部ファイルへの参照は使わない(読み込みは無効にしている)。
- ガイドライン: 庫内のマグカップのように、安全な使い方だけを描く(G-2)。
