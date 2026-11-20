# 公開の前の脅威の点検(計画0005の作業15)

- 日付: 2026-11-18
- 時点: `main`のPR #86のマージ`5559159`(2026-11-18 21:02 JST)と、その上の作業ブランチのコミット`bd50727`(incidentのステータスの更新だけ)。コミット808件、タグ17件、PR 78件、Issue 8件
- 対象: [計画0005](../plans/0005-v0.3.0-visuals-and-clock.md)の作業15に挙げた項目(プロンプトインジェクション、GitHub Actions、リポジトリの設定、開発の道具、前回の点検の残り)
- 目的: リポジトリを公開したときに外部の人が起こせることを、一次情報で確かめ、メンテナーの判断が要る点をまとめる(計画0005のD-9、E-7)
- 方法: 3つの領域に分け、エージェント(最も高性能なモデル)が読むだけで調べた。A: プロンプトインジェクションと開発の道具。B: GitHub Actionsとリポジトリの設定。C: 前回の点検([0004](0004-pre-publication-audit.md))の残り。一次情報は、Claude Codeの公式の文書(code.claude.com/docs)、GitHubの公式の文書(docs.github.com)、リポジトリのファイル、GitHubのMCPで読める情報。公式の文書は2026-11-18に取得した。リポジトリの設定の値は、エージェントからは読めないため、メンテナーが画面で確かめる項目にした(下の「画面で確かめる項目」)
- 書き方: 個人を特定できる値(メールアドレス、氏名、アカウント名、セッションのID)そのものは書かず、種類、件数、コミットのSHA、IssueやPRの番号で示す(0004と同じ)
- 状態: 2026-11-19に判断を受け取った(下の「判断」)。対策のうち、リポジトリの中で行うものは、作業ブランチ`claude/jolly-hypatia-hw287j`のPRで反映した。画面での設定と、公開用のリポジトリへの移行は、[計画0005](../plans/0005-v0.3.0-visuals-and-clock.md)の作業15と作業15の2に登録した

## 結論

- 外部の人の操作で、エージェントやワークフローが自動で起動する仕組みは、リポジトリの中にはなかった。
- フォークからのPRが、書き込みの権限、シークレット、`main`のキャッシュを得る経路はなかった。ワークフローへのスクリプトの注入の箇所もなかった。
- 新しい個人を特定できる値や秘密情報は、履歴、Issue、PR、コメントのどこにもなかった(セッションのURLを除く)。
- 弱いのは、規則と手順と、公開の前後に変える設定である。外部の人のPRとコメントの扱いが決まっていない。シークレットのスキャンと、脆弱性の非公開の報告の窓口がない。

## 結果

### A. プロンプトインジェクションと開発の道具

| 項目 | 確かめた方法 | 結果 | 判定 |
| --- | --- | --- | --- |
| 外部の人が書いた文をエージェントが読む経路 | `.claude/commands/babysit-pr.md`の12、21、27行、`docs/development/review-process.md`の42、43行、Claude Codeの文書(claude-code-on-the-webの「Auto-fix pull requests」)、MCPでのPRの作成者の一覧(78件) | 公開のあとは、次の経路がある。PRのコメントとレビューのコメント(PRを購読したセッションが読む。Auto-fixは、レビューのコメントを受けて修正をプッシュする)。Issue。外部の人のPRの本文、タイトル、ブランチ名、コミットメッセージ、差分(敵対的レビューのサブエージェントが読む)。CIのログ(babysit-prの手順1)。DependabotのPRの本文(いまもある。PR #83)。PRの作成者は、これまでメンテナーのアカウントかDependabotだけだった。コメントを書いた人の権限で絞るかは、文書に記述がなく確かめられなかった | 要判断(J-1、J-2) |
| 外部の文の指示に従わないための規則 | AGENTS.mdの「セキュリティ」、「PR・レビュー・マージ」の手順3と6、`grep -rnE "author_association\|コラボレーター\|外部の人\|フォーク" AGENTS.md docs/development/*.md .claude/commands/*.md`(0件) | 外部の文の「指示」に従わず、止めて報告する規則はある。足りないのは3点。(1) メンテナーの指摘と、それ以外の人の文を分ける規則がない(手順は「人間のレビュー」に対応して返信することを求める)。(2) 外部の人のPRを取り出してビルドやテストをすると、そのPRの`build.rs`、テスト、`scripts/`、`.claude/settings.json`のフックが動く(Claude Codeのhooksの文書。フックの設定の変更はファイルの監視で読み直される。permissionsの文書。信頼はリポジトリのルートに付く)。クラウドのセッションには、ブランチにプッシュできる資格情報がある(securityの文書の「Cloud execution security」)。外部の人のPRの扱いの規則がない。(3) 外部由来の例の並びに、コミットメッセージ、ブランチ名、PRのタイトル、差分の中のコメント、DependabotのPRの本文がない | 要判断(J-1、J-2)、要対応(K-3) |
| 外部の人の操作で起動する仕組み | `.github/workflows/ci.yml`の15〜19行、`release.yml`の21〜29行、`${{`の展開の一覧、追跡しているファイルの`claude-code-action\|@claude\|ANTHROPIC_API_KEY\|CLAUDE_CODE_OAUTH`の検索 | トリガーは`pull_request`、`push`(`main`とタグ`v*`)、`workflow_dispatch`だけ。`issue_comment`、`pull_request_target`、`workflow_run`、ラベルのトリガーはない。claude-code-actionの痕跡もない。リポジトリの外の仕組み(Claude GitHub App、Auto-fix、Code Review)は、ファイルからは確かめられない | 問題なし(リポジトリの外は画面で確かめる。S-11〜S-13) |
| クローンした人の環境で動くもの | Claude Codeの文書(permissionsの「What runs before you trust a folder」、hooksの「Workspace trust」)、追跡しているファイルの一覧 | `.claude/settings.json`のフック(PreToolUse 4件、Stop、SubagentStop)は、対話のセッションでは、フォルダを信頼する前には動かない。`claude -p`では、信頼の確認なしで動く。`permissions.allow`、`env`、`apiKeyHelper`、`.mcp.json`、サブエージェント、プラグイン、スキルの`allowed-tools`はない。フックは、拒否や確認を返すほかは状態ファイルを書くだけで、ネットワークにつながない(5つのスクリプトを全文読んだ)。`CLAUDE.md`が信頼の前に読み込まれるかは、文書に記述がなく確かめられなかった | 問題なし |
| 日本語を求めるフックと外部の貢献者 | `require-japanese-description.sh`の29〜34行、`require-japanese-note.sh`の79行、`require-japanese-reply.sh`の62〜86行、`block-dangerous-git.sh`の22〜24行 | 英語で書く外部の貢献者の説明文と返答を拒む。jqのない環境では、Bashを呼ぶたびに確認を求める。安全性の問題ではない | 要判断(J-14) |
| フックが外部の入力をどう扱うか | 5つのスクリプトを全文読み、登録したとおりの起動で試験の入力(コマンドの置き換えを含む)を与えた | 値はjqの`--arg`か`printf '%s'`で扱い、`eval`も引用符のない展開もない。ファイル名に使う値は使える文字を絞り、状態のフォルダは持ち主とリンクを確かめて700にしている。試験の入力でファイルは作られなかった | 問題なし |
| `main`へのプッシュを止めるフックの迂回 | 同じ試験と、記録の軽いレビューでの登録したとおりの起動での試験 | 次の形の`main`へのプッシュは止まらない(`block-dangerous-git.sh`の44行と62行の正規表現。62行は、送り元の名前として`HEAD:`だけを許す): `git`のオプションを挟んだ形、`refs/heads/`を付けた形(`HEAD:refs/heads/main`を含む)、`+`を付けた形、`HEAD`以外の送り元を付けた形(`<ブランチ>:main`)。`main`、`HEAD:main`、`main:main`は止まる。本当の防御は、GitHubの`main`のルールセットである | 要対応(K-1) |
| 設定ファイルを守るフックの対象 | `.claude/settings.json`の25行、Claude Codeのpermission-modesの「Protected paths」 | `protect-config.sh`は、Edit、Write、MultiEditだけが対象で、Bashでの書き込みとNotebookEditは通る。`.claude/`と`.cargo/`はClaude Codeが保護するパスだが、`.github/workflows/`、`AGENTS.md`、`CLAUDE.md`、`.agents/skills/`は対象外。AGENTS.mdには「重要な指示は本ファイルに追記する」の規則があり、注入された「指示」が規則のファイルに残るおそれがある | 要判断(J-6) |
| 権限の設定 | `.claude/settings.json`の1〜12行、Claude Codeのpermissionsの「What Read and Edit rules cover」、settingsの文書 | `allow`はなく、`deny`は`Read`の7件だけで、広すぎない。`deny`は`grep -r`やPythonには効かない(公式の文書の警告のとおり)。`.claude/settings.local.json`が`.gitignore`にない(Claude Codeが自分で作るときは、グローバルな除外に加えるが、手で作ったときは加えない) | 要対応(K-2) |
| そのほかの道具 | `.cargo/config.toml`、`crates/app/build.rs` | `.cargo/config.toml`は`rustflags`だけで、`runner`などはない。`build.rs`は1つ | 問題なし |

### B. GitHub Actionsとリポジトリの設定

| 項目 | 確かめた方法 | 結果 | 判定 |
| --- | --- | --- | --- |
| フォークからのPRで動くワークフロー | `ci.yml`の15〜19行、`release.yml`の21〜29行、`pull_request_target\|workflow_run\|issue_comment\|secrets.\|environment:\|self-hosted`の検索(0件)、GitHubの「Events that trigger workflows」の「Workflows in forked repositories」 | フォークのPRで動くのは`ci.yml`(`pull_request`)だけ。フォークからの実行には、`GITHUB_TOKEN`以外のシークレットは渡されず、`GITHUB_TOKEN`は読み取りだけになる(DependabotのPRも同じ) | 問題なし |
| トークンの権限 | `ci.yml`の21〜22行、`release.yml`の31〜32、147〜149、319〜320、360〜362行 | 書き込みの権限は、リリースのpublishのジョブ(`gh`と`sha256sum`だけを使う)にだけある。チェックアウトは6か所とも`persist-credentials: false` | 問題なし(既定の権限はS-3) |
| キャッシュの汚染 | `ci.yml`の74〜76、128〜131行、`release.yml`の181行のコメント、GitHubの「Dependency caching reference」の「Restrictions for accessing a cache」と「Cache access for low-trust workflow triggers」、[ADR-0017](../adr/0017-ci-build-cache-and-x64-spike-builds.md) | フォークのPRは`main`のキャッシュを読めるが、書き換えられない。リリースのワークフローはキャッシュを復元しない。ADR-0017の記述は、いまの公式の文書と一致していた | 問題なし |
| 成果物とログ | GitHubの「Downloading workflow artifacts」、「Setting repository visibility」、`ci.yml`の8行と213〜218行、`docs/development/onboarding.md`の417行 | 公開すると、誰でも、過去を含むすべての実行のログを読み、成果物をダウンロードできる。フォークのPRのCIも、同じ名前の成果物(外部の人のコードからのビルド)を作る。いまの手順では、実行のたびの成果物を、実機での確認に使ってよい(onboardingの417行、`ci.yml`の8行のコメント)。過去のログの中身は確かめていない | 要判断(J-9、J-16) |
| リリースのワークフローの起動の条件 | `release.yml`の5〜7、21〜23、79〜82行、`docs/development/onboarding.md`の9.1(435〜441行)、`scripts/prepare_release.py`の28〜33行、GitHubの「Available rules for rulesets」、MCPで読んだ共同作業者の一覧とリリースの実行の一覧 | タグのプッシュでは、タグを付けたコミットにある`release.yml`が動く。「タグが`main`の上にあるか」の検査もそのファイルの中にあるため、実際の防御はタグのルールセット`release-tags`である。共同作業者は所有者の1件(admin)だけ | 問題なし(ルールセットが有効かはS-1) |
| 手動実行の権限 | GitHubの「Manually running a workflow」(書き込みの権限が要る)、`release.yml`の73〜76行 | 外部の人が起動する経路はない | 問題なし(共同作業者を加えるときは見直す) |
| 初めての貢献者のワークフローの承認 | GitHubの「Managing GitHub Actions settings for a repository」の「Controlling changes from forks to workflows in public repositories」 | 既定では、初めての貢献者だけ承認が要る。一度マージされた人は承認なしで動き、誤字の修正のマージでこの条件を満たせる(同じ節の警告)。承認なしで動くと、外部の人が書き換えた`ci.yml`が動き、ログに好きな文を出せる。エージェントはCIのログを読むことがある(上のA) | 要判断(J-3) |
| アクションの固定とDependabot | `git ls-remote`での確認、`.github/dependabot.yml`の12〜27行、GitHubの「Secure use reference」の「Using third-party actions」 | `actions/checkout`(v7.0.1)、`actions/upload-artifact`(v7.0.2)、`Swatinem/rust-cache`(v2.9.3)の3つとも、固定したSHAがタグと一致した。Dependabotの`cooldown`は7日 | 問題なし(任意の強化はJ-10) |
| スクリプトの注入 | `grep -n '\${{' .github/workflows/*.yml`(`grep -c`で、`ci.yml`は15行、`release.yml`は28行)、GitHubの「Secure use reference」の「Use an intermediate environment variable」 | `run:`に直接埋め込むのは`matrix.target`(固定の値)だけ。`github.event.*`は使っていない。タグの名前は環境変数で渡している | 問題なし |
| 必須のステータスチェックの名前 | GitHubの「Available rules for rulesets」の「Require status checks to pass before merging」、onboardingの9.1 | 外部の人のPRが`ci.yml`を書き換えると、同じ名前のジョブを中身なしで成功させられる。マージできるのはメンテナーだけなので、被害は「検査が通ったように見える」ことに限られる | 要対応(K-4) |
| ブランチとタグのルールセット | onboardingの9.1、MCPの`list_branches`、GitHubの「Setting repository visibility」 | 記録上のルールセットは、`main`と`release-tags`の2つ。公開で無効になる「プッシュのルールセット」は、記録上ない | 問題なし(値はS-1) |
| Dependabot | `.github/dependabot.yml`の9〜57行、GitHubの「Dependabot options reference」の`cooldown` | 設定は妥当。`cooldown`はセキュリティの更新には効かない。`rust-toolchain.toml`は対象に入っていない。`dependabot.yml`が指定するラベル`dependencies`が、リポジトリにない(PR #83のDependabotの通知) | 要判断(J-11)、要対応(K-6) |
| シークレットのスキャンとプッシュの保護 | GitHubの「Secret scanning」、「Push protection」、「Setting repository visibility」 | 個人のアカウントの非公開のリポジトリでは使えない。公開したら、手で有効にする必要がある。前回の点検では、履歴の秘密情報は0件だった(0004の20行) | 要判断(J-4) |
| `SECURITY.md`、脆弱性の非公開の報告、CODEOWNERS | `ls`(どれもない)、GitHubの「Configuring private vulnerability reporting for a repository」、「Adding a security policy to your repository」、「About code owners」 | 脆弱性を非公開で報告する窓口がない。CODEOWNERSは、必要な承認の数が0で、PRはすべてメンテナーのアカウントで作られる(自分のPRは承認できない)ため、置いても効果は小さい | 要判断(J-5、J-12) |
| 変更できないリリース | GitHubの「Immutable releases」(`/en/code-security/concepts/supply-chain-security/immutable-releases`)と「Preventing changes to your releases」 | 有効にすると、公開したリリースのタグと添付のファイルが固定され、リリースの証明(attestation)が作られる。公開したリリースのタグは動かせず、消したあとも同じ名前を使えない。有効にしたあとのリリースにだけ効く。いまの「下書きを作り、メンテナーが公開する」流れは、公式の文書が勧める手順と同じである | 要判断(J-13) |
| 公開で変わる設定 | GitHubの「Setting repository visibility」の「Making a repository public」 | 誰でも読めてフォークでき、プッシュのルールセットが無効になり、Actionsの履歴とログを誰でも見られ、スターとウォッチャーが消え、Advanced Securityの機能が使えるようになり、フォークのPRの承認の設定が使われるようになる | 要判断(J-13) |

### C. 前回の点検の残り

調べたリポジトリは浅いクローンだったため、`origin`の完全な複製を作業用の場所に作って調べた(ブランチ2本、タグ17本、`refs/pull/*/head` 78本)。前回の時点は、0004の「時点」にあるPR #57のマージ`0d140cd`(2026-11-09 17:25 JST)とした。全体は808件で、`0d140cd`から辿れる分が547件、増えた分が261件である。前回の記録の件数(548件、ツールの作者494件、トレーラー487件)とは、どれも1件ずつ違う。前回がどの参照を数えたかは、記録がなく分からない。

| 項目 | 確かめた方法 | 結果 | 判定 |
| --- | --- | --- | --- |
| コミットの作者とコミッターのメールアドレス | 複製で`git log <範囲> --format='%ae'`と`'%ce'`を集計(範囲は全体、増えた分、`0d140cd`まで) | 全体808件のうち、作者はツール725件、メンテナーのもの82件、Dependabotのbot 1件。増えた261件のうち、メンテナーのもの28件は、どれもGitHubでのPRのマージ。前回の推奨(GitHubのnoreplyのアドレスに切り替える)は、増えた分にはまだ反映されていない | 問題なし(メンテナーのアドレスは2026-11-09の判断で公開してよい) |
| 作者名とコミッター名 | 同じ範囲で`'%an'`と`'%cn'`を集計 | 作者名はツール、メンテナーのアカウント名、Dependabotのbotの3種類。コミッター名は、ツール、`GitHub`、メンテナーのアカウント名の3種類(前回と同じ) | 問題なし |
| テンプレート由来のアカウント名 | 名前を`f467cb7`の`.github/dependabot.yml`から取り、出力では伏せて、差分(`git log --all -G -i`)、コミットメッセージ、`main`とブランチの先頭、作業ツリー、タグの注釈を検索 | 差分に残るのは、前回と同じ5件(`f467cb7`、`ae9a5f3`、`bbbea20`、`b57a49f`、`9e41046`)。増えた分には0件。5件は、`main`、ブランチ、17本のタグ、78本の`refs/pull/*/head`のすべてから辿れる(`git for-each-ref --contains`) | 要判断(J-8) |
| 今のファイルの個人を特定できる情報 | `git grep -I`で、メールアドレスの形、`C:\Users`、`/Users/`、`/home/`、セッションのIDの形、敬称を検索 | メールアドレスはツールのものだけ。パスはテスト用の汎用の値(`crates/app/src/storage.rs`の359行と366行)と、前回の記録の本文だけ。セッションのIDと敬称は0件。メンテナーのアカウント名は、配布元のURLと`dependabot.yml`の`assignees`だけ | 問題なし |
| 履歴の差分の個人を特定できる情報 | `git log --all -p`の追加行を、メールアドレスとパスの形で検索 | メールアドレスはツールのものだけ。パスは、前回の記録の本文と、伏せた書き方の注意書きと、テスト用の値だけ | 問題なし |
| コミットメッセージ | `git log --all --format=%B`と`%s`を検索 | 本文のメールアドレスは、`Co-Authored-By:`のツールのもの717件と、Dependabotの`Signed-off-by:`のbotのもの1件だけ。メンテナーのアカウント名は、マージの件名(`Merge pull request #N from <所有者>/<ブランチ>`)の77件だけ | 問題なし(2026-11-09の判断で公開してよい) |
| 秘密情報 | 全体の差分の追加行で、トークンと鍵の形(`gh[opsu]_`、`github_pat_`、`AKIA`、秘密鍵の見出し、`sk-ant-`、`xox?-`、`AIza`、JWTなど)と代入の形を検索し、40桁以上の16進の並び(632件)を分類 | 鍵とトークンの形は0件。16進の並びは、Cargoのチェックサム607件、Actionsの固定のSHA 18件、配布物のSHA-256 3件、コミットのSHA 2件、テスト用のダミー2件 | 問題なし(検索の形にない鍵は確かめていない。GitHubのsecret scanningは実行していない) |
| 仮の値 | `git grep`で`TODO(project)`、`<project URL>`、`example.com`、`CHANGEME`、`FIXME`などを検索 | `dependabot.yml`の`TODO(project)`はなくなった(PR #58)。`AGENTS.md`の92行に、テンプレート由来の`TODO(project)`のHTMLのコメント(言語の表を上書きする場合の注意)が残る。`f467cb7`から入っている。0004の21行は`AGENTS.md`の`TODO(project)`をテンプレートの使い方の説明として扱っており、92行のコメントを含めたかどうかは、記録がなく分からない | 要判断(J-15) |
| `Claude-Session:`のトレーラー | `git log <範囲> --format=%B`で`^Claude-Session:`の行を数え、IDは種類の数と件数だけを出した | 全体717件(増えた分231件)。すべて`Claude-Session: https://claude.ai/code/session_<ID>`の形。IDは3種類 | 要判断(J-7) |
| 所有者以外がセッションのURLを開いたときに見えるもの | Claude Codeの文書(`llms.txt`、claude-code-on-the-webの「Share sessions」、cloud-environmentsの「Link output back to the session」、settings-referenceの`attribution.sessionUrl`)を取得して検索 | 共有するには公開範囲を切り替える。公開範囲は、Team・EnterpriseではPrivateとTeam、Pro・Max・UltraではPrivateとPublicで、Publicはclaude.aiにログインしたすべての利用者に見える。`attribution.sessionUrl`を`false`にすると、トレーラーとPRの本文のリンクを付けなくなる(既定は`true`)。Privateのセッションを所有者以外が開いたときに何が見えるかは、文書に記述がなかった。既定の公開範囲がPrivateであるとも、明文では書かれていない | 要判断(J-7) |
| Issueの本文とコメント | MCPで8件すべて(#2、#3、#28、#29、#44、#47、#50、#59)と、コメント4件すべてを読んだ | 個人を特定できる値、秘密情報、会話の引用はない | 問題なし |
| PRの本文 | MCPで78件すべてを読み、機械的にも検索した | メールアドレス、テンプレートの作者の名前、ローカルのパス、秘密情報は0件。セッションのURLが、DependabotのPR #83を除く77件にある(IDはトレーラーと同じ3種類)。環境の情報(PR #15のWindowsのビルド番号、GPUの型番、モニターの台数)はあるが、ホスト名やIPアドレスはない | 要判断(J-7)。それ以外は問題なし |
| PRのコメント | MCPで、コメントのある78件のPRのコメント99件すべてを読んだ | 98件はメンテナーのアカウントで投稿された記録、1件はDependabot(#83)。個人を特定できる値、秘密情報、セッションのURLはない | 問題なし |
| レビューとレビューコメント | 検索のAPI(`is:pr -review:none`で0件)、MCPで6回(#1、#49、#83のレビュー、#1、#82、#86のレビューコメント) | どれも0件。残りの72件のPRは、MCPでは1件ずつ確かめていない | 問題なし(一部) |
| 指示らしき文 | 読んだ本文とコメントのすべて | 外部からの指示はない。指示の形の文は、PR #83のDependabotの通常の通知(ラベルがないので`dependabot.yml`を直すよう求めるもの)1件だけで、これには従っていない | 問題なし |

## 判断が要ること

重さの順に並べる。「推奨」はエージェントの案である。設定の変更(`.claude/`、`.github/workflows/`、`dependabot.yml`、ルールセットなど)は、どれもメンテナーの承認が要る(AGENTS.mdの「セキュリティ」)。

| 番号 | 重さ | 判断していただくこと | 推奨 |
| --- | --- | --- | --- |
| J-1 | high | 外部の人のPRを、エージェントが取り出し、ビルドし、テストしてよいか | メンテナーが差分を読んで許可するまで、取り出しもビルドもテストもしない。エージェントが読むのは、MCPで取り出した差分だけにする。AGENTS.mdの「セキュリティ」に加える |
| J-2 | medium | メンテナー以外の人が書いたPRのコメントとレビューの扱い | `author_association`がOWNERかCOLLABORATORでない人の文は、データとして扱い、要約をメンテナーに知らせるだけにする(返信も修正もしない)。AGENTS.mdとbabysit-prに書き、Auto-fixを使うPRにも当てはめる |
| J-3 | medium | 外部の貢献者のPRのワークフローの承認 | 公開のときに、「Require approval for all external contributors」を選ぶ。承認の前に、`.github/workflows/`の差分を見る |
| J-4 | medium | シークレットのスキャンとプッシュの保護 | 公開の直後に、Secret ProtectionとPush protectionを有効にし、最初のスキャンの結果を確かめる |
| J-5 | medium | `SECURITY.md`と脆弱性の非公開の報告 | エージェントが`SECURITY.md`の案を作り、報告の窓口(非公開の報告)と対象の版をメンテナーが決める。公開の直後に、非公開の報告を有効にする |
| J-6 | medium | `AGENTS.md`、`CLAUDE.md`、`.agents/skills/`を、承認が要るセキュリティ設定と、設定ファイルを守るフックの対象に加えるか | 加える。外部の文から入った「指示」が、規則のファイルに残るのを防ぐため |
| J-7 | medium | セッションのURL(コミットのトレーラー717件、PRの本文77件)の扱い | これからの分は、`attribution.sessionUrl: false`で止める(`.claude/settings.json`の変更)。過去の分は、J-8の移行で消す。それまでに、3つのセッションがPrivateであることを、メンテナーが確かめる。あわせて、アカウントのプランを教えていただく(公開範囲の選択肢がプランで違うため) |
| J-8 | medium | テンプレートの作者のアカウント名(差分の5件)の置き換えの方法 | 2026-11-09の判断の軸のとおり、書き換えた履歴を新しい公開用のリポジトリに移す。同じリポジトリで書き換えても、PRの参照(`refs/pull/*/head`)から古いコミットが辿れるおそれがあるため。`refs/pull`の扱いを公式の文書で確かめる作業を、作業15に登録する |
| J-16 | low | 実機での確認に使ってよい成果物の範囲(いまは、CIの実行のたびの成果物を使ってよい。onboardingの417行) | 外部の人のPRの実行の成果物は、実機での確認に使わない、と手順に書く(手順の変更)。メンテナーとエージェントのPR、手動実行、リリースの成果物は、これまでどおり使ってよい |
| J-9 | low | 過去のActionsのログと成果物の公開 | J-8で新しいリポジトリに移すなら、Actionsの履歴は移らないため、決めなくてよい。今のリポジトリを公開する場合は、公開の前に決める |
| J-10 | low | アクションのSHAでの固定を強制する設定 | Settingsの「Require actions to be pinned to a full-length commit SHA」を有効にし、使えるアクションを、ワークフローが使う3つ(`actions/checkout`、`actions/upload-artifact`、`Swatinem/rust-cache`)に限る。ロックファイル(`gh actions-lock`)は依存が増えるため、いまは採らない。訂正(2026-11-19): はじめは「3つの持ち主に限る」と書いたが、持ち主は2つ(`actions`、`Swatinem`)で、3つはアクションの数だった(`.github/workflows/`の`uses:`の行) |
| J-11 | low | Dependabotのアラートとセキュリティの更新、`rust-toolchain` | アラートだけを有効にし、セキュリティの更新は無効のままにする(公開から7日たっていない版のPRが来るため)。`rust-toolchain`は、いまは加えない |
| J-12 | low | CODEOWNERS | いまは置かない |
| J-13 | low | 変更できないリリースと、ワークフローのコードのスキャン | 変更できないリリース(Settings → General → Releases → 「Enable release immutability」)は、`v0.3.0`の正式なリリースの前に有効にする。公開したリリースのタグは動かせなくなる。CodeQLは、いまは使わない |
| J-14 | low | 日本語を求めるフックと、英語で書く外部の貢献者 | 貢献者向けの文書(`docs/development/onboarding.md`)に、フックが日本語を求めることを書く。フックは変えない |
| J-15 | low | `AGENTS.md`の92行のテンプレート由来の`TODO(project)`のコメント | 消す(UIラベルは英語と、言語の表ですでに決まっているため) |

## エージェントで直せること

承認が要るものは、承認のあとで直す。

| 番号 | 内容 | 承認 |
| --- | --- | --- |
| K-1 | `block-dangerous-git.sh`の正規表現を広げ、`git`のオプション、`refs/heads/`、`+`、`HEAD`以外の送り元(`<ブランチ>:main`)の形の`main`へのプッシュも止める。サンプル入力を`scripts/check_hooks.py`に加える | 要(`.claude/hooks/`の変更) |
| K-2 | `.gitignore`に`/.claude/settings.local.json`を加える | 不要 |
| K-3 | AGENTS.mdの外部由来の例の並びを、「外部の人が書けるもの全般」という原則の文に変え、コミットメッセージ、ブランチ名、PRのタイトル、差分の中のコメント、DependabotのPRの本文を例に足す | 要(規則の変更) |
| K-4 | レビューの手順の文書に、外部の人のPRで`.github/workflows/`が変わっているときは、チェックの成功を検査が通った根拠にしない、と書く | 要(規則の変更) |
| K-6 | `dependabot.yml`が指定するラベル`dependencies`を、リポジトリに作る(GitHubの操作)か、`dependabot.yml`からラベルの指定を外す | 要(どちらもメンテナーの判断。後者はセキュリティ設定の変更) |

## 画面で確かめる項目

エージェントは設定の値を読めないため、次をメンテナーに確かめていただく。

| 番号 | 場所 | 確かめること |
| --- | --- | --- |
| S-1 | GitHubのSettings → Rules → Rulesets | `main`と`release-tags`が「Active」で、対象、ルール、例外がonboardingの9.1と一致すること。必須のステータスチェックの送り元の指定。9.1にないルールセットがないこと |
| S-2 | Settings → Actions → General → 「Actions permissions」 | 許可しているアクションの範囲と、SHAでの固定の強制の状態(J-10) |
| S-3 | Settings → Actions → General → 「Workflow permissions」 | 制限された設定(「Read repository contents and packages permissions」)であること。「Allow GitHub Actions to create and approve pull requests」が無効であること |
| S-4 | Settings → Actions → General | 非公開のいまは「Fork pull request workflows」の各項目。公開したあとは「Approval for running fork pull request workflows from contributors」(J-3) |
| S-5 | Settings → Actions → General | 「Cache settings」と、成果物とログの保存の期間 |
| S-6 | Settings → Secrets and variables(Actions、Dependabot、Codespaces)と、Settings → Environments | シークレット、変数、環境がないこと(ワークフローは参照していない) |
| S-7 | Settings → Security and quality → Advanced Security | 依存グラフ、Dependabotのアラートとセキュリティの更新(J-11)。公開の直後に、Secret Protection、Push protection、Private vulnerability reporting(J-4、J-5) |
| S-8 | Settings → General → 「Releases」 | 「Enable release immutability」(J-13) |
| S-9 | Settings → Collaborators | 所有者のほかに、共同作業者や招待中の人がいないこと |
| S-10 | Actions → Caches | 不要なキャッシュがないこと(汚染を疑ったときに削除する場所。ADR-0017) |
| S-11 | Settings → Integrations → GitHub Apps | ClaudeやCopilotなどのアプリが入っているか。入っていれば、許可しているリポジトリ |
| S-12 | claude.ai/codeのセッションの、各PRのメニューのAuto-fix | 有効か。公開する前に、無効にするか、J-2の規則を先に決める |
| S-13 | claude.aiの組織の管理設定のCode Review | 有効か、いつ動かす設定か(Team・Enterpriseのプランだけの機能。管理設定の正確な場所は確かめられなかった) |
| S-14 | Settings → Moderation options → Interaction limits | 公開の直後に、一時的にコメントを共同作業者に限るか(最長6か月) |
| S-15 | Settings → Webhooks | 外部のサービスにイベントを送る設定がないこと |
| S-16 | ローカルのClaude Code | `~/.claude/settings.json`の`permissions.defaultMode`と、`/hooks`で5つのフックが読み込まれていること |
| S-17 | claude.ai/codeの3つのセッション | 公開範囲がPrivateであること(J-7) |

## 確かめられなかったこと

- 画面で確かめる項目(S-1〜S-17)の値。
- Auto-fixと、PRの通知の購読が、コメントを書いた人の権限で絞るかどうか(文書に記述がなかった)。
- `CLAUDE.md`が、フォルダを信頼する前に読み込まれるかどうか(文書に記述がなかった)。
- Privateのセッションを所有者以外が開いたときに何が見えるか(文書に記述がなかった)。
- 過去のActionsのログの中身。
- 公開の直後のシークレットのスキャンの結果(公開したあとでないと出ない)。
- 72件のPRのレビューコメントを、MCPで1件ずつ確かめること(検索のAPIでは0件)。
- リリースノートの本文(今回の範囲外)。

## 判断

2026-11-19に、メンテナーが、J-1〜J-16と、K-1〜K-4、K-6のすべてを、推奨どおりと判断した([計画0005](../plans/0005-v0.3.0-visuals-and-clock.md)のD-22)。推奨に、メンテナーが決めることや確かめることが含まれる項目は、その部分が残っている(下の表の「残り」)。

| 番号 | 決めた内容 | 反映した場所 | 残り |
| --- | --- | --- | --- |
| J-1 | 外部の人のPRは、メンテナーが差分を読んで許可するまで、取り出し、ビルド、テスト、スクリプトの実行をしない | `AGENTS.md`の「セキュリティ」、`.claude/commands/babysit-pr.md`の手順1、`docs/development/review-process.md`の「外部の人のPR」の行 | なし |
| J-2 | OWNERかCOLLABORATORでない人のコメントとレビューは、データとして扱い、要約をメンテナーに知らせる。PRの通知の購読やAuto-fixで受け取ったものも同じ | `AGENTS.md`の「セキュリティ」と「PR・レビュー・マージ」の手順6、`babysit-pr.md`の手順1と3d、`review-process.md`の「返信と解決」の行 | なし |
| J-3 | 公開のときに「Require approval for all external contributors」を選ぶ。承認の前に、`.github/workflows/`の差分を見る | 計画0005の作業15の2、`review-process.md`の「外部の人のPR」の行 | メンテナーの画面での設定 |
| J-4 | 公開の直後に、Secret ProtectionとPush protectionを有効にし、最初のスキャンの結果を確かめる | 計画0005の作業15の2 | メンテナーの画面での設定 |
| J-5 | エージェントが`SECURITY.md`の案を作る。公開の直後に、非公開の報告を有効にする | ルートの`SECURITY.md` | 非公開の報告の有効化(計画0005の作業15の2)。報告の窓口と対象の版は、2026-11-20に、いまの案のとおりと決めた(計画0005のD-23) |
| J-6 | `AGENTS.md`、`CLAUDE.md`、`.agents/skills/`を、承認が要るセキュリティ設定と、`protect-config.sh`の対象に加える | `AGENTS.md`の「セキュリティ」、`.claude/hooks/protect-config.sh`、`scripts/check_hooks.py`の試験の入力、`docs/development/onboarding.md`の6.3 | なし |
| J-7 | これからの分は`attribution.sessionUrl: false`で止め、コミットメッセージ、PRの本文、GitHubのコメントに、セッションのURLを書かない。過去の分は、J-8の移行で消す | `.claude/settings.json`、`AGENTS.md`の「コミットメッセージ」 | 3つのセッションがPrivateであることのメンテナーの確認(S-17)と、アカウントのプランの答え |
| J-8 | 書き換えた履歴を、新しい公開用のリポジトリに移す | 計画0005の作業15の2 | `refs/pull`の扱いを公式の文書で確かめること、移行の手順の案と、その承認 |
| J-9 | J-8で新しいリポジトリに移すため、決めない | なし | なし |
| J-10 | 「Require actions to be pinned to a full-length commit SHA」を有効にし、使えるアクションを、ワークフローが使う3つのアクションに限る。ロックファイルは使わない | 計画0005の作業15の2 | メンテナーの画面での設定 |
| J-11 | Dependabotのアラートだけを有効にし、セキュリティの更新は無効のままにする。`rust-toolchain`は加えない | 計画0005の作業15の2 | メンテナーの画面での設定 |
| J-12 | CODEOWNERSは置かない | なし | なし |
| J-13 | 変更できないリリースを`v0.3.0`の正式なリリースの前に有効にする。CodeQLは使わない | 計画0005の作業15の2 | メンテナーの画面での設定 |
| J-14 | 日本語を求めるフックは変えず、英語で書く貢献者への説明を書く | `docs/development/onboarding.md`の6.3 | なし |
| J-15 | `AGENTS.md`の`TODO(project)`のコメントを消す | `AGENTS.md`の言語ルールの表のあと | なし |
| J-16 | 外部の人のPRの実行の成果物は、実機での確認に使わない | `docs/development/onboarding.md`の「CIの成果物」の項目 | なし |
| K-1 | `block-dangerous-git.sh`が、`git`のオプション、`refs/heads/`、`+`、`<ブランチ>:main`の形の`main`へのプッシュも止める | `.claude/hooks/block-dangerous-git.sh`、`scripts/check_hooks.py`の試験の入力 | なし |
| K-2 | `.gitignore`に`/.claude/settings.local.json`を加える | `.gitignore` | なし |
| K-3 | 外部由来コンテンツを、外部の人が書けるもの全般という原則の文にし、例を足す | `AGENTS.md`の「セキュリティ」 | なし |
| K-4 | 外部の人のPRで`.github/workflows/`が変わっているときは、チェックの成功を検査が通った根拠にしない | `review-process.md`の「外部の人のPR」の行 | なし |
| K-6 | ラベル`dependencies`をリポジトリに作る(2026-11-20に決定。計画0005のD-23) | メンテナーがリポジトリに作った(D-24) | なし |
