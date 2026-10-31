Flying Microwave <version>

----------------------------------------------------------------------
ENGLISH
----------------------------------------------------------------------

WHAT IT IS

Flying Microwave is a Windows screen saver in which microwave ovens
fly across your screen. It is inspired by the classic flying-toaster
screen savers of the early 1990s, and is an independent work that is
not affiliated with them.

REQUIREMENTS

- Windows 11 (x64 or arm64)

INSTALL

1. Recommended: verify the download. Compute the SHA-256 checksum of
   the zip file and compare it with the matching line in
   SHA256SUMS.txt on the release page. In PowerShell:

       Get-FileHash <path to the zip file> -Algorithm SHA256

2. Extract the zip file to a folder of your choice.
3. Right-click FlyingMicrowave.scr and choose Install.

SETTINGS

Open Screen Saver Settings, select Flying Microwave, and click
Settings. In v0.1.0 this only shows a message. The full settings
dialog arrives in v0.2.0.

UNINSTALL

1. Open Screen Saver Settings and select another screen saver (or
   "(None)").
2. Delete FlyingMicrowave.scr.

The location of the settings file will be documented here when
settings are added.

KNOWN LIMITATIONS

- The file is not code-signed. Windows SmartScreen may show a warning,
  and Smart App Control in enforcement mode may block it.
- Group Policy on managed PCs may prevent you from choosing a screen
  saver.
- The arm64 build may be untested on real hardware. See the release
  notes.

PRIVACY

This program does not use the network and does not collect or send
any data (no telemetry).

LICENSE

Flying Microwave is released under the MIT License (see the file
LICENSE). The licenses of third-party components are listed in the
file THIRD-PARTY-LICENSES. Both files are included in the zip.

HELP

Questions, bug reports, and the latest release: <project URL>

----------------------------------------------------------------------
日本語
----------------------------------------------------------------------

概要

Flying Microwave は、電子レンジが画面を飛び回る Windows 用のスクリーン
セーバーです。1990年代前半の古典的な「空飛ぶトースター」のスクリーン
セーバーに着想を得た独自の作品で、それらとは無関係です。

動作環境

- Windows 11 (x64 または arm64)

インストール

1. 推奨: ダウンロードしたファイルを検証します。zip ファイルの SHA-256
   チェックサムを計算し、リリースページの SHA256SUMS.txt の該当する行と
   比較してください。PowerShell の場合:

       Get-FileHash <zip ファイルのパス> -Algorithm SHA256

2. zip ファイルを任意のフォルダーに展開します。
3. FlyingMicrowave.scr を右クリックして [インストール] を選びます。

設定

スクリーン セーバーの設定を開き、Flying Microwave を選んで [設定] を
クリックします。v0.1.0 ではメッセージが表示されるだけです。設定画面は
v0.2.0 で追加されます。

アンインストール

1. スクリーン セーバーの設定を開き、別のスクリーン セーバー(または
   「(なし)」)を選びます。
2. FlyingMicrowave.scr を削除します。

設定ファイルの場所は、設定機能の追加時にここに記載します。

既知の制限

- このファイルにはコード署名がありません。Windows の SmartScreen が
  警告を表示することがあり、Smart App Control が強制モードの環境では
  ブロックされることがあります。
- 管理されている PC では、グループ ポリシーによってスクリーン セーバーを
  選べない場合があります。
- arm64 版は実機で未検証の場合があります。リリースノートを参照して
  ください。

プライバシー

このプログラムはネットワークを使用せず、データの収集や送信
(テレメトリ)も行いません。

ライセンス

Flying Microwave は MIT ライセンスで公開されています(LICENSE ファイル
を参照)。第三者のコンポーネントのライセンスは THIRD-PARTY-LICENSES
ファイルに記載しています。どちらも zip に同梱しています。

ヘルプ

質問、不具合の報告、最新のリリース: <project URL>
