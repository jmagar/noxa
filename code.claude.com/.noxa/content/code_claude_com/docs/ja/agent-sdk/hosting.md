# Agent SDK のホスティング
## ​ホスティング要件
## ​SDK アーキテクチャの理解
## ​サンドボックスプロバイダーオプション
## ​本番環境デプロイメントパターン
## ​FAQ
## ​次のステップ







本番環境に Claude Agent SDK をデプロイしてホストする

Claude Agent SDK は従来のステートレス LLM API とは異なり、会話状態を維持し、永続的な環境でコマンドを実行します。このガイドでは、本番環境で SDK ベースのエージェントをデプロイするためのアーキテクチャ、ホスティングに関する考慮事項、およびベストプラクティスについて説明します。
基本的なサンドボックス化を超えたセキュリティ強化（ネットワーク制御、認証情報管理、分離オプションを含む）については、 [セキュアデプロイメント](https://code.claude.com/docs/ja/agent-sdk/secure-deployment)を参照してください。


## [​](https://code.claude.com/docs/ja/agent-sdk/hosting#%E3%83%9B%E3%82%B9%E3%83%86%E3%82%A3%E3%83%B3%E3%82%B0%E8%A6%81%E4%BB%B6) ホスティング要件


### [​](https://code.claude.com/docs/ja/agent-sdk/hosting#%E3%82%B3%E3%83%B3%E3%83%86%E3%83%8A%E3%83%99%E3%83%BC%E3%82%B9%E3%81%AE%E3%82%B5%E3%83%B3%E3%83%89%E3%83%9C%E3%83%83%E3%82%AF%E3%82%B9%E5%8C%96) コンテナベースのサンドボックス化


セキュリティと分離のため、SDK はサンドボックス化されたコンテナ環境内で実行する必要があります。これにより、プロセス分離、リソース制限、ネットワーク制御、および一時的なファイルシステムが提供されます。
SDK は、コマンド実行のための [プログラマティックサンドボックス設定](https://code.claude.com/docs/ja/agent-sdk/typescript#sandbox-settings)もサポートしています。


### [​](https://code.claude.com/docs/ja/agent-sdk/hosting#%E3%82%B7%E3%82%B9%E3%83%86%E3%83%A0%E8%A6%81%E4%BB%B6) システム要件


各 SDK インスタンスには以下が必要です：


- **ランタイム依存関係**
  - Python SDK の場合は Python 3.10 以上、TypeScript SDK の場合は Node.js 18 以上
  - 両方の SDK パッケージには、ホストプラットフォーム用のネイティブ Claude Code バイナリが含まれているため、生成された CLI に対して Claude Code または Node.js の個別インストールは不要です
- **リソース割り当て**
  - 推奨：1GiB RAM、5GiB のディスク、および 1 CPU（タスクに応じて必要に応じて変更してください）
- **ネットワークアクセス**
  - `api.anthropic.com` への送信 HTTPS
  - オプション：MCP サーバーまたは外部ツールへのアクセス


## [​](https://code.claude.com/docs/ja/agent-sdk/hosting#sdk-%E3%82%A2%E3%83%BC%E3%82%AD%E3%83%86%E3%82%AF%E3%83%81%E3%83%A3%E3%81%AE%E7%90%86%E8%A7%A3) SDK アーキテクチャの理解


ステートレス API 呼び出しとは異なり、Claude Agent SDK は以下を行う **長時間実行プロセス**として動作します：


- **永続的なシェル環境でコマンドを実行**
- **作業ディレクトリ内でファイル操作を管理**
- **前の相互作用からのコンテキストでツール実行を処理**


## [​](https://code.claude.com/docs/ja/agent-sdk/hosting#%E3%82%B5%E3%83%B3%E3%83%89%E3%83%9C%E3%83%83%E3%82%AF%E3%82%B9%E3%83%97%E3%83%AD%E3%83%90%E3%82%A4%E3%83%80%E3%83%BC%E3%82%AA%E3%83%97%E3%82%B7%E3%83%A7%E3%83%B3) サンドボックスプロバイダーオプション


AI コード実行用のセキュアなコンテナ環境を専門とするいくつかのプロバイダーがあります：


- **[Modal Sandbox](https://modal.com/docs/guide/sandbox)** - [デモ実装](https://modal.com/docs/examples/claude-slack-gif-creator)
- **[Cloudflare Sandboxes](https://github.com/cloudflare/sandbox-sdk)**
- **[Daytona](https://www.daytona.io/)**
- **[E2B](https://e2b.dev/)**
- **[Fly Machines](https://fly.io/docs/machines/)**
- **[Vercel Sandbox](https://vercel.com/docs/functions/sandbox)**


自己ホスト型オプション（Docker、gVisor、Firecracker）および詳細な分離設定については、 [分離テクノロジー](https://code.claude.com/docs/ja/agent-sdk/secure-deployment#isolation-technologies)を参照してください。


## [​](https://code.claude.com/docs/ja/agent-sdk/hosting#%E6%9C%AC%E7%95%AA%E7%92%B0%E5%A2%83%E3%83%87%E3%83%97%E3%83%AD%E3%82%A4%E3%83%A1%E3%83%B3%E3%83%88%E3%83%91%E3%82%BF%E3%83%BC%E3%83%B3) 本番環境デプロイメントパターン


### [​](https://code.claude.com/docs/ja/agent-sdk/hosting#%E3%83%91%E3%82%BF%E3%83%BC%E3%83%B3-1%EF%BC%9A%E3%82%A8%E3%83%95%E3%82%A7%E3%83%A1%E3%83%A9%E3%83%AB%E3%82%BB%E3%83%83%E3%82%B7%E3%83%A7%E3%83%B3) パターン 1：エフェメラルセッション


各ユーザータスク用に新しいコンテナを作成し、完了時に破棄します。
ワンオフタスクに最適です。ユーザーはタスク完了中も AI と相互作用できますが、完了後はコンテナが破棄されます。
**例：**


- バグ調査と修正：関連するコンテキストを使用して特定の問題をデバッグして解決
- 請求書処理：領収書/請求書からデータを抽出して会計システム用に構造化
- 翻訳タスク：言語間でドキュメントまたはコンテンツバッチを翻訳
- 画像/ビデオ処理：メディアファイルに変換、最適化を適用するか、メタデータを抽出


### [​](https://code.claude.com/docs/ja/agent-sdk/hosting#%E3%83%91%E3%82%BF%E3%83%BC%E3%83%B3-2%EF%BC%9A%E9%95%B7%E6%99%82%E9%96%93%E5%AE%9F%E8%A1%8C%E3%82%BB%E3%83%83%E3%82%B7%E3%83%A7%E3%83%B3) パターン 2：長時間実行セッション


長時間実行タスク用に永続的なコンテナインスタンスを維持します。多くの場合、需要に基づいてコンテナ内で複数の Claude Agent プロセスを実行します。
ユーザー入力なしでアクションを実行するプロアクティブエージェント、コンテンツを提供するエージェント、または大量のメッセージを処理するエージェントに最適です。
**例：**


- メールエージェント：受信メールを監視し、コンテンツに基づいて自律的にトリアージ、応答、またはアクションを実行
- サイトビルダー：ユーザーごとのカスタムウェブサイトをホストし、コンテナポート経由で提供されるライブ編集機能を備えています
- 高頻度チャットボット：Slack などのプラットフォームからの継続的なメッセージストリームを処理し、迅速な応答時間が重要です


### [​](https://code.claude.com/docs/ja/agent-sdk/hosting#%E3%83%91%E3%82%BF%E3%83%BC%E3%83%B3-3%EF%BC%9A%E3%83%8F%E3%82%A4%E3%83%96%E3%83%AA%E3%83%83%E3%83%89%E3%82%BB%E3%83%83%E3%82%B7%E3%83%A7%E3%83%B3) パターン 3：ハイブリッドセッション


履歴と状態で水和されたエフェメラルコンテナ。データベースから、または SDK のセッション再開機能から取得される可能性があります。
ユーザーからの断続的な相互作用があり、作業をキックオフして作業完了時にスピンダウンするが、続行できるコンテナに最適です。
**例：**


- 個人プロジェクトマネージャー：断続的なチェックインで進行中のプロジェクトを管理するのに役立ち、タスク、決定、進捗のコンテキストを維持
- 深い調査：数時間の調査タスクを実施し、調査結果を保存し、ユーザーが戻ったときに調査を再開
- カスタマーサポートエージェント：複数の相互作用にまたがるサポートチケットを処理し、チケット履歴と顧客コンテキストを読み込みます


### [​](https://code.claude.com/docs/ja/agent-sdk/hosting#%E3%83%91%E3%82%BF%E3%83%BC%E3%83%B3-4%EF%BC%9A%E5%8D%98%E4%B8%80%E3%82%B3%E3%83%B3%E3%83%86%E3%83%8A) パターン 4：単一コンテナ


1 つのグローバルコンテナで複数の Claude Agent SDK プロセスを実行します。
密接に協力する必要があるエージェントに最適です。これはおそらく最も人気のないパターンです。エージェントが互いに上書きするのを防ぐ必要があるためです。
**例：**


- **シミュレーション**：ビデオゲームなどのシミュレーション内で相互作用するエージェント。


## [​](https://code.claude.com/docs/ja/agent-sdk/hosting#faq) FAQ


### [​](https://code.claude.com/docs/ja/agent-sdk/hosting#%E3%82%B5%E3%83%B3%E3%83%89%E3%83%9C%E3%83%83%E3%82%AF%E3%82%B9%E3%81%A8%E9%80%9A%E4%BF%A1%E3%81%99%E3%82%8B%E3%81%AB%E3%81%AF%E3%81%A9%E3%81%86%E3%81%99%E3%82%8C%E3%81%B0%E3%82%88%E3%81%84%E3%81%A7%E3%81%99%E3%81%8B%EF%BC%9F) サンドボックスと通信するにはどうすればよいですか？


コンテナでホストする場合、SDK インスタンスと通信するためにポートを公開します。アプリケーションは外部クライアント用に HTTP/WebSocket エンドポイントを公開できますが、SDK はコンテナ内で内部的に実行されます。


### [​](https://code.claude.com/docs/ja/agent-sdk/hosting#%E3%82%B3%E3%83%B3%E3%83%86%E3%83%8A%E3%82%92%E3%83%9B%E3%82%B9%E3%83%88%E3%81%99%E3%82%8B%E3%82%B3%E3%82%B9%E3%83%88%E3%81%AF%E3%81%84%E3%81%8F%E3%82%89%E3%81%A7%E3%81%99%E3%81%8B%EF%BC%9F) コンテナをホストするコストはいくらですか？


エージェントを提供する場合の主なコストはトークンです。コンテナはプロビジョニング内容に基づいて異なりますが、最小コストは実行時間あたり約 5 セントです。


### [​](https://code.claude.com/docs/ja/agent-sdk/hosting#%E3%82%A2%E3%82%A4%E3%83%89%E3%83%AB%E3%82%B3%E3%83%B3%E3%83%86%E3%83%8A%E3%82%92%E3%82%B7%E3%83%A3%E3%83%83%E3%83%88%E3%83%80%E3%82%A6%E3%83%B3%E3%81%99%E3%82%8B%E3%81%B9%E3%81%8D%E3%81%8B%E3%80%81%E3%81%9D%E3%82%8C%E3%81%A8%E3%82%82%E6%B8%A9%E3%81%8B%E3%81%8F%E4%BF%9D%E3%81%A4%E3%81%B9%E3%81%8D%E3%81%8B%EF%BC%9F) アイドルコンテナをシャットダウンするべきか、それとも温かく保つべきか？


これはおそらくプロバイダーに依存します。異なるサンドボックスプロバイダーは、アイドルタイムアウト後にサンドボックスがスピンダウンする可能性がある異なる基準を設定できます。
ユーザーの応答がどのくらい頻繁に発生すると思われるかに基づいて、このタイムアウトを調整する必要があります。


### [​](https://code.claude.com/docs/ja/agent-sdk/hosting#claude-code-cli-%E3%81%AF%E3%81%A9%E3%81%AE%E3%81%8F%E3%82%89%E3%81%84%E3%81%AE%E9%A0%BB%E5%BA%A6%E3%81%A7%E6%9B%B4%E6%96%B0%E3%81%99%E3%82%8B%E5%BF%85%E8%A6%81%E3%81%8C%E3%81%82%E3%82%8A%E3%81%BE%E3%81%99%E3%81%8B%EF%BC%9F) Claude Code CLI はどのくらいの頻度で更新する必要がありますか？


Claude Code CLI は semver でバージョン管理されているため、破壊的な変更はバージョン管理されます。


### [​](https://code.claude.com/docs/ja/agent-sdk/hosting#%E3%82%B3%E3%83%B3%E3%83%86%E3%83%8A%E3%81%AE%E5%81%A5%E5%85%A8%E6%80%A7%E3%81%A8%E3%82%A8%E3%83%BC%E3%82%B8%E3%82%A7%E3%83%B3%E3%83%88%E3%81%AE%E3%83%91%E3%83%95%E3%82%A9%E3%83%BC%E3%83%9E%E3%83%B3%E3%82%B9%E3%82%92%E7%9B%A3%E8%A6%96%E3%81%99%E3%82%8B%E3%81%AB%E3%81%AF%E3%81%A9%E3%81%86%E3%81%99%E3%82%8C%E3%81%B0%E3%82%88%E3%81%84%E3%81%A7%E3%81%99%E3%81%8B%EF%BC%9F) コンテナの健全性とエージェントのパフォーマンスを監視するにはどうすればよいですか？


コンテナはサーバーであるため、バックエンド用に使用するのと同じログインフラストラクチャがコンテナで機能します。


### [​](https://code.claude.com/docs/ja/agent-sdk/hosting#%E3%82%A8%E3%83%BC%E3%82%B8%E3%82%A7%E3%83%B3%E3%83%88%E3%82%BB%E3%83%83%E3%82%B7%E3%83%A7%E3%83%B3%E3%81%AF%E3%82%BF%E3%82%A4%E3%83%A0%E3%82%A2%E3%82%A6%E3%83%88%E3%81%99%E3%82%8B%E5%89%8D%E3%81%AB%E3%81%A9%E3%81%AE%E3%81%8F%E3%82%89%E3%81%84%E5%AE%9F%E8%A1%8C%E3%81%A7%E3%81%8D%E3%81%BE%E3%81%99%E3%81%8B%EF%BC%9F) エージェントセッションはタイムアウトする前にどのくらい実行できますか？


エージェントセッションはタイムアウトしませんが、Claude がループに陥るのを防ぐために「maxTurns」プロパティを設定することを検討してください。


## [​](https://code.claude.com/docs/ja/agent-sdk/hosting#%E6%AC%A1%E3%81%AE%E3%82%B9%E3%83%86%E3%83%83%E3%83%97) 次のステップ


- [セキュアデプロイメント](https://code.claude.com/docs/ja/agent-sdk/secure-deployment) - ネットワーク制御、認証情報管理、および分離強化
- [TypeScript SDK - サンドボックス設定](https://code.claude.com/docs/ja/agent-sdk/typescript#sandbox-settings) - プログラマティックにサンドボックスを設定
- [セッションガイド](https://code.claude.com/docs/ja/agent-sdk/sessions) - セッション管理について学習
- [権限](https://code.claude.com/docs/ja/agent-sdk/permissions) - ツール権限を設定
- [コスト追跡](https://code.claude.com/docs/ja/agent-sdk/cost-tracking) - API 使用状況を監視
- [MCP 統合](https://code.claude.com/docs/ja/agent-sdk/mcp) - カスタムツールで拡張[Claude Code Docs home page](https://code.claude.com/docs/ja/overview)

[Privacy choices](https://code.claude.com/docs/ja/agent-sdk/hosting#)

