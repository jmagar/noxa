Agent SDK を使用して、コードを読み、バグを見つけ、すべて手動操作なしで修正する AI エージェントを構築します。
**実行内容：**


1. Agent SDK でプロジェクトをセットアップする
2. バグのあるコードを含むファイルを作成する
3. バグを自動的に見つけて修正するエージェントを実行する


## [​](https://code.claude.com/docs/ja/agent-sdk/quickstart#%E5%89%8D%E6%8F%90%E6%9D%A1%E4%BB%B6) 前提条件


- **Node.js 18+** または **Python 3.10+**
- **Anthropic アカウント**（[こちらでサインアップ](https://platform.claude.com/)）


## [​](https://code.claude.com/docs/ja/agent-sdk/quickstart#%E3%82%BB%E3%83%83%E3%83%88%E3%82%A2%E3%83%83%E3%83%97) セットアップ
## ​前提条件
## ​セットアップ
## ​バグのあるファイルを作成する
## ​バグを見つけて修正するエージェントを構築する
## ​主要な概念
## ​トラブルシューティング
## ​次のステップ









1

プロジェクトフォルダを作成する

このクイックスタート用に新しいディレクトリを作成します：

```
mkdir my-agent && cd my-agent
```

独自のプロジェクトの場合、任意のフォルダから SDK を実行できます。デフォルトでは、そのディレクトリとそのサブディレクトリ内のファイルにアクセスできます。 2

SDK をインストールする

お使いの言語用の Agent SDK パッケージをインストールします：

- TypeScript
- Python (uv)
- Python (pip)


```
npm install @anthropic-ai/claude-agent-sdk
```

[uv Python パッケージマネージャー](https://docs.astral.sh/uv/)は、仮想環境を自動的に処理する高速な Python パッケージマネージャーです：

```
uv init && uv add claude-agent-sdk
```

まず仮想環境を作成してからインストールします：

```
python3 -m venv .venv && source .venv/bin/activate
pip3 install claude-agent-sdk
```

TypeScript SDK は、プラットフォーム用のネイティブ Claude Code バイナリをオプションの依存関係としてバンドルしているため、Claude Code を別途インストールする必要はありません。 3

API キーを設定する

[Claude Console](https://platform.claude.com/) から API キーを取得し、プロジェクトディレクトリに `.env` ファイルを作成します：

```
ANTHROPIC_API_KEY=your-api-key
```

SDK は、サードパーティ API プロバイダーを介した認証もサポートしています：

- **Amazon Bedrock**：`CLAUDE_CODE_USE_BEDROCK=1` 環境変数を設定し、AWS 認証情報を構成します
- **Google Vertex AI**：`CLAUDE_CODE_USE_VERTEX=1` 環境変数を設定し、Google Cloud 認証情報を構成します
- **Microsoft Azure**：`CLAUDE_CODE_USE_FOUNDRY=1` 環境変数を設定し、Azure 認証情報を構成します

詳細については、 [Bedrock](https://code.claude.com/docs/ja/amazon-bedrock)、 [Vertex AI](https://code.claude.com/docs/ja/google-vertex-ai)、または [Azure AI Foundry](https://code.claude.com/docs/ja/microsoft-foundry) のセットアップガイドを参照してください。 事前に承認されていない限り、Anthropic は、Claude Agent SDK で構築されたエージェントを含む、サードパーティ開発者が claude.ai ログインまたはレート制限を提供することを許可していません。代わりに、このドキュメントで説明されている API キー認証方法を使用してください。


## [​](https://code.claude.com/docs/ja/agent-sdk/quickstart#%E3%83%90%E3%82%B0%E3%81%AE%E3%81%82%E3%82%8B%E3%83%95%E3%82%A1%E3%82%A4%E3%83%AB%E3%82%92%E4%BD%9C%E6%88%90%E3%81%99%E3%82%8B) バグのあるファイルを作成する


このクイックスタートでは、コード内のバグを見つけて修正できるエージェントを構築する手順を説明します。まず、エージェントが修正するための意図的なバグを含むファイルが必要です。 `my-agent` ディレクトリに `utils.py` を作成し、次のコードを貼り付けます：


```
def calculate_average(numbers):
    total = 0
    for num in numbers:
        total += num
    return total / len(numbers)


def get_user_name(user):
    return user["name"].upper()
```


このコードには 2 つのバグがあります：


1. `calculate_average([])` はゼロで除算してクラッシュします
2. `get_user_name(None)` は TypeError でクラッシュします


## [​](https://code.claude.com/docs/ja/agent-sdk/quickstart#%E3%83%90%E3%82%B0%E3%82%92%E8%A6%8B%E3%81%A4%E3%81%91%E3%81%A6%E4%BF%AE%E6%AD%A3%E3%81%99%E3%82%8B%E3%82%A8%E3%83%BC%E3%82%B8%E3%82%A7%E3%83%B3%E3%83%88%E3%82%92%E6%A7%8B%E7%AF%89%E3%81%99%E3%82%8B) バグを見つけて修正するエージェントを構築する


Python SDK を使用している場合は `agent.py` を作成し、TypeScript の場合は `agent.ts` を作成します：
Python TypeScript

```
import asyncio
from claude_agent_sdk import query, ClaudeAgentOptions, AssistantMessage, ResultMessage


async def main():
    # Agentic ループ：Claude が動作するときにメッセージをストリーミングします
    async for message in query(
        prompt="Review utils.py for bugs that would cause crashes. Fix any issues you find.",
        options=ClaudeAgentOptions(
            allowed_tools=["Read", "Edit", "Glob"],  # Claude が使用できるツール
            permission_mode="acceptEdits",  # ファイル編集を自動承認
        ),
    ):
        # 人間が読める出力を印刷します
        if isinstance(message, AssistantMessage):
            for block in message.content:
                if hasattr(block, "text"):
                    print(block.text)  # Claude の推論
                elif hasattr(block, "name"):
                    print(f"Tool: {block.name}")  # 呼び出されているツール
        elif isinstance(message, ResultMessage):
            print(f"Done: {message.subtype}")  # 最終結果


asyncio.run(main())
```


このコードには 3 つの主要な部分があります：


1. **`query`**：agentic ループを作成するメインエントリーポイント。非同期イテレーターを返すため、 `async for` を使用して Claude が動作するときにメッセージをストリーミングします。完全な API については、 [Python](https://code.claude.com/docs/ja/agent-sdk/python#query) または [TypeScript](https://code.claude.com/docs/ja/agent-sdk/typescript#query) SDK リファレンスを参照してください。
2. **`prompt`**：Claude に実行させたいこと。Claude はタスクに基づいて使用するツールを判断します。
3. **`options`**：エージェントの構成。この例では、 `allowedTools` を使用して `Read`、 `Edit`、 `Glob` を事前承認し、 `permissionMode: "acceptEdits"` を使用してファイル変更を自動承認します。その他のオプションには、 `systemPrompt`、 `mcpServers` などがあります。 [Python](https://code.claude.com/docs/ja/agent-sdk/python#claude-agent-options) または [TypeScript](https://code.claude.com/docs/ja/agent-sdk/typescript#options) のすべてのオプションを参照してください。


`async for` ループは、Claude が考え、ツールを呼び出し、結果を観察し、次に何をするかを決定する間、実行し続けます。各反復は、メッセージを生成します：Claude の推論、ツール呼び出し、ツール結果、または最終的な結果。SDK はオーケストレーション（ツール実行、コンテキスト管理、再試行）を処理するため、ストリームを消費するだけです。Claude がタスクを完了するか、エラーに達するとループが終了します。
ループ内のメッセージ処理は、人間が読める出力をフィルタリングします。フィルタリングなしでは、システム初期化と内部状態を含む生のメッセージオブジェクトが表示されます。これはデバッグに役立ちますが、そうでない場合はノイズが多くなります。
この例はストリーミングを使用してリアルタイムで進行状況を表示します。ライブ出力が不要な場合（バックグラウンドジョブや CI パイプラインなど）、すべてのメッセージを一度に収集できます。詳細については、 [ストリーミング対単一ターンモード](https://code.claude.com/docs/ja/agent-sdk/streaming-vs-single-mode) を参照してください。


### [​](https://code.claude.com/docs/ja/agent-sdk/quickstart#%E3%82%A8%E3%83%BC%E3%82%B8%E3%82%A7%E3%83%B3%E3%83%88%E3%82%92%E5%AE%9F%E8%A1%8C%E3%81%99%E3%82%8B) エージェントを実行する


エージェントの準備ができました。次のコマンドで実行します：


- Python
- TypeScript


```
python3 agent.py
```


```
npx tsx agent.ts
```


実行後、 `utils.py` を確認します。空のリストと null ユーザーを処理する防御的なコードが表示されます。エージェントは自律的に：


1. **読み取り** `utils.py` でコードを理解する
2. **分析** ロジックを分析し、クラッシュを引き起こすエッジケースを特定する
3. **編集** ファイルを編集して適切なエラーハンドリングを追加する


これが Agent SDK を異なるものにする理由です：Claude は、実装するよう求める代わりに、ツールを直接実行します。
「API key not found」が表示される場合は、 `.env` ファイルまたはシェル環境で `ANTHROPIC_API_KEY` 環境変数を設定していることを確認してください。詳細については、 [完全なトラブルシューティングガイド](https://code.claude.com/docs/ja/troubleshooting) を参照してください。


### [​](https://code.claude.com/docs/ja/agent-sdk/quickstart#%E4%BB%96%E3%81%AE%E3%83%97%E3%83%AD%E3%83%B3%E3%83%97%E3%83%88%E3%82%92%E8%A9%A6%E3%81%99) 他のプロンプトを試す


エージェントがセットアップされたので、いくつかの異なるプロンプトを試してください：


- `"Add docstrings to all functions in utils.py"`
- `"Add type hints to all functions in utils.py"`
- `"Create a README.md documenting the functions in utils.py"`


### [​](https://code.claude.com/docs/ja/agent-sdk/quickstart#%E3%82%A8%E3%83%BC%E3%82%B8%E3%82%A7%E3%83%B3%E3%83%88%E3%82%92%E3%82%AB%E3%82%B9%E3%82%BF%E3%83%9E%E3%82%A4%E3%82%BA%E3%81%99%E3%82%8B) エージェントをカスタマイズする


オプションを変更することで、エージェントの動作を変更できます。いくつかの例を次に示します：
**Web 検索機能を追加する：**
Python TypeScript

```
options = ClaudeAgentOptions(
    allowed_tools=["Read", "Edit", "Glob", "WebSearch"], permission_mode="acceptEdits"
)
```


**Claude にカスタムシステムプロンプトを提供する：**
Python TypeScript

```
options = ClaudeAgentOptions(
    allowed_tools=["Read", "Edit", "Glob"],
    permission_mode="acceptEdits",
    system_prompt="You are a senior Python developer. Always follow PEP 8 style guidelines.",
)
```


**ターミナルでコマンドを実行する：**
Python TypeScript

```
options = ClaudeAgentOptions(
    allowed_tools=["Read", "Edit", "Glob", "Bash"], permission_mode="acceptEdits"
)
```


`Bash` を有効にして、次を試してください： `"Write unit tests for utils.py, run them, and fix any failures"`


## [​](https://code.claude.com/docs/ja/agent-sdk/quickstart#%E4%B8%BB%E8%A6%81%E3%81%AA%E6%A6%82%E5%BF%B5) 主要な概念


**ツール** はエージェントが何ができるかを制御します：


| ツール | エージェントが実行できること |
| --- | --- |
| `Read`、 `Glob`、 `Grep` | 読み取り専用分析 |
| `Read`、 `Edit`、 `Glob` | コードの分析と変更 |
| `Read`、 `Edit`、 `Bash`、 `Glob`、 `Grep` | 完全な自動化 |


**権限モード** は、必要な人間の監視の量を制御します：


| モード | 動作 | ユースケース |
| --- | --- | --- |
| `acceptEdits` | ファイル編集と一般的なファイルシステムコマンドを自動承認し、他のアクションについては確認します | 信頼できる開発ワークフロー |
| `dontAsk` | `allowedTools` にないものを拒否します | ロックダウンされたヘッドレスエージェント |
| `auto`（TypeScript のみ） | モデル分類器が各ツール呼び出しを承認または拒否します | 安全ガードレール付きの自律エージェント |
| `bypassPermissions` | プロンプトなしですべてのツールを実行します | サンドボックス化された CI、完全に信頼できる環境 |
| `default` | 承認を処理するために `canUseTool` コールバックが必要です | カスタム承認フロー |


上記の例は `acceptEdits` モードを使用しており、ファイル操作を自動承認するため、エージェントはインタラクティブなプロンプトなしで実行できます。ユーザーに承認を促す場合は、 `default` モードを使用し、ユーザー入力を収集する [`canUseTool` コールバック](https://code.claude.com/docs/ja/agent-sdk/user-input) を提供します。より詳細な制御については、 [権限](https://code.claude.com/docs/ja/agent-sdk/permissions) を参照してください。


## [​](https://code.claude.com/docs/ja/agent-sdk/quickstart#%E3%83%88%E3%83%A9%E3%83%96%E3%83%AB%E3%82%B7%E3%83%A5%E3%83%BC%E3%83%86%E3%82%A3%E3%83%B3%E3%82%B0) トラブルシューティング


### [​](https://code.claude.com/docs/ja/agent-sdk/quickstart#api-%E3%82%A8%E3%83%A9%E3%83%BC-thinking-type-enabled-%E3%81%AF%E3%81%93%E3%81%AE%E3%83%A2%E3%83%87%E3%83%AB%E3%81%A7%E3%81%AF%E3%82%B5%E3%83%9D%E3%83%BC%E3%83%88%E3%81%95%E3%82%8C%E3%81%A6%E3%81%84%E3%81%BE%E3%81%9B%E3%82%93) API エラー `thinking.type.enabled` はこのモデルではサポートされていません


Claude Opus 4.7 は `thinking.type.enabled` を `thinking.type.adaptive` に置き換えます。古い Agent SDK バージョンは、 `claude-opus-4-7` を選択すると次の API エラーで失敗します：


```
API Error: 400 {"type":"invalid_request_error","message":"\"thinking.type.enabled\" is not supported for this model. Use \"thinking.type.adaptive\" and \"output_config.effort\" to control thinking behavior."}
```


Opus 4.7 を使用するには、Agent SDK v0.2.111 以降にアップグレードしてください。


## [​](https://code.claude.com/docs/ja/agent-sdk/quickstart#%E6%AC%A1%E3%81%AE%E3%82%B9%E3%83%86%E3%83%83%E3%83%97) 次のステップ


最初のエージェントを作成したので、その機能を拡張し、ユースケースに合わせてカスタマイズする方法を学びます：


- **[権限](https://code.claude.com/docs/ja/agent-sdk/permissions)**：エージェントが何ができるか、いつ承認が必要かを制御する
- **[Hooks](https://code.claude.com/docs/ja/agent-sdk/hooks)**：ツール呼び出しの前後にカスタムコードを実行する
- **[セッション](https://code.claude.com/docs/ja/agent-sdk/sessions)**：コンテキストを維持するマルチターンエージェントを構築する
- **[MCP サーバー](https://code.claude.com/docs/ja/agent-sdk/mcp)**：データベース、ブラウザー、API、その他の外部システムに接続する
- **[ホスティング](https://code.claude.com/docs/ja/agent-sdk/hosting)**：Docker、クラウド、CI/CD にエージェントをデプロイする
- **[サンプルエージェント](https://github.com/anthropics/claude-agent-sdk-demos)**：完全な例を参照：メールアシスタント、リサーチエージェント、その他[Claude Code Docs home page](https://code.claude.com/docs/ja/overview)

[Privacy choices](https://code.claude.com/docs/ja/agent-sdk/quickstart#)

