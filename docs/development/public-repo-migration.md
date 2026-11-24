# 公開用のリポジトリへの移行の手順(案)

- ステータス: 案(メンテナーの承認を待っている)
- 作成日: 2026-11-24
- 関連: [計画0005](../plans/0005-v0.3.0-visuals-and-clock.md)の作業15の2とE-7、D-22、[公開の前の脅威の点検](../reviews/0005-pre-publication-threat-review.md)のJ-7、J-8、J-9

履歴を書き換えて、新しい公開用のリポジトリに移す(D-22)ための手順の案である。書き換えるのは、次の2つである。

- テンプレートの作者のアカウント名(5つのコミットの差分に残っている。J-8)
- コミットのトレーラーの、セッションのURL(`Claude-Session:`の行。J-7)

## 同じリポジトリでは書き換えない理由

GitHubの文書「[Removing sensitive data from a repository](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/removing-sensitive-data-from-a-repository)」は、履歴を書き換えて強制的にプッシュしても、古いコミットがPRの参照やキャッシュされた表示から辿れると書いている("Through any pull requests that reference them")。`refs/pull/`は読み取り専用で、利用者は消せない。消せるのはGitHub Supportだけで、Supportは機密でないデータの削除は引き受けない("GitHub Support won't remove non-sensitive data")。アカウント名とセッションのURLを機密として扱ってもらえるかは、文書に書かれていない。

いまのリポジトリには、`refs/pull/*/head`が95本(PR #1〜#103)ある。書き換えでは根のコミットも変わるので、すべてのコミットのSHAが変わる。

また、公開用のリポジトリは、いまのリポジトリのフォークとして作らない。同じネットワークのリポジトリは、Gitのデータを共有するため([文書](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/working-with-forks/what-happens-to-forks-when-a-repository-is-deleted-or-changes-visibility))。

## 移るものと移らないもの

新しく作ったリポジトリに移せるのは、Gitのデータ(ブランチとタグ)だけである。ほかは次のとおり。

| もの | 扱い |
| --- | --- |
| PR(本文、レビュー、77件の本文のセッションのURLを含む) | 移す手段がない。いまのリポジトリに残る |
| Issue | 同じアカウントのリポジトリの間で、開いているものだけ移せる(`gh issue transfer`)。Privateのリポジトリから公開のリポジトリへは移せないので、新しいリポジトリがPrivateのうちに行う |
| リリースと添付 | 移す手段がない。タグを押し込んだあとで作り直す(`gh release download`と`gh release create --verify-tag`)。公開日は作り直した日になる |
| Actionsの実行の履歴と成果物 | 移らない(J-9) |
| ルールセット | JSONで書き出し、新しいリポジトリで読み込む |
| Dependabotのアラート、ラベル、公開の前後の設定(J-3、J-4、J-5、J-10、J-11、J-13) | 新しいリポジトリで改めて設定する |

## 判断していただくこと

推奨はエージェントの案である。

| 番号 | 判断していただくこと | 推奨 |
| --- | --- | --- |
| M-1 | 置き換えの文字列 | メンテナーのアカウント名にする。名前をメンテナーのものに変えるだけのコミット(差分で名前が出入りする5件のうちの2件)は中身が空になるが、`--prune-empty never`で残す(コミットの数と順番を変えず、記録の対応を取りやすくするため) |
| M-2 | 公開用のリポジトリの名前 | いまのリポジトリの名前を変え、公開用のリポジトリを元の名前(`flying-microwave`)で作る。配布済みのバイナリ(設定画面のリンク)とREADMEのURLが、そのまま公開用のリポジトリを指す。名前を変えたリポジトリへの転送は、同じ名前のリポジトリを作ると働かなくなる([文書](https://docs.github.com/en/repositories/creating-and-managing-repositories/renaming-a-repository)) |
| M-3 | いまのリポジトリの扱い | Privateのまま、アーカイブして残す。PRの本文と議論、Actionsの記録を残すため |
| M-4 | Issue | 開いているもの(5件)を、新しいリポジトリがPrivateのうちに移す。閉じたもの(3件)は移さない |
| M-5 | 過去のリリース | 15件すべてを、元の添付をそのまま上げて作り直す。添付は利用者が受け取ったものと同じにする(ハッシュが変わらない)。添付の中のSBOMには古いSHAが入るが、識別のための文字列で、動作には関わらない |
| M-6 | 文書の中の古いSHA(146件) | 移行の直後の最初のPRで、書き換えの対応表(`commit-map`)を使って、スクリプトで新しいSHAに直す |
| M-7 | 文書の中のPRとIssueの番号 | 新しいリポジトリの番号が重なるかを移行の直後に確かめる。重なるときは、マージの日付の検査(`scripts/test_merge_dates.py`)を旧と新で分ける。ファイルの中の`#NN`は自動でリンクにならない([文書](https://docs.github.com/en/get-started/writing-on-github/working-with-advanced-formatting/autolinked-references-and-urls)) |
| M-8 | 移行の時期 | 正式なリリース`v0.3.0`の前に行う。変更できないリリース(J-13)を、新しいリポジトリで有効にしてから`v0.3.0`を出すため |

## 手順

誰が行うかを、段ごとに示す。

1. 準備(エージェント): git-filter-repoのv2.47.0を、PyPIから取得する。公開から7日以上たっていること(2024-12-04に公開)と、wheelのsha256(`2cd04929b9024e83e65db571cbe36aec65ead0cb5f9ec5abe42158654af5ad83`)を確かめる。実行の前に、メンテナーの承認を得る(インターネットから取得した道具のため)。
2. 新しい複製を作る(エージェント): `main`とタグだけを持つ複製を作る。`--mirror`は使わない(`refs/pull/*`と古いブランチを移さないため)。
3. 書き換える(エージェント): `--replace-text`でアカウント名を置き換え、`--replace-message`でトレーラーの行を消す。式の例は、`regex:(?m)^Claude-Session: https://claude\.ai/code/session_[A-Za-z0-9]+\n?==>`。M-1のとおり`--prune-empty never`を付ける。
4. 確かめる(エージェント): 次のすべてを確かめ、結果をメンテナーに示す。
   - 差分(`git log --all -i -G<アカウント名>`)、コミットメッセージ(`git log --all -i --grep=<アカウント名>`)、タグの注釈(`git for-each-ref refs/tags --format='%(contents)'`)、ブランチとタグの名前に、アカウント名が0件。
   - コミットメッセージに`Claude-Session:`の行が0件。
   - コミットの数が、書き換えの前の`main`と同じ(移行の時点に`git rev-list --count main`で取る)。
   - 差分で名前が出入りするコミットは5件、名前を含むファイルを持つコミットは28件である(2026-11-24の`main`で、`git log -G`と、各コミットへの`git grep -i -F`で数えた)。`commit-map`で、この28件のほかは木(tree)が同じ。移行の時点の`main`で数え直す。
   - 18本のタグが、それぞれ対応するコミットを指している。
   - 書き換えでは、コミットの署名が消える(GitHubの文書の「Lost signatures on commits and tags」)。
5. リポジトリを用意する(メンテナー): いまのリポジトリの名前を変え(M-2)、元の名前で新しいリポジトリをPrivateで作る。フォークにはしない。押し込む前に、新しいリポジトリのActionsを止める(タグを押し込むと、リリースのワークフローが下書きを作るため)。
6. 押し込む(エージェント): `main`とタグを押し込む。そのあとで、Actionsを有効に戻すよう、メンテナーに依頼する。
7. 設定を作り直す(メンテナー): ルールセット2つ(`main`、`release-tags`)を読み込む。必須のチェックの対応のアプリ(GitHub Actions)も確かめる。ラベル`dependencies`、Dependabotのアラート、公開の前の設定(J-10、J-11)を行う。
8. Issueを移す(メンテナー): M-4のとおり。
9. リリースを作り直す(エージェント): M-5のとおり。Prereleaseと「Latest」の指定を、元のとおりにそろえる。
10. 直しのPR(エージェント): M-6とM-7のとおり直す。あわせて、移行を計画とincidentの記録に書く。
11. 公開(メンテナー): 公開範囲をPublicにする。公開のときと直後の設定(J-3、J-4、J-5)を行う。変更できないリリース(J-13)を有効にする。
12. 後始末(メンテナーとエージェント): いまのリポジトリをアーカイブする(M-3)。手元とエージェントの環境の複製を作り直し、古い履歴を混ぜない。Claude Code on the Webの環境の対象のリポジトリも、新しいリポジトリに変える。

## 確かめられなかったこと

- いまのリポジトリのシークレット、変数、Webhook、Environments、Pagesの有無(APIがプロキシに拒否された)。ワークフローは`secrets.`と`vars.`を使っていない。メンテナーに画面で確かめていただく。
- 新しいリポジトリのIssueとPRの番号の付き方(移したIssueの番号を含む)。読んだ文書に書かれていない。
- コミットメッセージの中の`Merge pull request #N`が、新しいリポジトリの画面でリンクになるか。
- GitHub Appのインストールの範囲を移す手順。
