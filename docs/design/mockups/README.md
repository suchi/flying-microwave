# 見た目の案(v0.2.0)

[計画0004](../../plans/0004-v0.2.0-settings.md)のD-5による、静的な見た目の案。2026-11-03にエージェントが作り、メンテナーが確かめて合意した。製品の実装は、この案をもとに作る。実装と違いがあれば、実装を正とし、この案は検討の経緯として残す。

| ファイル | 内容 |
| --- | --- |
| `microwave-sheet.svg`、`microwave-sheet.png` | 電子レンジの絵。ドアを閉じた状態と、55°・110°に開いた状態(羽ばたきの案A。[ビジュアルデザイン方針](../visual-design.md))。本体の色の違い(silver、graphite、white) |
| `microwave-scene.svg`、`microwave-scene.png` | 全画面の場面の例。大きさ・色・傾き・ドアの角度の違う電子レンジが、夜の背景を飛ぶ(mintを含む4色) |
| `microwave-icon.svg` | 設定画面の案の上部に置いた、ドアを閉じた電子レンジ |
| `settings.html` | 設定画面の案(eframe/eguiの部品で作れる配置に限った)。`?tab=flight`、`?tab=display`、`?tab=about`でタブを選ぶ |
| `settings-flight-100.png`、`settings-display-100.png`、`settings-about-100.png` | 設定画面の案の各タブ(拡大率100%) |
| `settings-display-150.png` | 拡大率150%での表示(配置が崩れないことの確認) |

- 作り方: SVGは、エージェントがPythonのスクリプトで座標を計算して書いた(斜め上から見た平行投影。ドアは左のヒンジで回る)。画像は、Playwrightのchromiumで描画して撮った。
- 作者とライセンス: Flying Microwave contributors、MIT。外部の素材は使っていない。
- 知的財産: 鳥の翼は付けず、電子レンジの部品(ドア)で飛ぶ姿にした(IP-2)。着想元の名称や絵柄は使っていない(IP-1、IP-3)。
- 表現: 庫内には、マグカップだけを置いた(危険な使い方を描かない。G-2)。
- 設定画面の案のAboutのタブは、ライセンスの行だけで、依存ライブラリのライセンスの本文を示す領域がない。実装では、この行の下に、`THIRD-PARTY-LICENSES.txt`の本文を表示するスクロールできる領域があり、着想元の説明とURLはその下に置く([計画0004](../../plans/0004-v0.2.0-settings.md)のD-12)。案の画像と`settings.html`は、検討の経緯として直さない。
- 設定画面の案のLEDの時計の文字は、ブラウザのフォントで描いた。製品では、文字を使わずに描く(resvgの文字の描画は使わないため)。
