# ADR-0028: スワップチェーンの待機オブジェクトを、待たずに確かめてから描く

- ステータス: 採用
- 日付: 2026-11-14

## 背景

CIのスモークテストの F-01(キーを押してから画面が消えるまで200ms以内)が、ときどき失敗していた([incident 0028](../incidents/0028-smoke-failure-merged-on-a-guess.md))。PR #81で加えた計測で、WARPでは、描く先の画像を受け取る処理(`get_current_texture`)が約1秒止まっていたことが分かった(incident 0028の「その後の確認」の表)。

wgpu 30.0.1のソースで確かめた待ちの仕組み(Cargoが取得したクレートの中のファイル):

- wgpu-coreの`Surface::get_current_texture`は、画像の取得を上限1000msで待つ(`wgpu-core-30.0.1/src/present.rs`の32行目の`FRAME_TIMEOUT_MS`と、177〜178行目)。
- DX12では、画像を返す前に、スワップチェーンの待機オブジェクト(`GetFrameLatencyWaitableObject`)を、この上限まで待つ(`wgpu-hal-30.0.1/src/dx12/mod.rs`の`acquire_texture`。既定は`Dx12UseFrameLatencyWaitableObject::Wait`)。上限に達しても、そのまま画像を返す(同じファイルの`SwapChain::wait`は`Ok(false)`を返し、`acquire_texture`はその値を見ない)。

製品は、描画とキーの処理を同じスレッドのイベントループで行う([ADR-0025](0025-draw-every-window-from-one-place.md)。`crates/app/src/saver.rs`)。そのため、この待ちの最中にキーが届くと、キーの処理が最大で約1秒遅れる。

マージしない試しのPR #82で、3つの案をCIにかけた(incident 0028の「その後の確認」)。

- 案A(表示の方式をMailboxにする): 待ちはなくならず、F-01が失敗した。
- 案B(待機オブジェクトを使わない。`None`): 待ちはなくなったが、終了の処理が止まり、2秒たってもプロセスが終わらなかった。
- 案E(wgpuには待たせず(`DontWait`)、描く前に自分で待ち時間0で確かめ、合図がまだならその回は描かない): 4つの確認がすべて通った。キーから画面が消えるまでは0msだった。

案Eでは、CIのWARPで、最初の2フレームのあと合図が一度も来ず、画面が止まったままだった(要約の行「23 draws attempted, 2 frames, 1 paced, 0 blank, 20 skipped」)。いまの方式(wgpuが1秒待つ)では、同じ環境でも1秒に1回は描く。

## 決定

判断していただいたこと(2026-11-14、メンテナーの判断):

- 案Eを、F-01の直し方として採る。
- 描けなかったときに試し直すまでの間隔は、1フレーム分にする。
- 合図が来ないまま、最後に描いてから1秒(wgpuの上限と同じ)たったら、合図を待たずに描く。最悪でも、いまの方式と同じく1秒に1回は描き、キーの処理は止めない。

実装の方式(エージェントの案):

- wgpuのインスタンスを`Dx12UseFrameLatencyWaitableObject::DontWait`で作る(`crates/app/src/render.rs`の`create_instance`)。wgpuは待機オブジェクトを作るが、待たない(`wgpu-types-30.0.1/src/backend.rs`の871〜872行目の`DontWait`の説明。原文は「This is useful if the application wants to wait for the waitable object itself.」)。
- `Target::draw`の始めで、`wgpu::Surface::as_hal::<Dx12>`でwgpu-halのサーフェスを取り出し、`waitable_handle`で得たハンドルを、`WaitForSingleObject`の待ち時間0で確かめる(`Target::swap_chain_signalled`)。合図があれば、そのカウントを1つ消費する。wgpuの`Wait`の方式が待って消費するのと同じである。
- 描くかどうかは`core::pacing::may_draw`で決める。合図があれば描く。合図がなければ、最後に確かめを通った描画から`core::pacing::SIGNAL_WAIT_LIMIT`(1000ms)たつまで、その回を`Skip::NotReady`でスキップし、たったら合図なしで描く。最初の描画は待たない。
- スキップしたときは、`FramePacer::frame_period`(上限の間隔とモニターのリフレッシュの間隔の長いほう。どちらも分からなければ60分の1秒)のあとに試し直す(`crates/app/src/saver.rs`の`retry_delay`。設定画面のプレビュー枠も同じ)。ほかの理由のスキップは、これまでどおり50ms後に試し直す。
- 終了のときの要約の行(`crates/app/src/diag.rs`の`FrameStats::summary`)に、`NotReady`の回数と、合図なしで描いた回数(`N drawn without signal`)を加える。実機で合図が来ているかを、ログで判定できるようにする。

wgpuの内部(`as_hal`)と`unsafe`を使う理由: wgpuの公開のAPIには、待機オブジェクトを待たずに確かめる手段がない。wgpu自身が、`DontWait`を「アプリが自分で待つ」ための設定として用意し、`waitable_handle`を公開している(`wgpu-hal-30.0.1/src/dx12/mod.rs`の654〜658行目)。`unsafe`の条件は、wgpuの`Surface::as_hal`の説明(ガードを通して資源を破棄しない)と、wgpu-halの`waitable_handle`の説明(スワップチェーンが生きている間だけ有効)である。描く先の作り直しは同じスレッドの`Target::resize`と`Target::draw`の中だけで行うため、確かめる間はハンドルが有効である。

### 検討した選択肢と採用しなかった理由

- 案A(Mailbox): CIで待ちがなくならず、F-01が失敗した。
- 案B(待機オブジェクトを使わない): CIで終了の処理が止まった。
- 案E(合図が来るまで描かない): CIのWARPで画面が止まったままになった。WARPは、ハードウェアのGPUがない環境で使われるため、実機でも同じになりうる。メンテナーが、1秒で待たずに描く案を選んだ。
- 描画を別のスレッドに移す: キーの処理は止まらなくなるが、ウィンドウとサーフェスの扱いを大きく変える。ADR-0025の決定(イベントループの1か所から順に描く)を変えることになるため、採らない。
- wgpuの上限(`FRAME_TIMEOUT_MS`)を短くする: wgpuの定数で、変えられない。

## 結果

- メリット: 描く先の画像を待つ間も、キーの処理が止まらない。CIの試しで、F-01の失敗の原因(約1秒の待ち)がなくなった。合図が来ない環境でも、いまの方式と同じく1秒に1回は描く。
- デメリット・受け入れたトレードオフ:
  - wgpuの内部のAPI(`as_hal`、`waitable_handle`)と`unsafe`に頼る。wgpuを更新するときに、この部分が変わっていないかを確かめる必要がある。
  - 合図を待たずに確かめる回数が増える(合図がまだの間、1フレームごとに確かめる)。1回の確かめは`WaitForSingleObject`の待ち時間0の呼び出しだけである。
  - 実機のGPUで、同じ待ちが起きていたか、合図が毎フレーム来るかは、まだ確かめていない。`v0.3.0-alpha.4`の実機での確認で、要約の行の`NotReady`と`drawn without signal`の数で確かめる(計画0005の作業10の2)。
- 将来の見直し条件: wgpuの更新で、待たずに確かめる手段が公開のAPIに加わったとき、または`as_hal`や`waitable_handle`が変わったとき。実機で合図が来ずに`drawn without signal`が多いとき。描画を別のスレッドに移すとき。
