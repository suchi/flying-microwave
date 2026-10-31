# Windowsスクリーンセーバーの仕様調査

- 調査日: 2026-10-31
- 目的: Windowsのスクリーンセーバー(`.scr`)が満たすべき仕様を一次情報で確認し、本プロジェクトの要件に落とし込む。
- 関連: [実装計画](../plans/0001-initial-implementation-plan.md)、[アーキテクチャ](../design/architecture.md)

## 要点

- スクリーンセーバーの実体は通常のWindows実行ファイルで、拡張子を`.scr`にしたもの。[MS-2]
- Windowsはコマンドライン引数(なし・`/c`・`/p <HWND>`・`/s`)でスクリーンセーバーに動作モードを伝える。Microsoftの`ScrnSave.lib`を使わない場合は、この引数を自前で処理する必要がある。[MS-1]
- 設定画面の一覧に表示される名前は、長いファイル名の場合はファイル名がそのまま使われる。[MS-2]
- 選択中のスクリーンセーバーは`HKCU\Control Panel\Desktop`の`SCRNSAVE.EXE`(REG_SZ)に保存される。[MS-3]
- Microsoftはスクリーンセーバーよりロック画面と電源プランを推奨している(グループポリシー・コントロールパネルのスクリーンセーバー機能は引き続き動作する)。[MS-4]

## 実体・配置・表示名

| 項目 | 一次情報の記述 | 本プロジェクトでの扱い |
| --- | --- | --- |
| ファイル形式 | コントロールパネルから実行されるには`.scr`拡張子が必要。単体アプリとして実行するには`.exe`拡張子が必要。[MS-2] | ビルド成果物の`.exe`を`.scr`にリネームして配布する。開発中は`.exe`のまま引数付きで起動して確認する。 |
| 一覧への登録 | コントロールパネルは「Windows Startup directory」から`.scr`ファイルを探す。[MS-2] | 文書内でこのディレクトリの具体的なパスは定義されていない。慣例的に`System32`へ置く例が多いが一次情報では未確認。後述のスパイクS5で確認する。 |
| 表示名 | 説明文字列はString Tableの序数1に置く。ただしファイル名が長い場合、コントロールパネルは説明文字列を無視しファイル名を使う。[MS-2] | 表示名はファイル名で決まる前提で命名する(案: `FlyingMicrowave.scr`)。念のためString Tableの序数1にも同じ名前を入れる。 |
| アイコン | `ID_APP`で識別されるアイコンは単体アプリとして実行したときのみ見える。[MS-2] | アイコンは任意。配布物の見分けやすさのため、リソースに埋め込む。 |
| 選択状態の保存 | `HKCU\Control Panel\Desktop\SCRNSAVE.EXE`(REG_SZ、ファイル名)。`rundll32.exe desk.cpl,InstallScreenSaver <file>`でも変更できる。グループポリシーの「Screen saver executable name」が有効な場合はこの値は無視される。[MS-3] | インストール手順(手動・スクリプト)の設計に使う。グループポリシーで固定された環境では選択できないことをREADMEに記載する。 |

## コマンドライン引数

`ScrnSave.lib`を使わない「4.0以上」のWin32スクリーンセーバーは、次の引数を処理しなければならない。[MS-1]

| 引数 | 意味(一次情報) | 本プロジェクトのモード |
| --- | --- | --- |
| (なし) | 設定ダイアログを表示する | Settings |
| `/c` | 設定ダイアログを、前面のウィンドウに対してモーダルで表示する | Settings |
| `/p <HWND>` | `<HWND>`の子ウィンドウとしてプレビューする | Preview |
| `/s` | スクリーンセーバーを実行する | Screensaver |
| `/a <HWND>` | Windows 95向け。パスワード変更 | 対象外 |

一次情報の補足:

- HWNDはコマンドライン上で符号なし10進数として表される。[MS-1]
- プレビューウィンドウは指定HWNDの子ウィンドウとして作成し、親のクライアント領域全体を覆う。[MS-1]

一次情報で確認できなかった点(推測を含むため実機で確認する):

- `/c:1234`のようにコロンでHWNDを付けた形式、大文字(`/S`)、`-s`のようなハイフン形式をWindowsが実際に渡すか。パーサはこれらを寛容に受理する設計とし、スパイクS3で実際の`argv`を記録して確認する。

## ScrnSave.libの既定動作(参考)

本プロジェクトは`ScrnSave.lib`を使わないが、Windows標準の振る舞いとして以下を参考にし、同等の終了条件を自前で実装する。[MS-2]

| メッセージ | `DefScreenSaverProc`の動作 |
| --- | --- |
| `WM_SETCURSOR` | カーソルを非表示にする |
| `WM_LBUTTONDOWN`・`WM_MBUTTONDOWN`・`WM_RBUTTONDOWN` | 終了する |
| `WM_KEYDOWN` | 終了する |
| `WM_MOUSEMOVE` | 終了する |
| `WM_ACTIVATE`(wParamがFALSE) | 終了する |

設計判断: マウスのわずかな揺れ(机の振動など)で終了しないよう、移動量に閾値を設ける。`ScrnSave.lib`内部に同様の閾値があるかは一次情報で確認できていない。

## 実行コンテキストとセキュリティ

- ユーザーが対話的にログオンしている場合、スクリーンセーバーはそのユーザーのセキュリティコンテキストで動く。[MS-2]
- ログオンしていない場合の挙動はWindowsのバージョンに依存する。文書にはWindows XP/2000(制限付きLocalSystem)とWindows Server 2003(特権を削除したLocalService)の記載のみで、Windows 10/11の記載はない。[MS-2]
- Windows Vista以降、パスワード保護がポリシーで有効な場合は、アプリケーションが`SC_SCREENSAVE`をどう処理してもスクリーンセーバーが起動する。[MS-2]
- パスワード関連の引数はWindows 95向けの`/a`のみで[MS-1]、本プロジェクトではパスワード処理を実装しない。

本プロジェクトへの帰結: ユーザープロファイルや設定ファイルにアクセスできない状況もありうるため、設定の読み込みに失敗しても既定値で動作し続ける設計にする。

## プラットフォームとしての位置付け

Microsoftの無人セットアップ(unattend)文書には次の注記がある。[MS-4]

> ScreenSaver functionality in Themes is removed. Screen saver functionality in Group Policies, Control Panel, and Sysprep continues to be functional. Lockscreen features and policies are preferred over using ScreenSaver.

スクリーンセーバー機能は現在も動作するが、Microsoftが推奨する方向ではない。将来のWindowsで機能が縮小されるリスクとして[実装計画のリスク表](../plans/0001-initial-implementation-plan.md#11-リスクと対策)に記載する。

## 本プロジェクトの要件への落とし込み

| ID | 要件 | 根拠 |
| --- | --- | --- |
| W-1 | 配布物は`.scr`拡張子の単一実行ファイルとする | [MS-2] |
| W-2 | 引数なし・`/c`(HWND付きを含む)で設定画面を表示する | [MS-1] |
| W-3 | `/p <HWND>`で、指定ウィンドウの子ウィンドウとしてクライアント領域全体にプレビューを描画する。親ウィンドウが消えたら終了する | [MS-1] |
| W-4 | `/s`で全画面表示し、キー入力・マウスボタン・一定以上のマウス移動・非アクティブ化で終了する。カーソルは非表示にする | [MS-1]、[MS-2] |
| W-5 | 表示名はファイル名で決まるため、ファイル名を製品名とする | [MS-2] |
| W-6 | 引数のHWNDは外部入力として検証してから使う(数値として解釈できるか、有効なウィンドウか) | AGENTS.mdのセキュリティ方針 |
| W-7 | 設定の読み込みに失敗しても既定値で動作する | [MS-2]の実行コンテキストの記述 |
| W-8 | 認識できない引数は、引数なしと同じく設定画面として扱う(誤って全画面起動しない) | 設計判断 |

## 未確認事項と検証方法(技術スパイク)

開発環境(Claude Code on the Web)はLinuxコンテナで、Windowsの画面を伴う動作は確認できない。以下はユーザーのWindows 11実機で確認し、結果を`docs/research/spike-results.md`(Phase 0で作成)に記録する。

| ID | 確認内容 | 確認方法 |
| --- | --- | --- |
| S1 | winitの子ウィンドウ(`with_parent_window`)とwgpuのサーフェスが、設定画面の小さなプレビュー枠内で描画できるか。親が閉じたときに終了できるか | 最小プログラムを`/p`で起動し、設定画面で別のスクリーンセーバーに切り替える |
| S2 | eframe(egui)の設定ウィンドウを、`/c`で渡されたウィンドウに対してモーダル相当にできるか | 設定画面の「設定」ボタンから起動する |
| S3 | Windowsが実際に渡す引数の形式(`/c:HWND`、大文字小文字など) | 受け取った`argv`をファイルに記録する診断ビルドで確認する |
| S4 | 複数モニタ(DPI混在を含む)での全画面表示、最前面化、カーソル非表示 | 2画面以上の環境で`/s`を実行する |
| S5 | インストール方法: 右クリックの「インストール」、`System32`への配置、任意フォルダへの配置で、一覧への表示と選択状態がどうなるか | 各方法を試し、`SCRNSAVE.EXE`の値を確認する |
| S6 | GPUが使えない環境(仮想マシン等)でのwgpuの挙動とフォールバック | 仮想マシンまたはGPUを無効化した環境で`/s`を実行する |

## 参考資料

- [MS-1] Microsoft Learn「Command-line arguments for Screen Savers」(KB182383、アーカイブ文書、2026-10-31取得)
- [MS-2] Microsoft Learn「Handling Screen Savers」(2026-10-31取得)
- [MS-3] Microsoft Learn「SCRNSAVE.EXE」(2026-10-31取得)
- [MS-4] Microsoft Learn「ScreenSaver」(Windows無人セットアップのリファレンス、2026-10-31取得)

[MS-1]: https://learn.microsoft.com/en-us/previous-versions/troubleshoot/windows/win32/screen-saver-command-line
[MS-2]: https://learn.microsoft.com/en-us/windows/win32/lwef/screen-saver-library
[MS-3]: https://learn.microsoft.com/en-us/windows/win32/devnotes/scrnsave-exe
[MS-4]: https://learn.microsoft.com/en-us/windows-hardware/customize/desktop/unattend/microsoft-windows-shell-setup-themes-screensaver
