# 公開の手順の一覧(メンテナーの作業)

- 作成日: 2026-11-25
- 関連: [計画0005](../plans/0005-v0.3.0-visuals-and-clock.md)のD-22、D-46、D-47、作業15の2、[公開の前の脅威の点検](../reviews/0005-pre-publication-threat-review.md)、[再点検](../reviews/0006-pre-publication-threat-recheck.md)

いまのリポジトリの公開範囲を、PrivateからPublicに変える(D-46)ときに、メンテナーがGitHubとClaudeの画面で行う作業の一覧である。上から順に行う。エージェントが行う作業(リポジトリの中の対策)は、この一覧の前に済ませる。

## 0. 始める前の前提

- [ ] 再点検の対策(N-1〜N-4、N-6の文書の直し、N-7、N-8、N-10)を入れたPRが、`main`にマージされている。エージェントが行う。
- [ ] N-6のうち、タグのプッシュに確認を求めるフックの変更を入れるか、入れないかを決めている。エージェントの編集が自動の権限の判定で止められたため、まだ入っていない。入れるときは、メンテナーの承認を得てエージェントが入れる。

## 1. 公開の前

### 1.1 Actionsの設定(Settings → Actions → General)

- [ ] 「Actions permissions」で、使えるアクションを限る(J-10、点検の記録のS-2)。「Allow <オーナー名>, and select non-<オーナー名>, actions and reusable workflows」を選び、次の3つだけを許す。
  - `actions/checkout`
  - `actions/upload-artifact`
  - `Swatinem/rust-cache`
- [ ] 同じ画面で、「Require actions to be pinned to a full-length commit SHA」を有効にする(J-10)。
- [ ] 「Workflow permissions」を「Read repository contents and packages permissions」にする(S-3)。
- [ ] 同じ画面で、「Allow GitHub Actions to create and approve pull requests」を無効にする(S-3)。
- [ ] 「Check, workflow run, status, artifact and log retention」の値を見て、メモしておく(S-5)。変えなくてよい。この値は、実行の履歴の保存の期間にも効く。

### 1.2 空であること、余計なものがないことを確かめる

- [ ] Settings → Secrets and variables の Actions、Dependabot、Codespaces が、どれも空(S-6)。
- [ ] Settings → Environments が空(S-6)。
- [ ] Settings → Collaborators に、知らない人や招待がない(S-9)。
- [ ] Settings → Webhooks が空(S-15)。
- [ ] Settings → GitHub Apps(Integrations)で、入っているアプリと、それぞれの権限を確かめる(S-11)。Claude GitHub Appは、使うリポジトリだけに限る。

### 1.3 Claude側の設定(claude.ai)

- [ ] Claude Code(claude.ai/code)のAuto-fixを、このリポジトリのPRで使わない設定にする(N-5、S-12)。公開すると、誰でもPRにコメントでき、その内容がエージェントに届くため。
- [ ] 予定のタスク(Routines)に、このリポジトリのGitHubのイベント(Issueの作成、PRの作成など)で起動するものがないことを確かめる(N-7)。これからも作らない。
- [ ] 次の3つのセッションの共有の設定が、Privateであることを確かめる(S-17)。コミットのトレーラーとPRの本文(#1〜#87)に、URLが残るため。
  - 末尾が`JShz`のセッション(PR #1〜#9のころ)
  - 末尾が`qd5S`のセッション(PR #10〜#66のころ)
  - 末尾が`bZWV`のセッション(PR #67以降。いまのセッション)
  - PRの本文とコメントの編集の履歴は、エージェントの道具で読めなかった。上の3つのほかに、このリポジトリで使ったセッションがあれば、それもPrivateであることを確かめる。
- [ ] アカウントのプランを、エージェントに教える(J-7の残り。Claude Shipの扱いが、プランで違うため)。

### 1.4 クラウドの環境(claude.ai/code の環境の設定)

- [ ] セットアップスクリプトのactionlintの版を、onboardingの6.5と同じにする(N-11)。
- [ ] 許可するホスト(Allowed domains)から、このプロジェクトに関係のないものを外す(N-11)。外してよいものの一覧は、エージェントに作らせる。

### 1.5 Dependabot(Settings → Advanced Security)

- [ ] Dependabot alertsを有効にする(J-11、S-7)。
- [ ] Dependabot security updatesは無効のままにする(S-7。更新は、決まった日数をおいたバージョンの更新だけにするため)。

### 1.6 直前に確かめる

- [ ] 開いたPRがない。
- [ ] 進行中のワークフローの実行がない(Actionsの画面)。
- [ ] `main`の最新のCIが成功している。

## 2. 公開と同時

- [ ] Settings → General → Danger Zone の「Change repository visibility」で、Publicに変える。ブランチとタグのルールセット(`main`、`release-tags`)は、プッシュのルールセットではないため、そのまま残る。
- [ ] 公開すると現れる、Settings → Actions → General の「Approval for running fork pull request workflows from contributors」を、「Require approval for all external contributors」にする(J-3)。既定の「first-time contributors」のままだと、一度マージされた人のPRは承認なしで動く。
  - 外部の人のPRのワークフローを承認する前に、`.github/workflows/`の差分を見る。

## 3. 公開の直後

- [ ] Settings → Advanced Security の「Private vulnerability reporting」を有効にする(J-5)。有効にするまで、`SECURITY.md`が案内する「Report a vulnerability」が出ない。
- [ ] 同じ画面で、Secret ProtectionとPush protectionを有効にする(J-4)。最初のスキャンの結果を見て、見つかったものがあればエージェントに知らせる。
- [ ] 必要なら、Settings → Moderation options → Interaction limits で、コメントできる人を絞る(S-14、N-5)。
- [ ] 次の状態が保たれていることを確かめる。
  - Settings → Rules → Rulesets の2つが、どちらも「Active」。
  - 変更できないリリース(Immutable releases)が有効。
  - 1.1のActionsの設定。
- [ ] ログインしていない状態(ブラウザーのプライベートウィンドウなど)で、コード、リリース、Actionsのログが見えることを確かめる。

## 承知しておくこと

- 公開すると、スターとウォッチャーは消え、これまでの変更が活動として公開される(GitHubの文書「Setting repository visibility」)。
- 過去のActionsの実行の履歴とログは、そのまま公開される(J-9。ログの中身は再点検で確かめ、問題はなかった)。
- 隠し要素の内容は、開発者向けの文書(`docs/`)とソースで、誰でも読めるようになる(N-9。受け入れると決めた)。

## 外部の人のPRを手元で試すとき

公開のあと、外部の人のPRを自分のPCで試すときは、[開発参加者向けオンボーディング](onboarding.md)の9.2の手順に従う(N-3)。
