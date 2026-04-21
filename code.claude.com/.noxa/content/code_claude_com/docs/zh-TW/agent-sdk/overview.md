# Agent SDK 概述
## ​開始使用
## ​功能
## ​將 Agent SDK 與其他 Claude 工具進行比較
## ​變更日誌
## ​報告錯誤
## ​品牌指南
## ​許可證和條款
## ​後續步驟









使用 Claude Code 作為程式庫構建生產級 AI 代理

Claude Code SDK 已重新命名為 Claude Agent SDK。如果您正在從舊 SDK 遷移，請參閱 [遷移指南](https://code.claude.com/docs/zh-TW/agent-sdk/migration-guide)。
構建能夠自主讀取檔案、執行命令、搜尋網路、編輯程式碼等的 AI 代理。Agent SDK 提供與 Claude Code 相同的工具、代理迴圈和上下文管理，可在 Python 和 TypeScript 中進行程式設計。
Opus 4.7 ( `claude-opus-4-7`) 需要 Agent SDK v0.2.111 或更高版本。如果您看到 `thinking.type.enabled` API 錯誤，請參閱 [故障排除](https://code.claude.com/docs/zh-TW/agent-sdk/quickstart#troubleshooting)。
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


Agent SDK 包含用於讀取檔案、執行命令和編輯程式碼的內建工具，因此您的代理可以立即開始工作，無需您實現工具執行。深入了解快速入門或探索使用 SDK 構建的真實代理：


## 快速入門

在幾分鐘內構建一個除錯代理

## 範例代理

電子郵件助手、研究代理等


## [​](https://code.claude.com/docs/zh-TW/agent-sdk/overview#%E9%96%8B%E5%A7%8B%E4%BD%BF%E7%94%A8) 開始使用


1

安裝 SDK


- TypeScript
- Python


```
npm install @anthropic-ai/claude-agent-sdk
```


```
pip install claude-agent-sdk
```

TypeScript SDK 為您的平台捆綁了原生 Claude Code 二進位檔案作為可選依賴項，因此您無需單獨安裝 Claude Code。 2

設定您的 API 金鑰

從 [主控台](https://platform.claude.com/)取得 API 金鑰，然後將其設定為環境變數：

```
export ANTHROPIC_API_KEY=your-api-key
```

SDK 也支援透過第三方 API 提供者進行身份驗證：

- **Amazon Bedrock**：設定 `CLAUDE_CODE_USE_BEDROCK=1` 環境變數並配置 AWS 認證
- **Google Vertex AI**：設定 `CLAUDE_CODE_USE_VERTEX=1` 環境變數並配置 Google Cloud 認證
- **Microsoft Azure**：設定 `CLAUDE_CODE_USE_FOUNDRY=1` 環境變數並配置 Azure 認證

有關詳細資訊，請參閱 [Bedrock](https://code.claude.com/docs/zh-TW/amazon-bedrock)、 [Vertex AI](https://code.claude.com/docs/zh-TW/google-vertex-ai) 或 [Azure AI Foundry](https://code.claude.com/docs/zh-TW/microsoft-foundry) 的設定指南。 除非事先獲得批准，否則 Anthropic 不允許第三方開發人員為其產品（包括基於 Claude Agent SDK 構建的代理）提供 claude.ai 登入或速率限制。請改用本文件中描述的 API 金鑰身份驗證方法。 3

執行您的第一個代理

此範例建立一個使用內建工具列出目前目錄中檔案的代理。 Python TypeScript

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


**準備好構建了嗎？** 遵循 [快速入門](https://code.claude.com/docs/zh-TW/agent-sdk/quickstart)在幾分鐘內建立一個尋找和修復錯誤的代理。


## [​](https://code.claude.com/docs/zh-TW/agent-sdk/overview#%E5%8A%9F%E8%83%BD) 功能


使 Claude Code 強大的一切都可在 SDK 中使用：


- 內建工具
- Hooks
- 子代理
- MCP
- 權限
- 工作階段

您的代理可以開箱即用地讀取檔案、執行命令和搜尋程式碼庫。主要工具包括：

| 工具 | 功能 |
| --- | --- |
| **Read** | 讀取工作目錄中的任何檔案 |
| **Write** | 建立新檔案 |
| **Edit** | 對現有檔案進行精確編輯 |
| **Bash** | 執行終端命令、指令碼、git 操作 |
| **Monitor** | 監視背景指令碼並對每個輸出行作為事件做出反應 |
| **Glob** | 按模式尋找檔案（ `**/*.ts`、 `src/**/*.py`） |
| **Grep** | 使用正規表達式搜尋檔案內容 |
| **WebSearch** | 搜尋網路以獲取最新資訊 |
| **WebFetch** | 擷取並解析網頁內容 |
| **[AskUserQuestion](https://code.claude.com/docs/zh-TW/agent-sdk/user-input#handle-clarifying-questions)** | 向使用者提出具有多選選項的澄清問題 |

此範例建立一個搜尋程式碼庫中 TODO 註解的代理： Python TypeScript

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

在代理生命週期的關鍵點執行自訂程式碼。SDK hooks 使用回呼函式來驗證、記錄、阻止或轉換代理行為。 **可用 hooks：** `PreToolUse`、 `PostToolUse`、 `Stop`、 `SessionStart`、 `SessionEnd`、 `UserPromptSubmit` 等。 此範例將所有檔案變更記錄到稽核檔案： Python TypeScript

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

[深入了解 hooks →](https://code.claude.com/docs/zh-TW/agent-sdk/hooks) 生成專門的代理來處理集中的子任務。您的主代理委派工作，子代理報告結果。 定義具有專門指令的自訂代理。在 `allowedTools` 中包含 `Agent`，因為子代理透過 Agent 工具呼叫： Python TypeScript

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

來自子代理上下文內的訊息包含 `parent_tool_use_id` 欄位，讓您追蹤哪些訊息屬於哪個子代理執行。 [深入了解子代理 →](https://code.claude.com/docs/zh-TW/agent-sdk/subagents) 透過 Model Context Protocol 連接到外部系統：資料庫、瀏覽器、API 和 [數百個更多](https://github.com/modelcontextprotocol/servers)。 此範例連接 [Playwright MCP 伺服器](https://github.com/microsoft/playwright-mcp)以為您的代理提供瀏覽器自動化功能： Python TypeScript

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

[深入了解 MCP →](https://code.claude.com/docs/zh-TW/agent-sdk/mcp) 精確控制您的代理可以使用哪些工具。允許安全操作、阻止危險操作或要求對敏感操作進行批准。 有關互動式批准提示和 `AskUserQuestion` 工具，請參閱 [處理批准和使用者輸入](https://code.claude.com/docs/zh-TW/agent-sdk/user-input)。 此範例建立一個唯讀代理，可以分析但不能修改程式碼。 `allowed_tools` 預先批准 `Read`、 `Glob` 和 `Grep`。 Python TypeScript

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

[深入了解權限 →](https://code.claude.com/docs/zh-TW/agent-sdk/permissions) 在多次交換中保持上下文。Claude 記住讀取的檔案、完成的分析和對話歷史。稍後恢復工作階段，或分叉它們以探索不同的方法。 此範例從第一個查詢中擷取工作階段 ID，然後恢復以繼續進行完整上下文： Python TypeScript

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

[深入了解工作階段 →](https://code.claude.com/docs/zh-TW/agent-sdk/sessions)


### [​](https://code.claude.com/docs/zh-TW/agent-sdk/overview#claude-code-%E5%8A%9F%E8%83%BD) Claude Code 功能


SDK 也支援 Claude Code 的基於檔案系統的配置。使用預設選項，SDK 從工作目錄中的 `.claude/` 和 `~/.claude/` 載入這些。要限制載入哪些來源，請在選項中設定 `setting_sources`（Python）或 `settingSources`（TypeScript）。


| 功能 | 描述 | 位置 |
| --- | --- | --- |
| [Skills](https://code.claude.com/docs/zh-TW/agent-sdk/skills) | 在 Markdown 中定義的專門功能 | `.claude/skills/*/SKILL.md` |
| [Slash commands](https://code.claude.com/docs/zh-TW/agent-sdk/slash-commands) | 用於常見任務的自訂命令 | `.claude/commands/*.md` |
| [Memory](https://code.claude.com/docs/zh-TW/agent-sdk/modifying-system-prompts) | 專案上下文和指令 | `CLAUDE.md` 或 `.claude/CLAUDE.md` |
| [Plugins](https://code.claude.com/docs/zh-TW/agent-sdk/plugins) | 使用自訂命令、代理和 MCP 伺服器進行擴展 | 透過 `plugins` 選項進行程式設計 |


## [​](https://code.claude.com/docs/zh-TW/agent-sdk/overview#%E5%B0%87-agent-sdk-%E8%88%87%E5%85%B6%E4%BB%96-claude-%E5%B7%A5%E5%85%B7%E9%80%B2%E8%A1%8C%E6%AF%94%E8%BC%83) 將 Agent SDK 與其他 Claude 工具進行比較


Claude 平台提供多種方式來使用 Claude 進行構建。以下是 Agent SDK 的適用方式：


- Agent SDK vs Client SDK
- Agent SDK vs Claude Code CLI

[Anthropic Client SDK](https://platform.claude.com/docs/en/api/client-sdks) 為您提供直接 API 存取：您傳送提示並自己實現工具執行。 **Agent SDK** 為您提供具有內建工具執行的 Claude。 使用 Client SDK，您實現工具迴圈。使用 Agent SDK，Claude 處理它： Python TypeScript

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

相同的功能，不同的介面：

| 使用案例 | 最佳選擇 |
| --- | --- |
| 互動式開發 | CLI |
| CI/CD 管道 | SDK |
| 自訂應用程式 | SDK |
| 一次性任務 | CLI |
| 生產自動化 | SDK |

許多團隊同時使用兩者：CLI 用於日常開發，SDK 用於生產。工作流程在它們之間直接轉換。


## [​](https://code.claude.com/docs/zh-TW/agent-sdk/overview#%E8%AE%8A%E6%9B%B4%E6%97%A5%E8%AA%8C) 變更日誌


查看完整的變更日誌以了解 SDK 更新、錯誤修復和新功能：


- **TypeScript SDK**：[檢視 CHANGELOG.md](https://github.com/anthropics/claude-agent-sdk-typescript/blob/main/CHANGELOG.md)
- **Python SDK**：[檢視 CHANGELOG.md](https://github.com/anthropics/claude-agent-sdk-python/blob/main/CHANGELOG.md)


## [​](https://code.claude.com/docs/zh-TW/agent-sdk/overview#%E5%A0%B1%E5%91%8A%E9%8C%AF%E8%AA%A4) 報告錯誤


如果您遇到 Agent SDK 的錯誤或問題：


- **TypeScript SDK**：[在 GitHub 上報告問題](https://github.com/anthropics/claude-agent-sdk-typescript/issues)
- **Python SDK**：[在 GitHub 上報告問題](https://github.com/anthropics/claude-agent-sdk-python/issues)


## [​](https://code.claude.com/docs/zh-TW/agent-sdk/overview#%E5%93%81%E7%89%8C%E6%8C%87%E5%8D%97) 品牌指南


對於整合 Claude Agent SDK 的合作夥伴，使用 Claude 品牌是可選的。在您的產品中引用 Claude 時：
**允許：**


- “Claude Agent”（下拉選單的首選）
- “Claude”（當已在標記為”Agents”的選單中時）
- ” Powered by Claude”（如果您有現有的代理名稱）


**不允許：**


- “Claude Code” 或 “Claude Code Agent”
- Claude Code 品牌的 ASCII 藝術或模仿 Claude Code 的視覺元素


您的產品應保持自己的品牌，不應顯示為 Claude Code 或任何 Anthropic 產品。有關品牌合規性的問題，請聯絡 Anthropic [銷售團隊](https://www.anthropic.com/contact-sales)。


## [​](https://code.claude.com/docs/zh-TW/agent-sdk/overview#%E8%A8%B1%E5%8F%AF%E8%AD%89%E5%92%8C%E6%A2%9D%E6%AC%BE) 許可證和條款


Claude Agent SDK 的使用受 [Anthropic 商業服務條款](https://www.anthropic.com/legal/commercial-terms)管制，包括當您使用它為您自己的客戶和最終使用者提供的產品和服務提供動力時，除非特定元件或依賴項受到該元件 LICENSE 檔案中指示的不同許可證的保護。


## [​](https://code.claude.com/docs/zh-TW/agent-sdk/overview#%E5%BE%8C%E7%BA%8C%E6%AD%A5%E9%A9%9F) 後續步驟


## 快速入門

構建在幾分鐘內尋找和修復錯誤的代理

## 範例代理

電子郵件助手、研究代理等

## TypeScript SDK

完整的 TypeScript API 參考和範例

## Python SDK

完整的 Python API 參考和範例[Claude Code Docs home page](https://code.claude.com/docs/zh-TW/overview)

[Privacy choices](https://code.claude.com/docs/zh-TW/agent-sdk/overview#)

