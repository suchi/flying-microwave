# 技術スパイク

- ステータス: 作業中(計画0005の作業4)。ADRで3Dの描画の方式を決め、製品に移したあとで削除する
- 関連: [計画0005](../docs/plans/0005-v0.3.0-visuals-and-clock.md)のD-2〜D-4、E-1、E-5と作業4、[ビジュアルデザイン方針](../docs/design/visual-design.md)、[ADR-0023](../docs/adr/0023-svg-sprites-from-v0.2.0.md)

## このディレクトリについて

- 位置づけ: 見た目の案を作り、描画の方式を決めるための使い捨てのプログラムである。製品のコードではない。配布物に入らない。
- ワークスペースとの関係: ルートの`Cargo.toml`は`spikes/`を`exclude`している。各スパイクは空の`[workspace]`表を持つ独立したワークスペースで、`Cargo.lock`も個別に持つ。製品のコード(`crates/`)からスパイクを参照しない。
- 依存の公開からの日数: スパイクの`Cargo.lock`を変えたときは、`python3 -I scripts/check_publish_age.py spikes/<名前>/Cargo.lock`を実行する。CIは、このファイルを検査しない。

| ディレクトリ | 内容 |
| --- | --- |
| `spikes/render3d/` | 電子レンジを箱のメッシュに面の絵を貼って、深度と透視投影で描く(計画0005のD-3)。ウィンドウを作らず、画像(PNG)に描く |

## render3d

- 依存: `wgpu =30.0.1`(既定の機能を切り、`std`、`parking_lot`、`wgsl`、`vulkan`、`dx12`)、`pollster =1.0.1`、`resvg =0.49.0`(既定の機能を切る。面の絵のラスタライズとPNGの書き出し)。`vulkan`の機能は、このスパイクのためだけに使う(2026-11-10にメンテナーが承認した。計画0005の作業4の案A)。
- 面の絵: `src/art.rs`の中で自作したSVGで、外部の素材を使わない。
- 画像の作り方(Linux): ソフトウェアのVulkanの実装(lavapipe。Ubuntuでは`mesa-vulkan-drivers`)を入れ、`spikes/render3d/`で次を実行する。画像は`out/`にでき、コミットしない。

```bash
cargo run --release --locked -- out
```

- 作る画像: `tumbling-*.png`(それぞれ違う向きに回転して飛ぶ場面)、`level.png`と`level-facing.png`(全機が同じ向きで飛ぶ場面の、補正しない版と補正した版)、`closeup-*.png`(1機の拡大)、`atlas.png`(面の絵のアトラス)。
- Windows(DX12、WARP)では、確かめていない。
