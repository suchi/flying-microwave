# v0.3.0-alpha.5の実機での確認の手順

- 作成日: 2026-11-23
- 対象: 試作版`v0.3.0-alpha.5`(タグはPR #99のマージのコミット`ccb6c57`に、2026-11-23 15:45(JST)に付いた)。[計画0005](../plans/0005-v0.3.0-visuals-and-clock.md)の作業10の6の確認(D-25の(3)、D-38)で、`alpha.4`の実機での確認で確かめられなかった項目(CPUの割合、60Hzの画面での動きの滑らかさ、PCに直接つないだモニターでの見た目、時計の12時間の形式、食べ物の数0)と、終了の速さの直し(D-34、[ADR-0029](../adr/0029-leave-gpu-resources-to-process-exit.md))を確かめる。もとにしたのは[`alpha.4`の手順書](hardware-check-v0.3.0-alpha.4.md)である。
- 確かめない項目: `alpha.4`の確認で問題がなかった項目(3Dの見た目と動きの値、飛び方の選択、24時間の形式の時計、左の側面・上面・底面の絵、食べ物の種類と大きさ)。計画0001の手動テストの項目30のうち、新しい版で書かれた設定ファイルで警告を出した状態の確認(`alpha.4`と同じく、この版では頼まない)。

手順の記号(P-1、A-1など)は本書の中だけのもので、ほかの手順書の記号とは別である。エージェントはWindowsで実行できないため、G-2のコマンドと、PowerShellのC-3とG-5のコマンドは未確認である。

以下は、メンテナーに渡す手順と同じ内容である。

対象: GitHub Releaseのプレリリース`v0.3.0-alpha.5`の`FlyingMicrowave-0.3.0-alpha.5-windows-x64.zip`。`alpha.4`の確認で、リモートデスクトップのために確かめられなかった項目と、終了の速さの直しを確かめるための確認です。リモートデスクトップではなく、PCの前で、PCに直接つないだモニターで行ってください(計画0005のD-25の(3))。結果は、番号ごとに「OK」「NG(見えたこと)」「できなかった」で返してください。最後に、ログと記録のファイル(手順Z)も送ってください。

## 準備

- P-1. GitHubのリリースの画面で、`v0.3.0-alpha.5`を開く(確認を先送りして、2026-11-23 21:04(JST)にプレリリースとして公開した。計画0005のD-39の(1))。
- P-2. リリースからzipをダウンロードし、ハッシュを確かめる。coreutilsの入ったcmdで、ダウンロードしたフォルダで次を実行する。

  ```bash
  sha256sum FlyingMicrowave-0.3.0-alpha.5-windows-x64.zip
  ```

  表示の先頭の64桁が`091ee74c7e46d3a470add797fd7209a8989965be522f6523aded9d1a669d4d59`であること(値は、リリースのワークフローのReleaseの18回目の、下書きを作るジョブ(Create draft release)のログの`sha256sum`の出力の行。リリースの`SHA256SUMS.txt`にも同じ値がある)。一致しないときは、先に進まずに知らせてください。
- P-3. エクスプローラーの「すべて展開」で、前の版とは別の新しいフォルダに展開し、`FlyingMicrowave.scr`を右クリックして「インストール」を選ぶ(Windows 11では「その他のオプションを確認」の中)。スクリーンセーバーの設定の画面で「FlyingMicrowave」が選ばれていること。
- P-4. 設定画面(設定の画面の[設定])で「Defaults」を押してから「OK」を押す(既定の設定に戻す)。
- P-5. 前のログと記録を消す(cmdで実行する)。

  ```bash
  del %TEMP%\flying-microwave.log %TEMP%\fm-usage.txt
  ```

## A. PCに直接つないだモニターでの見た目(E-1、G-3、D-12)

設定の画面の「プレビュー」ボタンで全画面にし、1〜2分見てください。

- A-1. 回転による明るさの変化(G-3): 電子レンジが回転して面の明るさが変わるとき、点滅のようにちらついて見えないこと。
- A-2. 縁: 電子レンジの縁のギザギザが、気になるかどうか(気になるなら、なめらかにする処理を加えることを検討する)。遠くで斜めを向いたドアの縁が、欠けて見えないこと。

## B. 時計の12時間の形式と、食べ物の数0(手動テストの項目31、33)

- B-1. 10:00〜12:59以外の時刻に行う(この時間は、12時間の形式でも24時間の形式と同じ数字になるため)。設定画面の Display のタブで Clock format を 12-hour にして OK を押し、全画面にする。電子レンジの正面のLEDの時刻で、十の位の0が出ない(例「 9:05」)。午後に確かめた場合は、左下の点が光る。確かめた時刻を書いてください。確かめたら、24-hour に戻して OK を押す。
- B-2. 設定画面の Flight のタブで Number of food items を0にして OK を押し、全画面にする。電子レンジだけが飛び、食べ物は飛ばない。確かめたら、4に戻して OK を押す。

## C. 終了の速さ(N-02、ADR-0029)と動きの滑らかさ(D-20)

- C-1. 全画面で、キーを押す。画面が、もたつかずに一瞬で消えること。何回か試し、一度でも0.5秒ほど残ったら、そのことを書く。
- C-2. 動きの滑らかさ: リフレッシュレートが60Hzのモニターで、全画面で1分ほど見て、電子レンジと食べ物が、ときどき一瞬止まったり、引っかかったりして見えないこと。止まって見えたら、どれくらいの頻度か(1秒に何回、数秒に1回など)を書く。60Hzでないモニターで見た場合は、そのリフレッシュレートを書いてください。
- C-3. プロセスが終わるまでの時間: 画面が消えたあとにプロセスが残る時間は、目でもログでも見えないため(ログの最後の行は`screensaver ended`で、そのあとの、OSが資源を回収する時間は書かれない。ADR-0029)、PowerShellで測る。PowerShell(cmdではない)で、展開したフォルダに移動してから、次の1行を実行する。全画面になったら、5秒ほど待ってからキーを押して終える。PowerShellに出る数字(プロセスが動いていたミリ秒)を書いてください。

  ```powershell
  $p = Start-Process -FilePath .\FlyingMicrowave.scr -ArgumentList '/s' -PassThru; $p.WaitForExit(); ($p.ExitTime - $p.StartTime).TotalMilliseconds
  ```

  エージェントは、この値から、ログのそのプロセスの`exit:`の行の時刻(`+Nms`。ログの時刻は、そのプロセスの最初のログの行からの経過。`crates/app/src/log.rs`の`write`)を引いて、キーで終わりを決めてからプロセスが終わるまでの、おおよその時間を求める。

## G. 負荷とメモリ(N-03、N-04)

全画面はキーやマウスの操作で終わるため、タスクマネージャーに切り替えて値を読むことはできません。cmdから全画面を起動し、1分後と5分後に、使ったCPU時間とメモリを記録します。

- G-1. 設定画面で「Defaults」を押してから「OK」を押す(既定の設定に戻す)。
- G-2. cmdで、展開したフォルダに移動してから、次の1行を実行する。全画面になったら、終わるまで(約5分。cmdに戻るまで)キーボードとマウスに触らないでください。

  ```bash
  start "" FlyingMicrowave.scr /s & timeout /t 60 /nobreak >nul & tasklist /v /fo list /fi "imagename eq FlyingMicrowave.scr" > %TEMP%\fm-usage.txt & timeout /t 240 /nobreak >nul & tasklist /v /fo list /fi "imagename eq FlyingMicrowave.scr" >> %TEMP%\fm-usage.txt
  ```

- G-3. 約5分たってcmdのプロンプトが戻ったら、キーを押して全画面を終える。`%TEMP%\fm-usage.txt`に、2回分の「CPU時間」と「メモリ使用量」があること。あわせて、モニターの数・解像度・リフレッシュレートと、GPUの名前を書いてください。CPUの割合(論理1コアあたり)は、2回の「CPU時間」の差を240秒で割って、エージェントが求めます。目安は、論理1コアの10%以下(N-03。4Kモニター1台、60fps)、メモリが200MB以下で増え続けない(N-04)。
- G-4. 記録が空のときや、「情報: 指定された条件に一致するタスクは実行されていません」のようなメッセージのときは、そのまま書いてください。`alpha.4`では、記録の「CPU 時間」が3回すべて`0:00:00`で、CPUの割合を求められなかった(理由は分かっていない。[`alpha.4`の手順書](hardware-check-v0.3.0-alpha.4.md)の「結果」の節)。同じになったときは、そのことも書いて、G-5も行ってください。
- G-5. G-2の2回の「CPU 時間」が同じ値だったときだけ行う。PowerShell(cmdではない)で、展開したフォルダに移動してから、次の1行を実行する。全画面になったら、約5分(PowerShellに数字が出るまで)キーボードとマウスに触らず、数字が出たらキーを押して全画面を終える。出た3つの数字(1分後のCPU時間の秒、5分後のCPU時間の秒、5分後のメモリのバイト数)を書いてください。

  ```powershell
  $p = Start-Process -FilePath .\FlyingMicrowave.scr -ArgumentList '/s' -PassThru; Start-Sleep 60; $p.Refresh(); $a = $p.TotalProcessorTime.TotalSeconds; Start-Sleep 240; $p.Refresh(); "$a $($p.TotalProcessorTime.TotalSeconds) $($p.WorkingSet64)"
  ```

## Z. 終わったあとと、送るもの

- Z-1. 設定画面で「Defaults」を押してから「OK」を押す(既定の設定に戻す)。
- Z-2. 次の2つのファイルを送ってください: `%TEMP%\flying-microwave.log`、`%TEMP%\fm-usage.txt`。
- Z-3. エージェントは、ログで次を判定する。
  - 終了の速さ(ADR-0029): 全画面の終了ごとに、終了を決めた行(`exit: ...`。`crates/app/src/saver.rs`の`decide`)、窓を隠した時間の行(`windows hidden in ... ms`。同じ`decide`)、後始末の行(`exit cleanup started: ...`と`exit cleanup: ...; windows, surfaces, GPU and instance left to the process exit`。`saver.rs`の`exiting`)、終わりの行(`screensaver ended: ...`。`saver.rs`の`run`)があり、`exit:`の行から`screensaver ended`の行までが短いこと(`alpha.4`の実機では34〜53ms)。ログで分かるのは`screensaver ended`の行までで、そのあとプロセスが終わるまで(OSが資源を回収する時間を含む)は、C-3の値で判定する。設定の画面のプレビュー枠の終了では、`preview exit: ...`、`preview cleanup started: ...`、`preview cleanup: ...; surface, GPU and window left to the process exit`、`preview ended: ...`の行(`crates/app/src/preview.rs`の`finish`、`exiting`、`run`)。
  - 画像の取得の遅れ: `window N: surface texture took ... ms`か`preview: surface texture took ... ms`の行(`saver.rs`の`try_draw`、`preview.rs`の`redraw`。100msより長くかかったときだけ、窓ごとに最初の5回まで書く)があるか。
  - 起動の速さ(N-01): 起動の行(`Flying Microwave 0.3.0-alpha.5 started`。`crates/app/src/main.rs`の`run`)から、ウィンドウごとの`first frame presented`の行(`saver.rs`の`try_draw`)までが1秒以内。
  - 設定の反映: 全画面の起動ごとの`settings: Config { ... }`の行(`saver.rs`の`settings: {config:?}`)で、`clock: Clock { enabled: true, format: H12 }`と`food_count: 0`が、手順のとおりに変わっていること(`alpha.4`では、C-3とE-5を行わなかったため、ログに出ていなかった)。
- Z-4. 公開は済んでいる(D-39の(1))。確認の結果は、エージェントがCHANGELOGのこの版の節に反映し、リリースノートの差し替えをお願いする(計画0001の7.3の手順5)。
