# 設定項目仕様

- 作成日: 2026-10-31
- ステータス: 採用(2026-10-31)。保存形式は[ADR-0004](../adr/0004-store-settings-as-toml.md)に記録した。項目は導入フェーズの着手時に見直す(先回りで実装しない)。
- 関連: [アーキテクチャ](architecture.md)、[実装計画](../plans/0001-initial-implementation-plan.md)

## 設定項目

UIラベルはAGENTS.mdの言語ルールに従い英語とする。

| キー | UIラベル | タブ | 型 | 範囲・選択肢 | 既定値 | 導入 | 説明 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `flight.count` | Number of microwaves | Flight | 整数 | 1〜50 | 12 | v0.2.0 | 同時に飛ぶ電子レンジの数 |
| `flight.speed` | Speed | Flight | 小数 | 0.25〜3.0 | 1.0 | v0.2.0 | 飛行速度の倍率 |
| `flight.scale` | Size | Flight | 小数 | 0.5〜2.0 | 1.0 | v0.2.0 | 大きさの倍率 |
| `flight.style` | Flight style | Flight | 選択 | tumbling / level | tumbling | v0.3.0 | 飛び方。tumblingは、それぞれ違う向きと速さで回転しながら飛ぶ。levelは、全機が同じ向き(v0.2.0に近い見え方)で飛ぶ。食べ物も同じ向きで飛ぶ([計画0005](../plans/0005-v0.3.0-visuals-and-clock.md)のD-4、D-15、D-17) |
| `display.max_fps` | Frame rate limit | Display | 整数 | 0 / 30 / 60 / 120(0はモニタのリフレッシュレートに合わせる) | 60 | v0.2.0 | 描画フレームレートの上限。垂直同期は常に有効で、この値で間引く |
| `display.monitors` | Monitors | Display | 選択 | all / primary | all | v0.2.0 | アニメーションを表示するモニタ。primaryでも他のモニタは黒で覆う |
| `input.exit_sensitivity` | Mouse sensitivity | Display | 選択 | low / medium / high | medium | v0.2.0 | 終了と判定するマウス移動の閾値。スクリーン座標の物理ピクセルで、low=48、medium=16、high=4(2026-11-09に、`v0.2.0-alpha.4`の実機で、3つとも想定どおりに終わることを確かめた。計画0004のD-16) |
| `flight.food_count` | Number of food items | Flight | 整数 | 0〜30 | 4 | v0.3.0 | 電子レンジとは別に飛ぶ食べ物の数。電子レンジの数を変えずに追加できるよう、別の項目にする |
| `clock.enabled` | Show clock on display | Display | 真偽 | - | true | v0.3.0 | 電子レンジのLED表示に現在時刻を出す |
| `clock.format` | Clock format | Display | 選択 | 24h / 12h | 24h | v0.3.0 | 時刻の表示形式 |
| `theme.background` | Background | Extras | 選択 | black / night_sky / time_of_day | black | v0.4.0 | 背景(time_of_dayは時刻で変化) |
| `theme.seasonal` | Seasonal themes | Extras | 真偽 | - | true | v0.4.0 | 季節・記念日のテーマを有効にする |
| `extras.surprises` | Surprises | Extras | 真偽 | - | true | v0.5.0 | まれに起こる隠し要素を有効にする |

タブの導入時期: Flight・Display・AboutはPhase 2(v0.2.0)、ExtrasはPhase 4(v0.4.0)で追加する。

候補として保留する項目(必要性を確認してから追加する):

- 飛行方向(左下へ・右下へ・ランダム)
- バッテリー駆動時のフレームレート低減
- 効果音のオン・オフ(既定はオフ)

## 設定ファイル

TOML形式とする([ADR-0004](../adr/0004-store-settings-as-toml.md))。場所は`%APPDATA%\FlyingMicrowave\config.toml`([計画0004](../plans/0004-v0.2.0-settings.md)のD-1)。テストのために、環境変数`FLYING_MICROWAVE_CONFIG_DIR`(ドライブ文字で始まる絶対パス。空の値は指定なしとし、それ以外の値はエラーにして本当の設定ファイルを使わない)でフォルダを上書きできる([ADR-0027](../adr/0027-settings-file-handling-from-v0.3.0.md)のE-1。計画0004のD-12)。`schema_version`は、設定項目を加えた版で1つ上げる(v0.2.0は1、v0.3.0は2。古い版のファイルも読み、ないキーは既定値にする。[ADR-0027](../adr/0027-settings-file-handling-from-v0.3.0.md)のE-3)。

```toml
schema_version = 2

[flight]
count = 12
speed = 1.0
scale = 1.0
style = "tumbling"

[display]
max_fps = 60
monitors = "all"

[input]
exit_sensitivity = "medium"

[clock]
enabled = true
format = "24h"
```

## 検証規則

| 状況 | 動作 |
| --- | --- |
| ファイルが存在しない | すべて既定値 |
| ファイルが大きすぎる(上限は64KiBを想定) | 読まずにすべて既定値 |
| TOMLとして解析できない | すべて既定値 |
| 範囲外の数値 | 範囲内に丸める |
| 有限でない小数(TOMLの`nan`・`inf`) | その項目だけ既定値(丸めの前に判定する。NaNは丸めても範囲内にならないため) |
| 小数の項目に整数が書かれている(`speed = 2`など) | 小数として受け付ける |
| 整数の項目に小数が書かれている | その項目だけ既定値 |
| 選択肢にない値(`max_fps = 45`など)・その他の型の違い | その項目だけ既定値 |
| 未知のキー | 無視する(新しいバージョンの設定を古いバージョンで読んでも動く) |
| `schema_version`が新しい | 読める項目だけ使い、書き込み時に利用者へ確認する(最初のOKで確認の文を画面の中に示し、ボタンがReplaceに変わる。それを押すと書き込む。未知のキーとコメントは消える。[ADR-0027](../adr/0027-settings-file-handling-from-v0.3.0.md)のE-3) |
| `schema_version`がない・整数でない・0以下 | 1として読む([ADR-0027](../adr/0027-settings-file-handling-from-v0.3.0.md)のE-3) |

設定画面は、ファイルを読めなかったとき(ファイルが大きすぎる、TOMLとして解析できない、読み込みに失敗した)は既定値を示す。そのままOKで保存すると、もとのファイルを置き換えることになるため、新しい`schema_version`のときと同じく、最初のOKで警告を示し、Replaceに変わったボタンで保存する。ダブルクリックで読まずに保存しないよう、警告を出してから0.7秒は確認として数えない(計画0004のD-10)。

検証は`core::config`の純粋関数として実装し、上の各行をテストケースにする。

保存: 一時ファイルに書いてから置き換える(書きかけのファイルを`/s`が読まないようにするため)。

## 設定画面

```mermaid
flowchart LR
    open(["Settings opened"]) --> load["load config or defaults"]
    load --> edit["user edits values"]
    edit --> ok{"button"}
    ok -->|"OK"| save["validate and save"] --> close(["close"])
    ok -->|"Cancel"| close
    ok -->|"Defaults"| reset["reset form to defaults"] --> edit
```

- タブ構成: Flight / Display / Extras / About(Extrasは上記のとおりPhase 4から)。
- Aboutにはバージョン、ライセンス、依存ライブラリのライセンス表記、着想元についての説明([先行作品と知的財産上の配慮](../research/prior-art-and-ip.md)のIP-1の範囲)、プロジェクトのURLを表示する。
- 保存に失敗した場合は、設定画面内にエラーメッセージを表示する(設定画面は利用者の操作中なので表示してよい)。
- 選択肢のUIラベル: `flight.style`はTumbling・Level、`clock.format`は24-hour・12-hour、`display.max_fps`はMatch monitor・30 fps・60 fps・120 fps、`display.monitors`はAll monitors・Primary only、`input.exit_sensitivity`はLow・Medium・High。
- 設定画面を開けないとき(GPUの初期化の失敗など)は、メッセージボックスで、開けなかったことと設定ファイルのパスを示し、終了コード7で終わる([ADR-0027](../adr/0027-settings-file-handling-from-v0.3.0.md)のE-4)。GPUの初期化が15秒で終わらないときも同じ扱いにする(`/s`・`/p`と同じ上限。[計画0004](../plans/0004-v0.2.0-settings.md)のD-17、[計画0005](../plans/0005-v0.3.0-visuals-and-clock.md)のD-1)。
- Phase 1と`v0.1.x`の`/c`は、設定画面の代わりに「設定は今後のバージョンで追加する」旨のメッセージボックス(Win32の`MessageBoxW`)を表示していた。v0.2.0の試作版から、設定画面(eframe)を表示する。
