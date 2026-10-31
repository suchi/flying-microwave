Flying Microwave <version>

[Draft. These instructions are finalized before the first release,
after they are checked on Windows 11. Remove this note at that time.]

----------------------------------------------------------------------
ENGLISH
----------------------------------------------------------------------

WHAT IT IS

Flying Microwave is a Windows screen saver in which microwave ovens
fly across your screen. It is inspired by the classic flying-toaster
screen savers, and is an independent work that is not affiliated with
or endorsed by their creators or rights holders.

REQUIREMENTS

- Windows 11 (x64 or arm64)

INSTALL

1. Recommended: verify the download. Compute the SHA-256 checksum of
   the zip file and compare it with the matching line in
   SHA256SUMS.txt on the release page. In PowerShell:

       Get-FileHash <path to the zip file> -Algorithm SHA256

2. Create a folder that you will keep, for example
   %LOCALAPPDATA%\Programs\FlyingMicrowave, and extract the zip file
   there. Avoid temporary places such as the Downloads folder. If you
   move or delete the file later, the screen saver may stop working.
3. Right-click FlyingMicrowave.scr and choose Install. On Windows 11,
   you may need to choose "Show more options" first.

In Screen Saver Settings, the screen saver is listed as
"FlyingMicrowave" (the name of the file).

SETTINGS

Open Screen Saver Settings, select FlyingMicrowave, and click
Settings. In v0.1.0 this only shows a message. The full settings
dialog arrives in v0.2.0.

UNINSTALL

1. Open Screen Saver Settings and select another screen saver (or
   "(None)").
2. Delete the folder that contains FlyingMicrowave.scr.

The location of the settings file will be documented here when
settings are added.

KNOWN LIMITATIONS

- The file is not code-signed. Windows SmartScreen may show a warning,
  and Smart App Control in enforcement mode is likely to block it.
- Group Policy on managed PCs may prevent you from choosing a screen
  saver.
- The arm64 build may be untested on real hardware. See the release
  notes.

PRIVACY

This program does not use the network and does not collect or send
any data (no telemetry).

LICENSE

Flying Microwave is released under the MIT License (see
LICENSE.txt). The licenses of third-party components are listed in
THIRD-PARTY-LICENSES.txt. Both files are included in the zip.

HELP

Questions, bug reports, and the latest release: <project URL>

----------------------------------------------------------------------
日本語
----------------------------------------------------------------------

概要

Flying Microwave は、電子レンジが画面を飛び回る Windows 用のスクリーン
セーバーです。古典的な「空飛ぶトースター」のスクリーンセーバーに
着想を得た独自の作品で、その作者や権利者とは提携・承認などの関係は
ありません。

動作環境

- Windows 11 (x64 または arm64)

インストール

1. 推奨: ダウンロードしたファイルを検証します。zip ファイルの SHA-256
   チェックサムを計算し、リリースページの SHA256SUMS.txt の該当する行と
   比較してください。PowerShell の場合:

       Get-FileHash <zip ファイルのパス> -Algorithm SHA256

2. 置き続けるフォルダー(例: %LOCALAPPDATA%\Programs\FlyingMicrowave)
   を作り、そこに zip ファイルを展開します。ダウンロード フォルダーなどの
   一時的な場所は避けてください。後でファイルを移動・削除すると、
   スクリーン セーバーが動かなくなることがあります。
3. FlyingMicrowave.scr を右クリックして [インストール] を選びます。
   Windows 11 では、先に [その他のオプションを確認] を選ぶ必要がある
   場合があります。

スクリーン セーバーの設定では、ファイル名のとおり「FlyingMicrowave」と
表示されます。

設定

スクリーン セーバーの設定を開き、FlyingMicrowave を選んで [設定] を
クリックします。v0.1.0 ではメッセージが表示されるだけです。設定画面は
v0.2.0 で追加されます。

アンインストール

1. スクリーン セーバーの設定を開き、別のスクリーン セーバー(または
   「(なし)」)を選びます。
2. FlyingMicrowave.scr を置いたフォルダーを削除します。

設定ファイルの場所は、設定機能の追加時にここに記載します。

既知の制限

- このファイルにはコード署名がありません。Windows の SmartScreen が
  警告を表示することがあり、Smart App Control が強制モードの環境では
  ブロックされる可能性が高いです。
- 管理されている PC では、グループ ポリシーによってスクリーン セーバーを
  選べない場合があります。
- arm64 版は実機で未検証の場合があります。リリースノートを参照して
  ください。

プライバシー

このプログラムはネットワークを使用せず、データの収集や送信
(テレメトリ)も行いません。

ライセンス

Flying Microwave は MIT ライセンスで公開されています(LICENSE.txt を
参照)。第三者のコンポーネントのライセンスは THIRD-PARTY-LICENSES.txt
に記載しています。どちらも zip に同梱しています。

ヘルプ

質問、不具合の報告、最新のリリース: <project URL>
