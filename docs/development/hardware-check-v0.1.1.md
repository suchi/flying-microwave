# v0.1.1の実機での確認の手順

- 作成日: 2026-11-03
- 対象: `v0.1.1-alpha.3`(コードはv0.1.1と同じ)。メンテナーは2026-11-03にこの手順で確認し、全体として問題がなかったと報告した。項目ごとの結果は[Issue #47](https://github.com/suchi/flying-microwave/issues/47)で受け取る([計画0001](../plans/0001-initial-implementation-plan.md)のD-33)。
- 関連: [計画0001](../plans/0001-initial-implementation-plan.md)の8.2(手動テスト)・N-01〜N-04、[計画0003](../plans/0003-v0.1.1-smoke-test.md)のD-7、[スパイクの結果](../research/spike-results.md)

手順の記号(P-1、A-5、B-6など)は本書の中だけのもので、アーキテクチャの決まり(A-1〜A-8)や計画の決定とは別である。記号と、計画の項目の対応は次のとおり。

| 本書の手順 | 計画の項目 |
| --- | --- |
| P-1〜P-3 | 8.2の9(ダウンロードしたzip)、計画0001のD-31(エクスプローラーでの展開) |
| A | 8.2の1・18・19、S5 |
| B | 8.2の2・3・4・17、計画0003のD-7、N-02 |
| C | 8.2の5・7、計画0003のD-7 |
| D | 8.2の8、S3の残り |
| E | 8.2の6、S4の残り |
| F | 8.2の11、S5 |
| G | 8.2の12(SEC-1) |
| H | 8.2の16・17、N-03・N-04 |
| I | 8.2の13 |
| J | 8.2の14 |
| K | 8.2の10、S6 |
| L | 計画0001のD-5(商標の確認) |
| Z | N-01、計画0003のD-7(ログ) |

本書は、メンテナーに渡した手順書と同じ内容である。手順の中の「添付の`measure.ps1`」と「添付の`flying-microwave-s6.wsb`」の中身は、末尾の付録に載せた。

対象: GitHub Releaseの下書き`v0.1.1-alpha.3`の`FlyingMicrowave-0.1.1-alpha.3-windows-x64.zip`(計画0001のv0.1.1の完了条件、手動テスト8.2、計画0003のD-7)。

結果は、項目の番号ごとに「OK」「NG(見えたこと)」「できなかった(理由)」で返してください。最後に、ログの全文を送ってください(手順Z)。数値(N-01、N-02の終了まで)は、エージェントがログから読みます。

所要時間の目安: Sandboxを除いて1時間ほど、Sandboxが30分ほど。分けて行ってかまいません。

## 見えるもの(成功の判定のもと)

- 全画面: 濃い紺色(ほぼ黒)の背景の上を、四角を組み合わせた仮の電子レンジが何個か動いている。電子レンジは、灰色の箱に、暗い色の窓、灰色の操作パネル、黄色のつまみ。止まった絵ではなく、動き続けていること。
- 前面化のログ: PowerShellで`Get-Content $env:TEMP\flying-microwave.log -Tail 30`を実行し、その回の行に`SetForegroundWindow: true`があること。`false`ならNG。

## 準備

- P-1. ダウンロード: ブラウザで、リポジトリのReleasesの画面から、下書き`v0.1.1-alpha.3`の`FlyingMicrowave-0.1.1-alpha.3-windows-x64.zip`をダウンロードする(手動テスト9のため、ブラウザで取得する)。
- P-2. チェックサム: PowerShellで次を実行し、出力のHashが`D28D0907DEAC45A9563F7CF30BBB0BFDBC585F56B68FED21F647AFFCFE7B2F1D`であること。

  ```powershell
  Get-FileHash "$env:USERPROFILE\Downloads\FlyingMicrowave-0.1.1-alpha.3-windows-x64.zip" -Algorithm SHA256
  ```

- P-3. エクスプローラーでの展開(D-31): zipを右クリックし、「すべて展開」で、`%LOCALAPPDATA%\Programs\FlyingMicrowave`(新しいフォルダ)に展開する。成功: エラーなく、`FlyingMicrowave.scr`、`README.md`、`LICENSE.txt`、`THIRD-PARTY-LICENSES.txt`、`FlyingMicrowave-0.1.1-alpha.3-windows-x64.spdx.json`の5つができる。
- P-4. 前のログを消す(この回のログだけにするため):

  ```powershell
  Remove-Item $env:TEMP\flying-microwave.log -ErrorAction SilentlyContinue
  ```

## A. ファイルとインストール(手動テスト19・18・1、S5)

- A-1. アイコン(19): エクスプローラーの表示を「大アイコン」にする。成功: `FlyingMicrowave.scr`が、濃い青の角丸の四角に白い電子レンジの絵のアイコンで表示される。
- A-2. バージョン情報(19): `FlyingMicrowave.scr`を右クリック、「プロパティ」の「詳細」タブ。成功: 製品名`Flying Microwave`、製品バージョン`0.1.1-alpha.3`、ファイル バージョン`0.1.1.0`。
- A-3. インストール(18、S5): 利用者向けの`README.md`の「インストール」の手順3のとおり、右クリックで「インストール」を選ぶ(Windows 11では「その他のオプションを確認」の中)。成功: スクリーンセーバーの設定の画面が開き、一覧で「FlyingMicrowave」が選ばれている。READMEの手順と画面が違ったら、違いを書く。
- A-4. 一覧の名前(1、S5): 設定の画面のドロップダウンに「FlyingMicrowave」と表示される。
- A-5. 別のスクリーンセーバーを選んだあと(S5): 一覧で別のもの(「(なし)」以外)を選んで「適用」し、ドロップダウンを開く。成功の判定ではなく記録: 「FlyingMicrowave」が一覧に残っているか。そのあと、FlyingMicrowaveを選び直して「適用」する。

## B. 設定の画面からの確認(手動テスト2・3・4・17、D-7)

- B-1. プレビュー枠(2): 設定の画面の小さなモニタの絵の中に、「見えるもの」の全画面と同じ絵が小さく動いている。
- B-2. プレビューのプロセスの終了(2): タスクマネージャーの「詳細」タブを開き、`FlyingMicrowave.scr`があることを確かめてから、設定の画面で別のスクリーンセーバーを選ぶ。成功: 1秒ほどで`FlyingMicrowave.scr`が一覧から消える。確かめたら、FlyingMicrowaveを選び直す。
- B-3. 「設定」ボタン(3): 成功: 設定は今後の版で追加するという内容のメッセージボックスが、設定の画面の手前に表示される。OKで閉じる。
- B-4. 「プレビュー」ボタンをマウスで押す(4): 成功: 全画面が「見えるもの」のとおりになり、5秒待っても勝手に終わらない。その状態で、次の終わらせ方を1回ずつ試す(1回ごとに「プレビュー」を押し直す)。
  - キーを押す → 終わる
  - マウスを大きく動かす → 終わる
  - マウスをクリックする → 終わる
  - マウスを机の上でわずかに揺らす(数ピクセル) → 終わらない(このあとキーで終わらせる)
  - タッチ(タッチ画面がある場合) → 終わる
- B-5. 「プレビュー」ボタンをキーボードで押す(4): Tabキーで「プレビュー」ボタンに移り、Enterキーで押す。成功: 全画面になり、Enterを離したことで勝手に終わらない。キーを押して終わらせる。
- B-6. 前面化(4、D-7): ログの末尾を表示する。成功: B-4・B-5の回の行に`SetForegroundWindow: true`がある。
- B-7. 終了の速さ(N-02): B-4で、キーを押してから画面が消えるまでに、もたつきを感じないこと(一瞬で消える)。数値(0.2秒以内)は実機では測れないため、CIの値(10ミリ秒)と合わせて記録する。

## C. 待ち時間での起動とロック(手動テスト5・7)

- C-1. 待ち時間で起動(5、D-7): 待ち時間を1分にして「適用」し、マウスとキーボードに触れずに待つ。成功: 1分ほどで全画面が「見えるもの」のとおりになる。キーを押して終わらせる。ログの末尾に、この回の`SetForegroundWindow: true`がある。
- C-2. ロック(7): 「再開時にログオン画面に戻る」をオンにしてC-1と同じことを行う。成功: キーで終わらせたあと、ロック画面が表示される。確かめたら、元の設定に戻す。

## D. 右クリックとダブルクリック(手動テスト8)

- D-1. ダブルクリック: `FlyingMicrowave.scr`をダブルクリック。成功: 全画面になり、キーで終わる。
- D-2. 右クリックの「テスト」: 成功: 全画面になり、キーで終わる。
- D-3. 右クリックの「構成」: 成功: B-3と同じメッセージボックスが出る。
- D-4. 右クリックの「インストール」: A-3と同じ。

## E. 複数モニタ(手動テスト6、S4。環境がある場合)

- E-1. 2台以上のモニタで「プレビュー」を押す。成功: すべてのモニタが「見えるもの」で覆われ、5秒待っても勝手に終わらない。拡大率(ディスプレイの設定の「拡大/縮小」)がモニタで違う組み合わせなら、それも記録する(拡大率を変えて試す必要はない)。

## F. パスの名前(手動テスト11、S5)

- F-1. 空白と日本語を含むフォルダ: `C:\テスト 用\`を作り、`FlyingMicrowave.scr`を写し、右クリックの「テスト」。成功: 全画面になり、キーで終わる。
- F-2. 日本語を含むユーザー名: そのようなアカウントがあれば試す。なければ「できなかった」でよい。
- F-1のフォルダは、確かめたら削除する。

## G. DLLの差し替えが効かないこと(手動テスト12、SEC-1)

Process Monitor(Microsoft Sysinternals)を使う。入っていなければ、Microsoftのサイトから入手する。

- G-1. `C:\FMDllTest\`を作り、`FlyingMicrowave.scr`を写す。同じフォルダに、中身が空のファイルを3つ作る。

  ```powershell
  New-Item -ItemType File C:\FMDllTest\D3d10warp.dll, C:\FMDllTest\d3d12.dll, C:\FMDllTest\dxgi.dll
  ```

- G-2. Process Monitorを起動し、フィルタに「Path」「begins with」「C:\FMDllTest\」「Include」を加える。
- G-3. エクスプローラーで`C:\FMDllTest\FlyingMicrowave.scr`を右クリックし、「テスト」を選ぶ。全画面になったら、キーで終わらせる。
- G-4. 成功: Process Monitorの一覧に、`C:\FMDllTest\D3d10warp.dll`、`C:\FMDllTest\d3d12.dll`、`C:\FMDllTest\dxgi.dll`のパスの行が1行もない(`FlyingMicrowave.scr`自体の行はあってよい)。全画面が正しく動いたことだけでは判定しない。
- G-5. `C:\FMDllTest\`を削除する。

## H. 負荷とメモリ(手動テスト16・17、N-03・N-04)

- H-1. 添付の`measure.ps1`を、デスクトップなどに置く。計測の間に画面が消えたりスリープしたりしないよう、電源の設定で「画面の電源を切る」と「スリープ」を15分以上にし、スクリーンセーバーの待ち時間も15分以上にする。
- H-2. PowerShellで実行する。

  ```powershell
  powershell -ExecutionPolicy Bypass -File "$env:USERPROFILE\Desktop\measure.ps1" -Scr "$env:LOCALAPPDATA\Programs\FlyingMicrowave\FlyingMicrowave.scr"
  ```

- H-3. 全画面になったら、マウスとキーボードに触れずに約10分待つ。音が2回鳴ったら、キーを押して終わらせる。
- H-4. 成功の判定(エージェントが行う): デスクトップの`flying-microwave-measure.csv`と、PowerShellの画面の出力(画面の解像度とリフレッシュレートの表、最後の「FlyingMicrowave のプロセスは残っていない」)を送る。N-03(4K・60Hzで論理1コアの10%以下)とN-04(200MB以下、増え続けない)をエージェントが判定する。
- このスクリプトは、エージェントが実行して確かめていない(Windowsを実行できないため)。エラーになったら、そのメッセージを送ってほしい。
- H-5. H-1で変えた電源とスクリーンセーバーの設定を元に戻す。

## I. スリープと消灯からの復帰(手動テスト13)

- I-1. 「プレビュー」で全画面にし、スタートメニューを使わずにスリープさせる(PCの電源ボタンが「スリープ」の設定の場合はそれを押す。ノートPCならふたを閉じる)。復帰させる。成功: 復帰のあと、全画面がまだ動いているか、すでに終わっているかのどちらか。画面が固まったまま操作できない状態にならない。どちらだったかを書く。
- I-2. ディスプレイの消灯(画面の電源を切るまでの時間を1分にする)で同じことを行う。確かめたら、設定を元に戻す。

## J. Smart App Control(手動テスト14)

- J-1. 「Windows セキュリティ」の「アプリとブラウザー コントロール」の「Smart App Control」の状態(オン、評価、オフ)を書く。オンなら、A-3やD-2で警告やブロックが出たかを書く。

## K. Windows Sandbox(S6、手動テスト10)

GPUのない環境(WARP)と、VC++ランタイムのない環境を確かめる。Windows Sandboxは、Windows 11 Pro・Enterprise・Educationで使える。Homeでは使えないので、その場合は「できなかった」でよい。

- K-1. 「Windows の機能の有効化または無効化」で「Windows サンドボックス」をオンにする(再起動が要る場合がある)。
- K-2. `C:\FlyingMicrowaveTest\`を作り、`FlyingMicrowave.scr`を写す。
- K-3. 添付の`flying-microwave-s6.wsb`(GPUとネットワークを無効にし、`C:\FlyingMicrowaveTest\`を読み取り専用で`C:\FlyingMicrowave\`に見せる設定)をダブルクリックし、Sandboxを起動する。
- K-4. Sandboxの中のエクスプローラーで`C:\FlyingMicrowave\FlyingMicrowave.scr`を右クリックし、「テスト」。成功: 全画面が「見えるもの」のとおりに動く(最初の絵が出るまで数秒かかってもよい)。キーで終わる。
- K-5. Sandboxの中のPowerShellで`Get-Content $env:TEMP\flying-microwave.log`を実行し、出力を選んでコピーし、Sandboxの外に貼って送る。成功の判定(エージェントが行う): `Microsoft Basic Render Driver`の行があり、`first frame presented`の行がある。
- K-6. Sandboxを閉じる(中身は消える)。`C:\FlyingMicrowaveTest\`を削除する。

## L. 商標の確認(Phase 0の作業1、D-5)

- L-1. 通常のブラウザで、次を確かめる。結果(件数、似た名前の登録があればその名前・区分・状態)を書く。
  - 日本(J-PlatPat): 第9類(コンピュータプログラム)を中心に、文字「FLYING MICROWAVE」「FLYINGMICROWAVE」「フライングマイクロウェーブ」。
  - 米国(USPTO): 「FLYING MICROWAVE」(前回の自動の検索で0件)の再確認と、「FLYINGMICROWAVE」。

## Z. ログを送る

- Z-1. すべての確認のあとで、次を実行し、出力を送る。

  ```powershell
  Get-Content $env:TEMP\flying-microwave.log
  ```

- ログには、実行ファイルのパス(ユーザー名を含むことがある)、GPUの名前とドライバの版、モニタの位置と大きさが含まれる。文書などの利用者のデータは含まない。ユーザー名を伏せたい場合は、置き換えてから送ってよい。エージェントは、ログをリポジトリに入れず、数値だけを記録する。

## アンインストール(すべてのあと)

- 利用者向けの`README.md`の「アンインストール」の手順どおりに削除し、手順と画面の違いがあれば書く(手動テスト18)。

## 付録: measure.ps1

手順Hの計測用のスクリプト。エージェントはWindowsで実行して確かめていない。Windows PowerShell 5.1で日本語のメッセージを正しく読ませるため、ファイルはBOM付きのUTF-8で保存する。

```powershell
# Flying Microwave: measure CPU, GPU and memory of the full-screen mode
# (plan 0001, N-03 and N-04, manual test 16).
#
# Usage (PowerShell, from any folder):
#   powershell -ExecutionPolicy Bypass -File .\measure.ps1 -Scr "<path to FlyingMicrowave.scr>"
#
# The script starts the screensaver with /s, samples its process once a
# minute, and beeps twice when it is done. Do not touch the mouse or the
# keyboard until the beeps; then press a key to end the screensaver.
# The results are written to a CSV file on the desktop.
# This script has not been run by the agent (it cannot run Windows).
# Saved as UTF-8 with a BOM, so that Windows PowerShell 5.1 reads the
# Japanese messages correctly.

param(
    [Parameter(Mandatory = $true)][string]$Scr,
    [int]$Minutes = 10,
    [string]$Out = (Join-Path ([Environment]::GetFolderPath('Desktop')) 'flying-microwave-measure.csv')
)

$ErrorActionPreference = 'Stop'
$Scr = (Resolve-Path -LiteralPath $Scr).Path

# Display modes, so that the CPU figure can be compared with N-03 (4K, 60 Hz).
Get-CimInstance Win32_VideoController |
    Select-Object Name, CurrentHorizontalResolution, CurrentVerticalResolution, CurrentRefreshRate |
    Format-Table -AutoSize | Out-String | Write-Output

# Why: CreateProcess (UseShellExecute = false) starts the .scr as a program
# with exactly the argument /s, without the shell's verb for .scr files.
$info = New-Object System.Diagnostics.ProcessStartInfo
$info.FileName = $Scr
$info.Arguments = '/s'
$info.UseShellExecute = $false
$p = [System.Diagnostics.Process]::Start($info)

# Skip the start-up (first frame within 1 s is N-01, measured from the log).
Start-Sleep -Seconds 10

function Get-GpuPercent([int]$ProcessId) {
    # Why: the WMI class has English names on every Windows language,
    # unlike the counter paths of Get-Counter.
    try {
        $engines = Get-CimInstance Win32_PerfFormattedData_GPUPerformanceCounters_GPUEngine |
            Where-Object { $_.Name -like "pid_$($ProcessId)_*" }
        return [math]::Round((($engines | Measure-Object -Property UtilizationPercentage -Sum).Sum), 1)
    } catch {
        return 'n/a'
    }
}

$rows = @()
$p.Refresh()
$lastCpu = $p.TotalProcessorTime.TotalSeconds
$lastTime = Get-Date
for ($i = 1; $i -le $Minutes; $i++) {
    Start-Sleep -Seconds 60
    if ($p.HasExited) { Write-Output "スクリーンセーバーが $i 分目に終了した(計測の途中で終わった)"; break }
    $p.Refresh()
    $now = Get-Date
    $cpu = $p.TotalProcessorTime.TotalSeconds
    $rows += [pscustomobject]@{
        Minute               = $i
        CpuPercentOfOneCore  = [math]::Round(($cpu - $lastCpu) / ($now - $lastTime).TotalSeconds * 100, 1)
        GpuPercent           = Get-GpuPercent $p.Id
        WorkingSetMB         = [math]::Round($p.WorkingSet64 / 1MB, 1)
        PrivateMB            = [math]::Round($p.PrivateMemorySize64 / 1MB, 1)
    }
    $lastCpu = $cpu
    $lastTime = $now
}

$rows | Export-Csv -LiteralPath $Out -NoTypeInformation -Encoding UTF8
$rows | Format-Table -AutoSize | Out-String | Write-Output
[console]::Beep(880, 300); [console]::Beep(880, 300)

# Manual test 17: no process may be left after the screensaver ends.
if (-not $p.WaitForExit(120000)) {
    Write-Output '合図の音から2分たっても、スクリーンセーバーが終わっていない'
}
Start-Sleep -Seconds 2
# Why: whether ProcessName keeps the .scr extension is not confirmed, so
# both forms are matched.
$left = Get-Process | Where-Object { $_.ProcessName -like 'FlyingMicrowave*' }
if ($left) { Write-Output "プロセスが残っている: $($left.Id -join ', ')" } else { Write-Output 'FlyingMicrowave のプロセスは残っていない' }
Write-Output "結果のファイル: $Out"
```

## 付録: flying-microwave-s6.wsb

手順KのWindows Sandboxの設定。仮想のGPUとネットワークを無効にし、`C:\FlyingMicrowaveTest\`を読み取り専用で`C:\FlyingMicrowave\`に見せる。

```xml
<Configuration>
  <vGPU>Disable</vGPU>
  <Networking>Disable</Networking>
  <MappedFolders>
    <MappedFolder>
      <HostFolder>C:\FlyingMicrowaveTest</HostFolder>
      <SandboxFolder>C:\FlyingMicrowave</SandboxFolder>
      <ReadOnly>true</ReadOnly>
    </MappedFolder>
  </MappedFolders>
</Configuration>
```
