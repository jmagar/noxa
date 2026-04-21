使用 Agent SDK 構建一個 AI 代理，它可以讀取您的代碼、發現錯誤並自動修復它們，無需手動干預。
**您將執行的操作：**


1. 使用 Agent SDK 設置項目
2. 創建一個包含一些有缺陷代碼的文件
3. 運行一個代理，自動查找並修復錯誤


## [​](https://code.claude.com/docs/zh-TW/agent-sdk/quickstart#%E5%85%88%E6%B1%BA%E6%A2%9D%E4%BB%B6) 先決條件


- **Node.js 18+** 或 **Python 3.10+**
- 一個 **Anthropic 帳戶**（[在此註冊](https://platform.claude.com/)）


## [​](https://code.claude.com/docs/zh-TW/agent-sdk/quickstart#%E8%A8%AD%E7%BD%AE) 設置
## ​先決條件
## ​設置
## ​創建有缺陷的文件
## ​構建查找並修復錯誤的代理
## ​關鍵概念
## ​故障排除
## ​後續步驟









1

創建項目文件夾

為此快速開始創建一個新目錄：

```
mkdir my-agent && cd my-agent
```

對於您自己的項目，您可以從任何文件夾運行 SDK；默認情況下，它將有權訪問該目錄及其子目錄中的文件。 2

安裝 SDK

為您的語言安裝 Agent SDK 包：

- TypeScript
- Python (uv)
- Python (pip)


```
npm install @anthropic-ai/claude-agent-sdk
```

[uv Python 包管理器](https://docs.astral.sh/uv/)是一個快速的 Python 包管理器，可自動處理虛擬環境：

```
uv init && uv add claude-agent-sdk
```

首先創建虛擬環境，然後安裝：

```
python3 -m venv .venv && source .venv/bin/activate
pip3 install claude-agent-sdk
```

TypeScript SDK 為您的平台捆綁了一個本機 Claude Code 二進制文件作為可選依賴項，因此您無需單獨安裝 Claude Code。 3

設置您的 API 密鑰

從 [Claude 控制台](https://platform.claude.com/)獲取 API 密鑰，然後在您的項目目錄中創建一個 `.env` 文件：

```
ANTHROPIC_API_KEY=your-api-key
```

SDK 還支持通過第三方 API 提供商進行身份驗證：

- **Amazon Bedrock**：設置 `CLAUDE_CODE_USE_BEDROCK=1` 環境變量並配置 AWS 憑證
- **Google Vertex AI**：設置 `CLAUDE_CODE_USE_VERTEX=1` 環境變量並配置 Google Cloud 憑證
- **Microsoft Azure**：設置 `CLAUDE_CODE_USE_FOUNDRY=1` 環境變量並配置 Azure 憑證

有關詳細信息，請參閱 [Bedrock](https://code.claude.com/docs/zh-TW/amazon-bedrock)、 [Vertex AI](https://code.claude.com/docs/zh-TW/google-vertex-ai) 或 [Azure AI Foundry](https://code.claude.com/docs/zh-TW/microsoft-foundry) 的設置指南。 除非事先獲得批准，否則 Anthropic 不允許第三方開發人員提供 claude.ai 登錄或對其產品的速率限制，包括基於 Claude Agent SDK 構建的代理。請改用本文檔中描述的 API 密鑰身份驗證方法。


## [​](https://code.claude.com/docs/zh-TW/agent-sdk/quickstart#%E5%89%B5%E5%BB%BA%E6%9C%89%E7%BC%BA%E9%99%B7%E7%9A%84%E6%96%87%E4%BB%B6) 創建有缺陷的文件


此快速開始將引導您構建一個可以查找並修復代碼中的錯誤的代理。首先，您需要一個包含一些故意錯誤的文件供代理修復。在 `my-agent` 目錄中創建 `utils.py` 並粘貼以下代碼：


```
def calculate_average(numbers):
    total = 0
    for num in numbers:
        total += num
    return total / len(numbers)


def get_user_name(user):
    return user["name"].upper()
```


此代碼有兩個錯誤：


1. `calculate_average([])` 因除以零而崩潰
2. `get_user_name(None)` 因 TypeError 而崩潰


## [​](https://code.claude.com/docs/zh-TW/agent-sdk/quickstart#%E6%A7%8B%E5%BB%BA%E6%9F%A5%E6%89%BE%E4%B8%A6%E4%BF%AE%E5%BE%A9%E9%8C%AF%E8%AA%A4%E7%9A%84%E4%BB%A3%E7%90%86) 構建查找並修復錯誤的代理


如果您使用 Python SDK，請創建 `agent.py`，或者如果使用 TypeScript，請創建 `agent.ts`：
Python TypeScript

```
import asyncio
from claude_agent_sdk import query, ClaudeAgentOptions, AssistantMessage, ResultMessage


async def main():
    # Agentic loop: streams messages as Claude works
    async for message in query(
        prompt="Review utils.py for bugs that would cause crashes. Fix any issues you find.",
        options=ClaudeAgentOptions(
            allowed_tools=["Read", "Edit", "Glob"],  # Tools Claude can use
            permission_mode="acceptEdits",  # Auto-approve file edits
        ),
    ):
        # Print human-readable output
        if isinstance(message, AssistantMessage):
            for block in message.content:
                if hasattr(block, "text"):
                    print(block.text)  # Claude's reasoning
                elif hasattr(block, "name"):
                    print(f"Tool: {block.name}")  # Tool being called
        elif isinstance(message, ResultMessage):
            print(f"Done: {message.subtype}")  # Final result


asyncio.run(main())
```


此代碼有三個主要部分：


1. **`query`**：創建 agentic 循環的主要入口點。它返回一個異步迭代器，因此您使用 `async for` 在 Claude 工作時流式傳輸消息。請參閱 [Python](https://code.claude.com/docs/zh-TW/agent-sdk/python#query) 或 [TypeScript](https://code.claude.com/docs/zh-TW/agent-sdk/typescript#query) SDK 參考中的完整 API。
2. **`prompt`**：您希望 Claude 執行的操作。Claude 根據任務確定要使用哪些工具。
3. **`options`**：代理的配置。此示例使用 `allowedTools` 預先批准 `Read`、 `Edit` 和 `Glob`，並使用 `permissionMode: "acceptEdits"` 自動批准文件更改。其他選項包括 `systemPrompt`、 `mcpServers` 等。請參閱 [Python](https://code.claude.com/docs/zh-TW/agent-sdk/python#claude-agent-options) 或 [TypeScript](https://code.claude.com/docs/zh-TW/agent-sdk/typescript#options) 的所有選項。


`async for` 循環在 Claude 思考、調用工具、觀察結果並決定下一步操作時持續運行。每次迭代都會產生一條消息：Claude 的推理、工具調用、工具結果或最終結果。SDK 處理編排（工具執行、上下文管理、重試），因此您只需使用流。當 Claude 完成任務或遇到錯誤時，循環結束。
循環內的消息處理會過濾人類可讀的輸出。如果沒有過濾，您會看到原始消息對象，包括系統初始化和內部狀態，這對於調試很有用，但通常很冗長。
此示例使用流式傳輸來實時顯示進度。如果您不需要實時輸出（例如，對於後台作業或 CI 管道），您可以一次收集所有消息。有關詳細信息，請參閱 [流式傳輸與單輪模式](https://code.claude.com/docs/zh-TW/agent-sdk/streaming-vs-single-mode)。


### [​](https://code.claude.com/docs/zh-TW/agent-sdk/quickstart#%E9%81%8B%E8%A1%8C%E6%82%A8%E7%9A%84%E4%BB%A3%E7%90%86) 運行您的代理


您的代理已準備好。使用以下命令運行它：


- Python
- TypeScript


```
python3 agent.py
```


```
npx tsx agent.ts
```


運行後，檢查 `utils.py`。您將看到處理空列表和空用戶的防禦性代碼。您的代理自主地：


1. **讀取** `utils.py` 以理解代碼
2. **分析**邏輯並識別會導致崩潰的邊界情況
3. **編輯**文件以添加適當的錯誤處理


這就是 Agent SDK 的不同之處：Claude 直接執行工具，而不是要求您實現它們。
如果您看到”API key not found”，請確保您已在 `.env` 文件或 shell 環境中設置 `ANTHROPIC_API_KEY` 環境變量。有關更多幫助，請參閱 [完整故障排除指南](https://code.claude.com/docs/zh-TW/troubleshooting)。


### [​](https://code.claude.com/docs/zh-TW/agent-sdk/quickstart#%E5%98%97%E8%A9%A6%E5%85%B6%E4%BB%96%E6%8F%90%E7%A4%BA) 嘗試其他提示


現在您的代理已設置好，嘗試一些不同的提示：


- `"Add docstrings to all functions in utils.py"`
- `"Add type hints to all functions in utils.py"`
- `"Create a README.md documenting the functions in utils.py"`


### [​](https://code.claude.com/docs/zh-TW/agent-sdk/quickstart#%E8%87%AA%E5%AE%9A%E7%BE%A9%E6%82%A8%E7%9A%84%E4%BB%A3%E7%90%86) 自定義您的代理


您可以通過更改選項來修改代理的行為。以下是一些示例：
**添加網絡搜索功能：**
Python TypeScript

```
options = ClaudeAgentOptions(
    allowed_tools=["Read", "Edit", "Glob", "WebSearch"], permission_mode="acceptEdits"
)
```


**給 Claude 一個自定義系統提示：**
Python TypeScript

```
options = ClaudeAgentOptions(
    allowed_tools=["Read", "Edit", "Glob"],
    permission_mode="acceptEdits",
    system_prompt="You are a senior Python developer. Always follow PEP 8 style guidelines.",
)
```


**在終端中運行命令：**
Python TypeScript

```
options = ClaudeAgentOptions(
    allowed_tools=["Read", "Edit", "Glob", "Bash"], permission_mode="acceptEdits"
)
```


啟用 `Bash` 後，嘗試： `"Write unit tests for utils.py, run them, and fix any failures"`


## [​](https://code.claude.com/docs/zh-TW/agent-sdk/quickstart#%E9%97%9C%E9%8D%B5%E6%A6%82%E5%BF%B5) 關鍵概念


**工具**控制您的代理可以執行的操作：


| 工具 | 代理可以執行的操作 |
| --- | --- |
| `Read`、 `Glob`、 `Grep` | 只讀分析 |
| `Read`、 `Edit`、 `Glob` | 分析和修改代碼 |
| `Read`、 `Edit`、 `Bash`、 `Glob`、 `Grep` | 完全自動化 |


**權限模式**控制您想要多少人工監督：


| 模式 | 行為 | 用例 |
| --- | --- | --- |
| `acceptEdits` | 自動批准文件編輯和常見文件系統命令，詢問其他操作 | 受信任的開發工作流 |
| `dontAsk` | 拒絕不在 `allowedTools` 中的任何內容 | 鎖定的無頭代理 |
| `auto`（僅 TypeScript） | 模型分類器批准或拒絕每個工具調用 | 具有安全防護的自主代理 |
| `bypassPermissions` | 運行每個工具而不提示 | 沙箱 CI、完全受信任的環境 |
| `default` | 需要 `canUseTool` 回調來處理批准 | 自定義批准流程 |


上面的示例使用 `acceptEdits` 模式，它自動批准文件操作，以便代理可以無需交互提示地運行。如果您想提示用戶批准，請使用 `default` 模式並提供一個 [`canUseTool` 回調](https://code.claude.com/docs/zh-TW/agent-sdk/user-input)來收集用戶輸入。如需更多控制，請參閱 [權限](https://code.claude.com/docs/zh-TW/agent-sdk/permissions)。


## [​](https://code.claude.com/docs/zh-TW/agent-sdk/quickstart#%E6%95%85%E9%9A%9C%E6%8E%92%E9%99%A4) 故障排除


### [​](https://code.claude.com/docs/zh-TW/agent-sdk/quickstart#api-%E9%8C%AF%E8%AA%A4-thinking-type-enabled-%E4%B8%8D%E6%94%AF%E6%8C%81%E6%AD%A4%E6%A8%A1%E5%9E%8B) API 錯誤 `thinking.type.enabled` 不支持此模型


Claude Opus 4.7 將 `thinking.type.enabled` 替換為 `thinking.type.adaptive`。當您選擇 `claude-opus-4-7` 時，較舊的 Agent SDK 版本會失敗並出現以下 API 錯誤：


```
API Error: 400 {"type":"invalid_request_error","message":"\"thinking.type.enabled\" is not supported for this model. Use \"thinking.type.adaptive\" and \"output_config.effort\" to control thinking behavior."}
```


升級到 Agent SDK v0.2.111 或更高版本以使用 Opus 4.7。


## [​](https://code.claude.com/docs/zh-TW/agent-sdk/quickstart#%E5%BE%8C%E7%BA%8C%E6%AD%A5%E9%A9%9F) 後續步驟


現在您已創建了第一個代理，了解如何擴展其功能並根據您的用例進行定制：


- **[權限](https://code.claude.com/docs/zh-TW/agent-sdk/permissions)**：控制您的代理可以執行的操作以及何時需要批准
- **[Hooks](https://code.claude.com/docs/zh-TW/agent-sdk/hooks)**：在工具調用之前或之後運行自定義代碼
- **[會話](https://code.claude.com/docs/zh-TW/agent-sdk/sessions)**：構建維護上下文的多輪代理
- **[MCP 服務器](https://code.claude.com/docs/zh-TW/agent-sdk/mcp)**：連接到數據庫、瀏覽器、API 和其他外部系統
- **[託管](https://code.claude.com/docs/zh-TW/agent-sdk/hosting)**：將代理部署到 Docker、雲和 CI/CD
- **[示例代理](https://github.com/anthropics/claude-agent-sdk-demos)**：查看完整示例：電子郵件助手、研究代理等[Claude Code Docs home page](https://code.claude.com/docs/zh-TW/overview)

[Privacy choices](https://code.claude.com/docs/zh-TW/agent-sdk/quickstart#)

