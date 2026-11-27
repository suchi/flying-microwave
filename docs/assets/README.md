# 文書で使う画像

ルートのREADMEなど、文書で使う画像を置く。素材ごとの作者・ライセンス・出典を、ここに記録する([先行作品と知的財産上の配慮](../research/prior-art-and-ip.md)の方針IP-3)。

| ファイル | 内容 | 作者 | ライセンス | 出典・外部素材 |
| --- | --- | --- | --- | --- |
| `banner.svg` | ルートのREADMEの先頭に置くバナー(1280×360) | 本プロジェクトで作成 | MIT(リポジトリと同じ。[LICENSE](../../LICENSE)) | 本プロジェクトの面の絵(`assets/sprites`、MIT)から`scripts/icon/make_art.py`で作った。外部の素材は使っていない。文字はSVGの要素で書き、フォントは埋め込まず、見る環境のものを使う |
| `site/img/microwave-*.svg`(このフォルダの外) | ランディングページ(計画0006)で飛ぶ電子レンジ。色ごとに4つ | 本プロジェクトで作成 | MIT(リポジトリと同じ。[LICENSE](../../LICENSE)) | 本プロジェクトの面の絵(`assets/sprites`、MIT)から`scripts/icon/make_art.py`で作った。外部の素材は使っていない |
| `site/index.html`と`site/en/index.html`の中のGitHubのマーク(このフォルダの外) | リポジトリへのリンクのアイコン | GitHub Inc. | MIT(Copyright (c) 2026 GitHub Inc.) | Octicons 19.42.0(npmの`@primer/octicons`)の`mark-github-24.svg`のパスをそのまま使った。版は公開から7日以上たったものを選んだ |
| `crates/app/res/icon.svg`(このフォルダの外) | 実行ファイルのアイコンの元の絵(256×256)。`.ico`はここから`scripts/icon/make_ico.py`で作る | 本プロジェクトで作成 | MIT(リポジトリと同じ。[LICENSE](../../LICENSE)) | 本プロジェクトの面の絵(`assets/sprites`、MIT)から`scripts/icon/make_art.py`で作った。外部の素材は使っていない |

作り方と方針は、[ビジュアルデザイン方針](../design/visual-design.md)の「リポジトリのバナーとアイコン」にある。
