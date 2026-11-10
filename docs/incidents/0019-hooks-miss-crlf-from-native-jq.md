# 0019: Windowsのネイティブのjqでは、設定の保護のフックが確認を求めないおそれがある

- 日付: 2026-11-10
- ステータス: 対策中(対策はPR #62とPR #63でマージした。メンテナーのWindowsでの確認を待っている)
- 関連: [0009](0009-guard-hooks-never-ran.md)、[0012](0012-hook-check-tests-failed-on-windows.md)、[0018](0018-english-progress-note-again.md)、`.claude/hooks/`

## 起きたこと

2026-11-10に、incident 0018の対策のフック(`require-japanese-note.sh`)の再レビューで、レビューのエージェントが次の点を指摘した。WindowsのGit Bashからネイティブのjq.exeを使うと、jqの出力の行末がCRLFになり、既存のフックの判定が外れる。エージェントが一次情報と再現で確かめた。

確かめた事実:

- jqの公式マニュアル(jq 1.7.1の`docs/content/manual/v1.7/manual.yml`の`--binary`の項)には、次のように書かれている。「Windows users using WSL, MSYS2, or Cygwin, should use this option when using a native jq.exe, otherwise jq will turn newlines (LFs) into carriage-return-then-newline (CRLF).」Git BashはMSYS2を土台にしている。
- オンボーディングは、Windowsでのjqの導入の例として、`winget install jqlang.jq`と`scoop install jq`を挙げている。どちらもネイティブのjq.exeを入れる(推測。パッケージの中身は確かめていない)。
- フックは、`jq -r`で取り出した値を、シェルの変数に入れて比べている。コマンド置換(`$(...)`)は末尾のLFだけを取り除くため、CRは値の末尾に残る。
- Linuxで、出力の各行にCRを付け、jqの終了コードは保つ仮のjqを`PATH`の先頭に置いて、各フックを登録したとおりの起動で試した(2026-11-10)。
  - `protect-config.sh`: `.claude/settings.json`と`.github/dependabot.yml`の編集で、確認(`ask`)を出さなかった。通常のjqでは出す。パターンが`*/.claude/settings.json`のように末尾で終わる形で、CRのある値に一致しないため。`.claude/hooks/`の下のファイルは、パターンの末尾の`*`がCRも含むため、確認を出した。
  - `require-japanese-reply.sh`: `stop_hook_active`の値が`true`と一致せず、続けて書き直しを求める回数の上限(2回)が効かなかった。4回続けて求めた(通常のjqでは3回目から通す)。
  - `block-dangerous-git.sh`と`require-japanese-description.sh`: CRがあっても、サンプルでは通常のjqと同じ判定だった。ただし、`require-japanese-description.sh`は、説明文が空文字列のときに誤って拒否した(対策のレビューで見つかった)。
  - `require-japanese-note.sh`(0018の対策): 取り出した値からCRを取り除くようにしてあり、同じ判定だった。
- 2026-11-10に、メンテナーのWindowsのMSYS2のシェル(UCRT64)で`jq -r .a <<< '{"a":"x"}' | od -c`を実行すると、出力は`x  \r  \n`だった。メンテナーの環境では、jqの出力がCRLFになる。Claude Codeがフックを起動するGit Bashも、MSYS2を土台にしているため、同じ結果になると考えられる(推測。Git Bashでは試していない)。フックそのものをWindowsの実機で起動しては、まだ確かめていない。
- `scripts/check_hooks.py`は、Windowsでは範囲外として何もしない(0012)。CIのWindowsのジョブでも、フックの動作は確かめていない。

## なぜなぜ分析

| なぜ | 答え(確かめた事実) |
| --- | --- |
| なぜ設定の保護のフックが確認を求めないおそれがあるか | ネイティブのjq.exeをGit Bashから使うと出力がCRLFになり、取り出したパスの末尾にCRが残って、パターンに一致しないため |
| なぜCRを考えていなかったか | フックを作るときに、Windowsでフックが動く環境(Git Bashと、そこから呼ぶネイティブのjq.exe)の振る舞いを、一次情報で洗い出さなかったため。0012では、`sh -c`での起動と、Git Bashがないときの扱いだけを確かめた |
| なぜ検査で見つからなかったか | フックの動作の検査は、LinuxとmacOSの起動だけを再現し、Windowsは範囲外とした(0012)。Linuxの検査では、jqの出力はLFだけになる |

## 真因

フックが前提にしている実行環境の違い(Windowsでの、Git Bashとネイティブのjq.exeの組み合わせ)を洗い出さず、Linuxでだけ確かめていた。Windowsで確かめる手段がないまま、その差をLinuxの検査で模擬することもしていなかった。

## 対策

2026-11-10にメンテナーが承認した(`.claude/hooks/`と`scripts/check_hooks.py`の変更。前者はセキュリティ設定)。

1. 現象への対応: フックが`jq -r`で取り出して比べる値や、空かどうかを確かめる値から、jqの終了コードを確かめたあと、別の行でCRを取り除いた(`tr -d '\r'`)。jq.exeはすべてのLFをCRLFにするため、すべてのCRを取り除く。正規表現で調べるだけの値(`require-japanese-note.sh`の本文)は、CRに左右されないため変えていない。`require-japanese-reply.sh`のセッションのキーは、以前から使える文字だけを残している。CRを取り除くと、パターンへの一致は増える方向にだけ変わる(bashはCRで語を区切らないため)。例外は、`block-dangerous-git.sh`の`.env.example`などを含むときの除外で、これはCRがなくても同じ形で外れる。`require-japanese-description.sh`と`require-japanese-reply.sh`では、値が空文字列のときに値がCRだけになり、誤って書き直しを求めていたことも、これで直した(レビューで見つかった)。`require-japanese-note.sh`は、0018の対策(PR #62)ですでにCRを取り除いていたが、ほかのフックと同じ形にそろえた。`protect-config.sh`の、jqがないときや入力を解析できないときに確認を求める動作は変えていない。
2. 仕組み: `scripts/check_hooks.py`で、各サンプルを、出力の行末をCRLFにする仮のjq(jqの終了コードは保つ)でも実行し、期待する判断になることを確かめるようにした。Windowsの実機がなくても、Linuxの検査とCIで、同じ種類の欠けを見つけられる。macOSでの動作は未確認(CIにmacOSのジョブはない)。1の前のフックでは、この検査が`protect-config.sh`と`require-japanese-reply.sh`で失敗し、1のあとは通ることを確かめた(2026-11-10)。
3. 確認: メンテナーのWindowsで、jqの出力に`\r`があることを確かめた(2026-11-10。上の「確かめた事実」)。対策のあとに、メンテナーの環境で、`.claude/settings.json`の編集で確認を求めることを確かめる。

## 確かめ方

- 2の検査が、対策の前のフックで失敗し、対策のあとのフックで通ること(2026-11-10に確かめた)。CIのLinuxのジョブで、毎回確かめる。
- 対策のあと、メンテナーのWindowsで、`protect-config.sh`が`.claude/settings.json`の編集で確認を求めること。
