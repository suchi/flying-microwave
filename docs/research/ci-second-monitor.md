# CIで2台目のモニターを用意する方法の調査

- 日付: 2026-11-24
- 関連: [計画0005](../plans/0005-v0.3.0-visuals-and-clock.md)の作業13とE-6(計画0004のE-7から引き継いだもの)、[incident 0014](../incidents/0014-second-monitor-not-drawn.md)(2台目のモニターに描かれないままリリースした)

GitHubのホスト型のWindowsのランナーで、2台目の(仮想の)モニターを用意し、スクリーンセーバーが各モニターに窓を出して描くことを、スモークテストで確かめられるかを調べた。

## 結論

ランナーのモニターは1台(1024x768)である。仮想のモニターを足して確かめられるかは、一次情報では確かめられなかった。エージェントの推奨は、CIでの確認を見送り、これまでどおり実機での確認で見ることである(下の「推奨」)。

## 確かめた事実

### このリポジトリのCI

- CIのWindowsのジョブは`windows-2025`のランナーで動く(`.github/workflows/ci.yml`の102行目)。2026-11-24の`main`の実行のログでは、イメージは`windows-2025-vs2026`、描画のアダプターは`Microsoft Basic Render Driver`(WARP)だった。
- 製品は、起動のときに`monitors: N`と、モニターごとの`monitor i: Area {...}`の行をログに書く(`crates/app/src/saver.rs`の`create_windows`)。スモークテストが失敗して製品のログを出した実行(2026-11-02、11-13、11-14、11-19のもの)には、どれも`monitors: 1`と、幅1024、高さ768のモニターの行があった。通ったときは、スモークテストはこの行を出さない(`crates/smoke/src/windows.rs`の`with_target`は、失敗のときだけ製品の全ログを出す)。
- スモークテストの判定(`crates/smoke/src/lib.rs`の`screensaver_ready`)は、`monitors: N`の数だけ、窓ごとの最初のフレームを待つ。2台目のモニターがあれば、いまの判定のままで2台目の描画も確かめる形になっている。

### GitHubとランナーのイメージの公式の情報

- GitHubの文書([GitHub-hosted runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners))は、Windowsのランナーが管理者の権限で動き、ジョブごとに新しい仮想マシンになることを書いている。画面、解像度、モニターの記述はない。
- ランナーのイメージの公式のリポジトリ(actions/runner-images)のWindows Server 2025のイメージの説明にも、画面やモニターの記述はない。Issue #2935では、既定の解像度が1024x768であることと、`Set-DisplayResolution`で変えられることが示されている。複数のモニターを扱うIssueは見つからなかった。
- 仮想のモニターを足すには、Indirect Displayのドライバー(ユーザーモードのドライバー。[Microsoftの文書](https://learn.microsoft.com/en-us/windows-hardware/drivers/display/indirect-display-driver-model-overview))が要る。Windowsの標準の機能で、ドライバーなしにモニターを足す方法は見つからなかった。

## 候補

| 候補 | 内容 | 見込みと危険 |
| --- | --- | --- |
| A | VirtualDrivers/Virtual-Display-Driver(MIT。保守は活発) | ランナーで動いた記録がない。ヘッドレスでの導入の不具合がIssueにある。導入には、第三者の署名の証明書を、マシン全体の信頼された発行元に入れる。取得元が2つ増える |
| B | MolotovCherry/virtual-display-rs | 保守が止まっている。自己署名の証明書を信頼されたルートに入れる。採らない |
| C | Microsoftのサンプルのフォーク(ge9/IddSampleDriver) | 保守が止まっている。採らない |
| D | Parsec VDD | ドライバー本体を確かめられなかった。対話のセッションが要るという報告がある(手掛かり) |
| E | Microsoftのサンプル(Windows-driver-samplesの`video/IndirectDisplay`)をCIでビルドして署名する | 第三者のバイナリは入れずに済む。WDKがイメージにない。ドライバーのコードを自分たちで持つことになり、範囲が大きい。ランナーで動くかは分からない |

第三者のプロジェクトが、ホスト型のランナーでAとDを試して取りやめた例がある(コミットの記述で、一次情報ではない)。

## 分からなかったこと

- ランナーの表示のアダプター(ディスプレイのドライバー)の種類。
- Windows Server 2025のランナーで、Indirect Displayのドライバーが読み込まれ、2台目のモニターとしてデスクトップに加わるか。
- ジョブの途中で再起動できるか、Secure Bootが有効か。
- 仮想のモニターに、WARPで表示できるか。
- Chromiumのテスト用の仮想ディスプレイの説明は、`chromium.googlesource.com`に接続できず、読めなかった。

## 推奨

エージェントの推奨は、CIでの確認を見送ることである。理由は、ランナーで動いた記録が一次情報になく、試すにはCIの実行を何度も重ねることになり、いちばん見込みのある候補(A)でも、第三者の証明書を信頼に入れて取得元が増えるためである。

ほかの方法(メンテナーの判断):

1. 見送り(推奨): これまでどおり、複数のモニターは実機での確認で見る。ログの`window N: first frame presented`の行がモニターの数だけあるかでも判定する(incident 0014の対策)。
2. テストのときだけ、1つのモニターを2つの領域に分けて窓を2つ出す仕組みを製品に加える。2つの窓を描く経路はCIで通るが、本物の2台目のモニターは確かめない。製品のコードとアーキテクチャに関わる。
3. 手動実行の別のジョブで、Aを1回だけ試す。必須の検査にはしない。ワークフロー(セキュリティ設定)の変更と、証明書を信頼に入れることの承認が要る。
