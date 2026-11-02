# Flying Microwave

[日本語](#日本語) | [English](#english)

## 日本語

### 概要

Flying Microwave は、電子レンジが画面を飛び回る Windows 用のスクリーン セーバーです。古典的な「空飛ぶトースター」のスクリーン セーバーに着想を得た独自の作品で、その作者や権利者とは提携・承認などの関係はありません。

### 動作環境

- Windows 11(x64)
- arm64 版は、動作を保証せずに提供しています。

### インストール

1. 推奨: ダウンロードしたファイルを検証します。zip ファイルの SHA-256 チェックサムを計算し、リリース ページの `SHA256SUMS.txt` の該当する行と比較してください。PowerShell の場合:

   ```powershell
   Get-FileHash <zip ファイルのパス> -Algorithm SHA256
   ```

2. 置き続けるフォルダー(例: `%LOCALAPPDATA%\Programs\FlyingMicrowave`)を作り、そこに zip ファイルを展開します。ダウンロード フォルダーなどの一時的な場所は避けてください。後でファイルを移動・削除すると、スクリーン セーバーが動かなくなることがあります。
3. `FlyingMicrowave.scr` を右クリックして [インストール] を選びます。Windows 11 では、先に [その他のオプションを確認] を選ぶ必要がある場合があります。

スクリーン セーバーの設定では、ファイル名のとおり「FlyingMicrowave」と表示されます。

### 設定

スクリーン セーバーの設定を開き、FlyingMicrowave を選んで [設定] をクリックします。現在のバージョンでは、メッセージが表示されるだけです。設定画面は、今後のバージョンで追加します。

### アンインストール

1. スクリーン セーバーの設定を開き、別のスクリーン セーバー(または「(なし)」)を選びます。
2. インストールの手順 2 で作ったフォルダーを削除します(既存のフォルダーに展開した場合は、zip から展開したファイルだけを削除します)。
3. 必要なら、一時フォルダーの診断用のログ(`%TEMP%\flying-microwave.log`)も削除します。

### 既知の制限

- このファイルにはコード署名がありません。Windows の SmartScreen が警告を表示することがあり、Smart App Control が強制モードの環境ではブロックされる可能性が高いです。
- 管理されている PC では、グループ ポリシーによってスクリーン セーバーを選べない場合があります。
- この版は、x64 版も含めて、Windows 11 の実機での動作の確認をしていません。自動の確認だけを行っています。行った確認は、リリース ノートを参照してください。
- arm64 版は、動作を保証せずに提供しています。
- スクリーン セーバーの画面が前面に出なかった場合は、キーを押しても終わりません。押したキーが、画面の後ろにあるアプリに届くことがあります。その場合は、マウスを動かすかクリックして終わらせてください。

### プライバシー

このプログラムはネットワークを使用せず、データの収集や送信(テレメトリ)も行いません。不具合の調査のため、一時フォルダーに診断用のログ(`%TEMP%\flying-microwave.log`。1MB まで)を書きます。ログには、起動の情報、終了の理由と、PC の構成(GPU の名前とドライバーの版、モニターの位置と大きさ)が含まれます。文書などの利用者のデータは書かず、ログをどこかへ送信することもありません。

### ライセンス

Flying Microwave は MIT ライセンスで公開しています(`LICENSE.txt` を参照)。第三者のコンポーネントのライセンスは `THIRD-PARTY-LICENSES.txt` に記載しています。実行ファイルに組み込まれるソフトウェア部品と、そのコードを生成する部品の一覧(SBOM。`THIRD-PARTY-LICENSES.txt` と同じ範囲)を、SPDX 2.3 の形式で `FlyingMicrowave-<版>-windows-<x64 または arm64>.spdx.json` に記載しています。どれも zip に同梱しています。

### ヘルプ

質問、不具合の報告、最新のリリースは、次のページにあります。不具合を報告するときは、診断用のログを添えてください。報告は公開されるため、添える前にログの内容を確かめてください。

- プロジェクト: <https://github.com/suchi/flying-microwave>
- 不具合の報告: <https://github.com/suchi/flying-microwave/issues>

## English

### What it is

Flying Microwave is a Windows screen saver in which microwave ovens fly across your screen. It is inspired by the classic flying-toaster screen savers, and is an independent work that is not affiliated with or endorsed by their creators or rights holders.

### Requirements

- Windows 11 (x64)
- The arm64 build is provided without any guarantee that it works.

### Install

1. Recommended: verify the download. Compute the SHA-256 checksum of the zip file and compare it with the matching line in `SHA256SUMS.txt` on the release page. In PowerShell:

   ```powershell
   Get-FileHash <path to the zip file> -Algorithm SHA256
   ```

2. Create a folder that you will keep, for example `%LOCALAPPDATA%\Programs\FlyingMicrowave`, and extract the zip file there. Avoid temporary places such as the Downloads folder. If you move or delete the file later, the screen saver may stop working.
3. Right-click `FlyingMicrowave.scr` and choose Install. On Windows 11, you may need to choose "Show more options" first.

In Screen Saver Settings, the screen saver is listed as "FlyingMicrowave" (the name of the file).

### Settings

Open Screen Saver Settings, select FlyingMicrowave, and click Settings. The current version only shows a message. A settings dialog will be added in a future version.

### Uninstall

1. Open Screen Saver Settings and select another screen saver (or "(None)").
2. Delete the folder you created in step 2 of Install (or, if you used an existing folder, only the files extracted from the zip).
3. If you like, also delete the diagnostic log in your temporary folder (`%TEMP%\flying-microwave.log`).

### Known limitations

- The file is not code-signed. Windows SmartScreen may show a warning, and Smart App Control in enforcement mode is likely to block it.
- Group Policy on managed PCs may prevent you from choosing a screen saver.
- This version, including the x64 build, has not been tested on real Windows 11 hardware. It has only been checked automatically. See the release notes for the checks.
- The arm64 build is provided without any guarantee that it works.
- If the screen saver could not come to the front, pressing a key does not end it, and the key may reach the app behind the screen. In that case, move or click the mouse to end it.

### Privacy

This program does not use the network and does not collect or send any data (no telemetry). To help investigate problems, it writes a diagnostic log to your temporary folder (`%TEMP%\flying-microwave.log`, up to 1 MB). The log contains startup information, the reason for exiting, and details of your PC's configuration (the GPU name and driver version, and the positions and sizes of your monitors). It does not record your documents or other user data, and it is never sent anywhere.

### License

Flying Microwave is released under the MIT License (see `LICENSE.txt`). The licenses of third-party components are listed in `THIRD-PARTY-LICENSES.txt`. The software components built into the executable, and those that generate its code, are listed (an SBOM, covering the same components as `THIRD-PARTY-LICENSES.txt`) in SPDX 2.3 format in `FlyingMicrowave-<version>-windows-<x64 or arm64>.spdx.json`. All of these files are included in the zip.

### Help

Questions, bug reports, and the latest release are on the following pages. When you report a bug, please attach the diagnostic log. Reports are public, so check the contents of the log before you attach it.

- Project: <https://github.com/suchi/flying-microwave>
- Bug reports: <https://github.com/suchi/flying-microwave/issues>
