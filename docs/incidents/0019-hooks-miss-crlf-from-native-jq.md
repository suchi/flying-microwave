# 0019: Windowsのネイティブのjqでは、設定の保護のフックが確認を求めないおそれがある

- 日付: 2026-11-10
- ステータス: 対策の判断待ち(セキュリティ設定の変更のため、メンテナーの承認が要る)
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
  - `block-dangerous-git.sh`と`require-japanese-description.sh`: CRがあっても、通常のjqと同じ判定だった。
  - `require-japanese-note.sh`(0018の対策): 取り出した値からCRを取り除くようにしてあり、同じ判定だった。
- Windowsの実機では試していない。メンテナーの環境のjqがネイティブのjq.exeかどうか、出力が実際にCRLFになるかは、確かめていない。
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

案を示し、メンテナーの承認を待っている(`.claude/hooks/`と`scripts/check_hooks.py`の変更。前者はセキュリティ設定)。

1. 現象への対応: すべてのフックで、`jq -r`で取り出した値からCRを取り除く(`tr -d '\r'`)。
2. 仕組み: `scripts/check_hooks.py`で、各サンプルを、出力にCRを付ける仮のjqでも実行し、通常のjqと同じ判定になることを確かめる。Windowsの実機がなくても、LinuxのCIで同じ種類の欠けを見つけられる。
3. 確認: メンテナーのWindowsで、Git Bashから`jq -r .a <<< '{"a":"x"}' | od -c`を実行し、出力に`\r`があるかを確かめる(任意。結果は本記録に書く)。

## 確かめ方

- 2の検査が、対策の前のフックで失敗し、対策のあとのフックで通ること。
- 3の結果が分かれば、Windowsでの前提を記録する。
