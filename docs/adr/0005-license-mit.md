# ADR-0005: ライセンスにMITを採用する

- ステータス: 採用
- 日付: 2026-10-31

## 背景

AGENTS.mdは、利用する技術にオープンソースであることを求めている。一方、本プロジェクト自身のライセンスは未定だった。リポジトリは将来公開する予定であり、公開と配布のためにライセンスを決める必要がある。

選択肢としてMIT OR Apache-2.0、MIT、Apache-2.0、GPL-3.0を比較した。実装計画の確認事項D-6として提示され、当初の推奨はMIT OR Apache-2.0(Rustの慣例でwgpu・eframeと同じ)だった。メンテナーが2026-10-31にMITに決定した。

主要な依存のライセンスは、[言語・エコシステムの比較検討](../research/tech-stack.md#依存crateの候補rust採用時)で確認している(2026-10-31に[crates.io][CIO-1]で確認)。winitはApache-2.0、wgpu・eframe・resvgなどはMIT OR Apache-2.0またはApache-2.0 OR MIT、bytemuckはZlib OR Apache-2.0 OR MITである。eframeの既定フォントだけが別のライセンスを含み、これは[ADR-0003](0003-settings-ui-with-eframe.md)で差し替える。

## 決定

本プロジェクトのライセンスにMITを採用する。

- 理由: 簡潔で寛容である。OSIの承認ライセンスであり、SignPath Foundationによる無償のコード署名を将来利用する場合の条件(すべての構成要素がOSIの承認ライセンスであること。[ADR-0013](0013-no-code-signing-for-v0.md))を満たせる。依存crateの寛容なライセンスとも組み合わせて配布できる。
- 著作権者の表記: 「Flying Microwave contributors」とする。リポジトリを公開したときに、特定の個人が識別されないようにするため。
- 第三者のライセンス表記: 依存crateなどのライセンス表記は、[ADR-0011](0011-license-notices-with-cargo-about.md)の方法で生成し、配布物に同梱する。
- LICENSEファイル: リポジトリのルートの[LICENSE](../../LICENSE)に置く。

### 検討した選択肢と採用しなかった理由

- MIT OR Apache-2.0: 当初の推奨。Rustの慣例でwgpu・eframeと同じ、winit(Apache-2.0)とも両立する。2つのライセンス文を同梱する必要があり、MITだけの簡潔さを優先して採用しなかった。
- Apache-2.0: 特許条項がある。ただしMITよりやや長く、簡潔さを優先して採用しなかった。
- GPL-3.0: 派生物もOSSにできる。しかし利用の制約が強いため採用しなかった。

## 結果

- メリット:
  - ライセンス文が短く、利用者・貢献者が理解しやすい。
  - OSIの承認ライセンスであり、SignPath Foundationの条件を満たせる。
  - 依存crateの寛容なライセンスとの組み合わせが単純である。
- デメリット・受け入れたトレードオフ:
  - Apache-2.0にある特許条項がない。
  - MIT OR Apache-2.0の二重ライセンスというRustの慣例とは異なる。
  - 依存crateのライセンス表記を、配布物に漏れなく同梱する運用が必要になる。
- 将来の見直し条件:
  - 特許条項が必要になったとき(Apache-2.0やMIT OR Apache-2.0への変更を検討する。ライセンスの変更は、新しいADRで非推奨化の手順に従って記録する)。
  - 組み込む依存やコードのライセンスが、MITでの配布と両立しなくなったとき。

[CIO-1]: https://crates.io/
