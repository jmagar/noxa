# Agent SDK の概要
## ​はじめに
## ​機能
## ​Agent SDK と他の Claude ツールを比較します
## ​変更ログ
## ​バグの報告
## ​ブランドガイドライン
## ​ライセンスと利用規約
## ​次のステップ









Claude Code をライブラリとして使用して、本番環境対応の AI エージェントを構築します

Claude Code SDK は Claude Agent SDK に名前が変更されました。古い SDK から移行する場合は、 [移行ガイド](https://code.claude.com/docs/ja/agent-sdk/migration-guide)を参照してください。
ファイルを自動的に読み取り、コマンドを実行し、ウェブを検索し、コードを編集するなど、さらに多くのことができる AI エージェントを構築します。Agent SDK は、Claude Code を強化する同じツール、エージェントループ、およびコンテキスト管理を提供し、Python と TypeScript でプログラム可能です。
Opus 4.7（ `claude-opus-4-7`）には Agent SDK v0.2.111 以降が必要です。 `thinking.type.enabled` API エラーが表示される場合は、 [トラブルシューティング](https://code.claude.com/docs/ja/agent-sdk/quickstart#troubleshooting)を参照してください。
Python TypeScript

```
import asyncio
from claude_agent_sdk import query, ClaudeAgentOptions


async def main():
    async for message in query(
        prompt="Find and fix the bug in auth.py",
        options=ClaudeAgentOptions(allowed_tools=["Read", "Edit", "Bash"]),
    ):
        print(message)  # Claude reads the file, finds the bug, edits it


asyncio.run(main())
```


Agent SDK には、ファイルの読み取り、コマンドの実行、コードの編集用の組み込みツールが含まれているため、ツール実行を実装することなく、エージェントはすぐに動作を開始できます。クイックスタートに進むか、SDK で構築された実際のエージェントを探索してください。


## クイックスタート

数分でバグ修正エージェントを構築します

## エージェントの例

メールアシスタント、リサーチエージェント、その他


## [​](https://code.claude.com/docs/ja/agent-sdk/overview#%E3%81%AF%E3%81%98%E3%82%81%E3%81%AB) はじめに


1

SDK をインストールします


- TypeScript
- Python


```
npm install @anthropic-ai/claude-agent-sdk
```


```
pip install claude-agent-sdk
```

TypeScript SDK は、プラットフォーム用のネイティブ Claude Code バイナリをオプションの依存関係としてバンドルしているため、Claude Code を別途インストールする必要はありません。 2

API キーを設定します

[Console](https://platform.claude.com/) から API キーを取得し、環境変数として設定します。

```
export ANTHROPIC_API_KEY=your-api-key
```

SDK はサードパーティ API プロバイダーを介した認証もサポートしています。

- **Amazon Bedrock**: `CLAUDE_CODE_USE_BEDROCK=1` 環境変数を設定し、AWS 認証情報を構成します
- **Google Vertex AI**: `CLAUDE_CODE_USE_VERTEX=1` 環境変数を設定し、Google Cloud 認証情報を構成します
- **Microsoft Azure**: `CLAUDE_CODE_USE_FOUNDRY=1` 環境変数を設定し、Azure 認証情報を構成します

詳細については、 [Bedrock](https://code.claude.com/docs/ja/amazon-bedrock)、 [Vertex AI](https://code.claude.com/docs/ja/google-vertex-ai)、または [Azure AI Foundry](https://code.claude.com/docs/ja/microsoft-foundry) のセットアップガイドを参照してください。 事前に承認されていない限り、Anthropic は、Claude Agent SDK で構築されたエージェントを含む、サードパーティ開発者が claude.ai ログインまたはレート制限を提供することを許可していません。代わりに、このドキュメントで説明されている API キー認証方法を使用してください。 3

最初のエージェントを実行します

この例は、組み込みツールを使用して現在のディレクトリ内のファイルをリストするエージェントを作成します。 Python TypeScript

```
import asyncio
from claude_agent_sdk import query, ClaudeAgentOptions


async def main():
    async for message in query(
        prompt="What files are in this directory?",
        options=ClaudeAgentOptions(allowed_tools=["Bash", "Glob"]),
    ):
        if hasattr(message, "result"):
            print(message.result)


asyncio.run(main())
```


**構築する準備はできていますか？** [クイックスタート](https://code.claude.com/docs/ja/agent-sdk/quickstart)に従って、数分でバグを見つけて修正するエージェントを作成します。


## [​](https://code.claude.com/docs/ja/agent-sdk/overview#%E6%A9%9F%E8%83%BD) 機能


Claude Code を強力にするすべてのものが SDK で利用可能です。


- 組み込みツール
- Hooks
- サブエージェント
- MCP
- 権限
- セッション

エージェントは、ファイルの読み取り、コマンドの実行、コードベースの検索をすぐに実行できます。主要なツールは次のとおりです。

| ツール | 機能 |
| --- | --- |
| **Read** | 作業ディレクトリ内の任意のファイルを読み取ります |
| **Write** | 新しいファイルを作成します |
| **Edit** | 既存ファイルに正確な編集を加えます |
| **Bash** | ターミナルコマンド、スクリプト、git 操作を実行します |
| **Monitor** | バックグラウンドスクリプトを監視し、各出力行をイベントとして反応します |
| **Glob** | パターン（ `**/*.ts`、 `src/**/*.py`）でファイルを検索します |
| **Grep** | 正規表現でファイルコンテンツを検索します |
| **WebSearch** | 現在の情報をウェブで検索します |
| **WebFetch** | ウェブページコンテンツを取得して解析します |
| **[AskUserQuestion](https://code.claude.com/docs/ja/agent-sdk/user-input#handle-clarifying-questions)** | 複数選択オプション付きで、ユーザーに明確化の質問をします |

この例は、コードベースで TODO コメントを検索するエージェントを作成します。 Python TypeScript

```
import asyncio
from claude_agent_sdk import query, ClaudeAgentOptions


async def main():
    async for message in query(
        prompt="Find all TODO comments and create a summary",
        options=ClaudeAgentOptions(allowed_tools=["Read", "Glob", "Grep"]),
    ):
        if hasattr(message, "result"):
            print(message.result)


asyncio.run(main())
```

エージェントライフサイクルの重要なポイントでカスタムコードを実行します。SDK hooks はコールバック関数を使用して、エージェントの動作を検証、ログ、ブロック、または変換します。 **利用可能な hooks:** `PreToolUse`、 `PostToolUse`、 `Stop`、 `SessionStart`、 `SessionEnd`、 `UserPromptSubmit` など。 この例は、すべてのファイル変更を監査ファイルにログします。 Python TypeScript

```
import asyncio
from datetime import datetime
from claude_agent_sdk import query, ClaudeAgentOptions, HookMatcher


async def log_file_change(input_data, tool_use_id, context):
    file_path = input_data.get("tool_input", {}).get("file_path", "unknown")
    with open("./audit.log", "a") as f:
        f.write(f"{datetime.now()}: modified {file_path}\n")
    return {}


async def main():
    async for message in query(
        prompt="Refactor utils.py to improve readability",
        options=ClaudeAgentOptions(
            permission_mode="acceptEdits",
            hooks={
                "PostToolUse": [
                    HookMatcher(matcher="Edit|Write", hooks=[log_file_change])
                ]
            },
        ),
    ):
        if hasattr(message, "result"):
            print(message.result)


asyncio.run(main())
```

[hooks の詳細を学ぶ →](https://code.claude.com/docs/ja/agent-sdk/hooks) 特定のサブタスクを処理するために特化したエージェントを生成します。メインエージェントが作業を委譲し、サブエージェントが結果を報告します。 特化した指示を持つカスタムエージェントを定義します。サブエージェントは Agent ツール経由で呼び出されるため、 `allowedTools` に `Agent` を含めます。 Python TypeScript

```
import asyncio
from claude_agent_sdk import query, ClaudeAgentOptions, AgentDefinition


async def main():
    async for message in query(
        prompt="Use the code-reviewer agent to review this codebase",
        options=ClaudeAgentOptions(
            allowed_tools=["Read", "Glob", "Grep", "Agent"],
            agents={
                "code-reviewer": AgentDefinition(
                    description="Expert code reviewer for quality and security reviews.",
                    prompt="Analyze code quality and suggest improvements.",
                    tools=["Read", "Glob", "Grep"],
                )
            },
        ),
    ):
        if hasattr(message, "result"):
            print(message.result)


asyncio.run(main())
```

サブエージェントのコンテキスト内からのメッセージには `parent_tool_use_id` フィールドが含まれており、どのメッセージがどのサブエージェント実行に属しているかを追跡できます。 [サブエージェントの詳細を学ぶ →](https://code.claude.com/docs/ja/agent-sdk/subagents) Model Context Protocol を介して外部システムに接続します。データベース、ブラウザ、API、および [数百以上](https://github.com/modelcontextprotocol/servers)。 この例は、 [Playwright MCP サーバー](https://github.com/microsoft/playwright-mcp)を接続して、エージェントにブラウザ自動化機能を提供します。 Python TypeScript

```
import asyncio
from claude_agent_sdk import query, ClaudeAgentOptions


async def main():
    async for message in query(
        prompt="Open example.com and describe what you see",
        options=ClaudeAgentOptions(
            mcp_servers={
                "playwright": {"command": "npx", "args": ["@playwright/mcp@latest"]}
            }
        ),
    ):
        if hasattr(message, "result"):
            print(message.result)


asyncio.run(main())
```

[MCP の詳細を学ぶ →](https://code.claude.com/docs/ja/agent-sdk/mcp) エージェントが使用できるツールを正確に制御します。安全な操作を許可し、危険な操作をブロックするか、機密アクションの承認を要求します。 対話的な承認プロンプトと `AskUserQuestion` ツールについては、 [承認とユーザー入力の処理](https://code.claude.com/docs/ja/agent-sdk/user-input)を参照してください。 この例は、コードを分析できるが変更できない読み取り専用エージェントを作成します。 `allowed_tools` は `Read`、 `Glob`、および `Grep` を事前承認します。 Python TypeScript

```
import asyncio
from claude_agent_sdk import query, ClaudeAgentOptions


async def main():
    async for message in query(
        prompt="Review this code for best practices",
        options=ClaudeAgentOptions(
            allowed_tools=["Read", "Glob", "Grep"],
        ),
    ):
        if hasattr(message, "result"):
            print(message.result)


asyncio.run(main())
```

[権限の詳細を学ぶ →](https://code.claude.com/docs/ja/agent-sdk/permissions) 複数の交換にわたってコンテキストを維持します。Claude は読み取ったファイル、実行した分析、および会話履歴を記憶します。後でセッションを再開するか、異なるアプローチを探索するためにフォークします。 この例は、最初のクエリからセッション ID をキャプチャし、その後、完全なコンテキストで続行するために再開します。 Python TypeScript

```
import asyncio
from claude_agent_sdk import query, ClaudeAgentOptions, SystemMessage, ResultMessage


async def main():
    session_id = None

    # First query: capture the session ID
    async for message in query(
        prompt="Read the authentication module",
        options=ClaudeAgentOptions(allowed_tools=["Read", "Glob"]),
    ):
        if isinstance(message, SystemMessage) and message.subtype == "init":
            session_id = message.data["session_id"]

    # Resume with full context from the first query
    async for message in query(
        prompt="Now find all places that call it",  # "it" = auth module
        options=ClaudeAgentOptions(resume=session_id),
    ):
        if isinstance(message, ResultMessage):
            print(message.result)


asyncio.run(main())
```

[セッションの詳細を学ぶ →](https://code.claude.com/docs/ja/agent-sdk/sessions)


### [​](https://code.claude.com/docs/ja/agent-sdk/overview#claude-code-%E3%81%AE%E6%A9%9F%E8%83%BD) Claude Code の機能


SDK はまた Claude Code のファイルシステムベースの構成をサポートしています。デフォルトオプションでは、SDK は作業ディレクトリの `.claude/` と `~/.claude/` からこれらを読み込みます。どのソースを読み込むかを制限するには、オプションで `setting_sources`（Python）または `settingSources`（TypeScript）を設定します。


| 機能 | 説明 | 場所 |
| --- | --- | --- |
| [Skills](https://code.claude.com/docs/ja/agent-sdk/skills) | Markdown で定義された特化した機能 | `.claude/skills/*/SKILL.md` |
| [Slash commands](https://code.claude.com/docs/ja/agent-sdk/slash-commands) | 一般的なタスク用のカスタムコマンド | `.claude/commands/*.md` |
| [Memory](https://code.claude.com/docs/ja/agent-sdk/modifying-system-prompts) | プロジェクトコンテキストと指示 | `CLAUDE.md` または `.claude/CLAUDE.md` |
| [Plugins](https://code.claude.com/docs/ja/agent-sdk/plugins) | カスタムコマンド、エージェント、MCP サーバーで拡張 | `plugins` オプション経由でプログラム的に |


## [​](https://code.claude.com/docs/ja/agent-sdk/overview#agent-sdk-%E3%81%A8%E4%BB%96%E3%81%AE-claude-%E3%83%84%E3%83%BC%E3%83%AB%E3%82%92%E6%AF%94%E8%BC%83%E3%81%97%E3%81%BE%E3%81%99) Agent SDK と他の Claude ツールを比較します


Claude Platform は Claude で構築するための複数の方法を提供しています。Agent SDK がどのように適合するかは次のとおりです。


- Agent SDK vs Client SDK
- Agent SDK vs Claude Code CLI

[Anthropic Client SDK](https://platform.claude.com/docs/en/api/client-sdks) は直接 API アクセスを提供します。プロンプトを送信し、ツール実行を自分で実装します。 **Agent SDK** は、組み込みツール実行を備えた Claude を提供します。 Client SDK では、ツールループを実装します。Agent SDK では、Claude がそれを処理します。 Python TypeScript

```
# Client SDK: You implement the tool loop
response = client.messages.create(...)
while response.stop_reason == "tool_use":
    result = your_tool_executor(response.tool_use)
    response = client.messages.create(tool_result=result, **params)

# Agent SDK: Claude handles tools autonomously
async for message in query(prompt="Fix the bug in auth.py"):
    print(message)
```

同じ機能、異なるインターフェース。

| ユースケース | 最適な選択 |
| --- | --- |
| インタラクティブな開発 | CLI |
| CI/CD パイプライン | SDK |
| カスタムアプリケーション | SDK |
| 1 回限りのタスク | CLI |
| 本番環境の自動化 | SDK |

多くのチームは両方を使用しています。日常的な開発には CLI、本番環境には SDK を使用します。ワークフローはそれらの間で直接変換されます。


## [​](https://code.claude.com/docs/ja/agent-sdk/overview#%E5%A4%89%E6%9B%B4%E3%83%AD%E3%82%B0) 変更ログ


SDK の更新、バグ修正、および新機能の完全な変更ログを表示します。


- **TypeScript SDK**: [CHANGELOG.md を表示](https://github.com/anthropics/claude-agent-sdk-typescript/blob/main/CHANGELOG.md)
- **Python SDK**: [CHANGELOG.md を表示](https://github.com/anthropics/claude-agent-sdk-python/blob/main/CHANGELOG.md)


## [​](https://code.claude.com/docs/ja/agent-sdk/overview#%E3%83%90%E3%82%B0%E3%81%AE%E5%A0%B1%E5%91%8A) バグの報告


Agent SDK でバグまたは問題が発生した場合。


- **TypeScript SDK**: [GitHub で問題を報告](https://github.com/anthropics/claude-agent-sdk-typescript/issues)
- **Python SDK**: [GitHub で問題を報告](https://github.com/anthropics/claude-agent-sdk-python/issues)


## [​](https://code.claude.com/docs/ja/agent-sdk/overview#%E3%83%96%E3%83%A9%E3%83%B3%E3%83%89%E3%82%AC%E3%82%A4%E3%83%89%E3%83%A9%E3%82%A4%E3%83%B3) ブランドガイドライン


Claude Agent SDK を統合するパートナーの場合、Claude ブランドの使用はオプションです。製品で Claude を参照する場合。
**許可されています:**


- ‘Claude Agent’（ドロップダウンメニューに推奨）
- ‘Claude’（既に’Agents’というラベルが付いたメニュー内の場合）
- ’ Powered by Claude’（既存のエージェント名がある場合）


**許可されていません:**


- ‘Claude Code’または’Claude Code Agent’
- Claude Code ブランドの ASCII アートまたは Claude Code を模倣する視覚要素


製品は独自のブランドを維持し、Claude Code または任意の Anthropic 製品のように見えるべきではありません。ブランドコンプライアンスに関する質問については、Anthropic [営業チーム](https://www.anthropic.com/contact-sales)に連絡してください。


## [​](https://code.claude.com/docs/ja/agent-sdk/overview#%E3%83%A9%E3%82%A4%E3%82%BB%E3%83%B3%E3%82%B9%E3%81%A8%E5%88%A9%E7%94%A8%E8%A6%8F%E7%B4%84) ライセンスと利用規約


Claude Agent SDK の使用は、 [Anthropic の商用利用規約](https://www.anthropic.com/legal/commercial-terms)によって管理されます。これは、Claude Agent SDK を使用して、独自のカスタマーおよびエンドユーザーに利用可能にする製品およびサービスを強化する場合を含みます。ただし、特定のコンポーネントまたは依存関係が、そのコンポーネントの LICENSE ファイルに示されているように異なるライセンスの対象である場合を除きます。


## [​](https://code.claude.com/docs/ja/agent-sdk/overview#%E6%AC%A1%E3%81%AE%E3%82%B9%E3%83%86%E3%83%83%E3%83%97) 次のステップ


## クイックスタート

数分でバグを見つけて修正するエージェントを構築します

## エージェントの例

メールアシスタント、リサーチエージェント、その他

## TypeScript SDK

完全な TypeScript API リファレンスと例

## Python SDK

完全な Python API リファレンスと例[Claude Code Docs home page](https://code.claude.com/docs/ja/overview)

[Privacy choices](https://code.claude.com/docs/ja/agent-sdk/overview#)

