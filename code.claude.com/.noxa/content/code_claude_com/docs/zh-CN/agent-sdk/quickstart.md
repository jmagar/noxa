使用 Agent SDK 构建一个 AI 代理，它可以读取你的代码、发现错误并修复它们，所有这一切都无需手动干预。
**你将做什么：**


1. 使用 Agent SDK 设置一个项目
2. 创建一个包含一些有缺陷代码的文件
3. 运行一个代理，自动查找并修复错误


## [​](https://code.claude.com/docs/zh-CN/agent-sdk/quickstart#%E5%89%8D%E7%BD%AE%E6%9D%A1%E4%BB%B6) 前置条件


- **Node.js 18+** 或 **Python 3.10+**
- 一个 **Anthropic 账户**（[在此注册](https://platform.claude.com/)）


## [​](https://code.claude.com/docs/zh-CN/agent-sdk/quickstart#%E8%AE%BE%E7%BD%AE) 设置
## ​前置条件
## ​设置
## ​创建一个有缺陷的文件
## ​构建一个查找和修复错误的代理
## ​关键概念
## ​故障排除
## ​后续步骤









1

创建项目文件夹

为此快速开始创建一个新目录：

```
mkdir my-agent && cd my-agent
```

对于你自己的项目，你可以从任何文件夹运行 SDK；默认情况下，它将有权访问该目录及其子目录中的文件。 2

安装 SDK

为你的语言安装 Agent SDK 包：

- TypeScript
- Python (uv)
- Python (pip)


```
npm install @anthropic-ai/claude-agent-sdk
```

[uv Python 包管理器](https://docs.astral.sh/uv/)是一个快速的 Python 包管理器，可以自动处理虚拟环境：

```
uv init && uv add claude-agent-sdk
```

首先创建一个虚拟环境，然后安装：

```
python3 -m venv .venv && source .venv/bin/activate
pip3 install claude-agent-sdk
```

TypeScript SDK 为你的平台捆绑了一个本地 Claude Code 二进制文件作为可选依赖项，所以你不需要单独安装 Claude Code。 3

设置你的 API 密钥

从 [Claude 控制台](https://platform.claude.com/)获取 API 密钥，然后在你的项目目录中创建一个 `.env` 文件：

```
ANTHROPIC_API_KEY=your-api-key
```

SDK 还支持通过第三方 API 提供商进行身份验证：

- **Amazon Bedrock**：设置 `CLAUDE_CODE_USE_BEDROCK=1` 环境变量并配置 AWS 凭证
- **Google Vertex AI**：设置 `CLAUDE_CODE_USE_VERTEX=1` 环境变量并配置 Google Cloud 凭证
- **Microsoft Azure**：设置 `CLAUDE_CODE_USE_FOUNDRY=1` 环境变量并配置 Azure 凭证

有关详细信息，请参阅 [Bedrock](https://code.claude.com/docs/zh-CN/amazon-bedrock)、 [Vertex AI](https://code.claude.com/docs/zh-CN/google-vertex-ai) 或 [Azure AI Foundry](https://code.claude.com/docs/zh-CN/microsoft-foundry) 的设置指南。 除非事先获得批准，否则 Anthropic 不允许第三方开发者提供 claude.ai 登录或对其产品的速率限制，包括基于 Claude Agent SDK 构建的代理。请改用本文档中描述的 API 密钥身份验证方法。


## [​](https://code.claude.com/docs/zh-CN/agent-sdk/quickstart#%E5%88%9B%E5%BB%BA%E4%B8%80%E4%B8%AA%E6%9C%89%E7%BC%BA%E9%99%B7%E7%9A%84%E6%96%87%E4%BB%B6) 创建一个有缺陷的文件


此快速开始将引导你构建一个可以查找和修复代码中错误的代理。首先，你需要一个包含一些有意错误的文件供代理修复。在 `my-agent` 目录中创建 `utils.py` 并粘贴以下代码：


```
def calculate_average(numbers):
    total = 0
    for num in numbers:
        total += num
    return total / len(numbers)


def get_user_name(user):
    return user["name"].upper()
```


此代码有两个错误：


1. `calculate_average([])` 会因除以零而崩溃
2. `get_user_name(None)` 会因 TypeError 而崩溃


## [​](https://code.claude.com/docs/zh-CN/agent-sdk/quickstart#%E6%9E%84%E5%BB%BA%E4%B8%80%E4%B8%AA%E6%9F%A5%E6%89%BE%E5%92%8C%E4%BF%AE%E5%A4%8D%E9%94%99%E8%AF%AF%E7%9A%84%E4%BB%A3%E7%90%86) 构建一个查找和修复错误的代理


如果你使用 Python SDK，创建 `agent.py`，或者如果使用 TypeScript，创建 `agent.ts`：
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


此代码有三个主要部分：


1. **`query`**：创建 agentic 循环的主入口点。它返回一个异步迭代器，所以你使用 `async for` 来流式传输 Claude 工作时的消息。查看 [Python](https://code.claude.com/docs/zh-CN/agent-sdk/python#query) 或 [TypeScript](https://code.claude.com/docs/zh-CN/agent-sdk/typescript#query) SDK 参考中的完整 API。
2. **`prompt`**：你想让 Claude 做什么。Claude 根据任务确定要使用哪些工具。
3. **`options`**：代理的配置。此示例使用 `allowedTools` 预先批准 `Read`、 `Edit` 和 `Glob`，以及 `permissionMode: "acceptEdits"` 来自动批准文件更改。其他选项包括 `systemPrompt`、 `mcpServers` 等。查看 [Python](https://code.claude.com/docs/zh-CN/agent-sdk/python#claude-agent-options) 或 [TypeScript](https://code.claude.com/docs/zh-CN/agent-sdk/typescript#options) 的所有选项。


`async for` 循环在 Claude 思考、调用工具、观察结果并决定下一步做什么时继续运行。每次迭代都会产生一条消息：Claude 的推理、工具调用、工具结果或最终结果。SDK 处理编排（工具执行、上下文管理、重试），所以你只需使用流。当 Claude 完成任务或遇到错误时，循环结束。
循环内的消息处理过滤人类可读的输出。如果没有过滤，你会看到原始消息对象，包括系统初始化和内部状态，这对调试很有用，但通常很冗长。
此示例使用流式传输来实时显示进度。如果你不需要实时输出（例如，对于后台作业或 CI 管道），你可以一次性收集所有消息。有关详细信息，请参阅 [流式传输与单轮模式](https://code.claude.com/docs/zh-CN/agent-sdk/streaming-vs-single-mode)。


### [​](https://code.claude.com/docs/zh-CN/agent-sdk/quickstart#%E8%BF%90%E8%A1%8C%E4%BD%A0%E7%9A%84%E4%BB%A3%E7%90%86) 运行你的代理


你的代理已准备好。使用以下命令运行它：


- Python
- TypeScript


```
python3 agent.py
```


```
npx tsx agent.ts
```


运行后，检查 `utils.py`。你会看到处理空列表和空用户的防御性代码。你的代理自主地：


1. **读取** `utils.py` 以理解代码
2. **分析**了逻辑并识别了会导致崩溃的边界情况
3. **编辑**了文件以添加适当的错误处理


这就是 Agent SDK 的与众不同之处：Claude 直接执行工具，而不是要求你实现它们。
如果你看到”API key not found”，请确保你已在 `.env` 文件或 shell 环境中设置了 `ANTHROPIC_API_KEY` 环境变量。有关更多帮助，请参阅 [完整故障排除指南](https://code.claude.com/docs/zh-CN/troubleshooting)。


### [​](https://code.claude.com/docs/zh-CN/agent-sdk/quickstart#%E5%B0%9D%E8%AF%95%E5%85%B6%E4%BB%96%E6%8F%90%E7%A4%BA) 尝试其他提示


现在你的代理已设置好，尝试一些不同的提示：


- `"Add docstrings to all functions in utils.py"`
- `"Add type hints to all functions in utils.py"`
- `"Create a README.md documenting the functions in utils.py"`


### [​](https://code.claude.com/docs/zh-CN/agent-sdk/quickstart#%E8%87%AA%E5%AE%9A%E4%B9%89%E4%BD%A0%E7%9A%84%E4%BB%A3%E7%90%86) 自定义你的代理


你可以通过更改选项来修改代理的行为。以下是一些示例：
**添加网络搜索功能：**
Python TypeScript

```
options = ClaudeAgentOptions(
    allowed_tools=["Read", "Edit", "Glob", "WebSearch"], permission_mode="acceptEdits"
)
```


**给 Claude 一个自定义系统提示：**
Python TypeScript

```
options = ClaudeAgentOptions(
    allowed_tools=["Read", "Edit", "Glob"],
    permission_mode="acceptEdits",
    system_prompt="You are a senior Python developer. Always follow PEP 8 style guidelines.",
)
```


**在终端中运行命令：**
Python TypeScript

```
options = ClaudeAgentOptions(
    allowed_tools=["Read", "Edit", "Glob", "Bash"], permission_mode="acceptEdits"
)
```


启用 `Bash` 后，尝试： `"Write unit tests for utils.py, run them, and fix any failures"`


## [​](https://code.claude.com/docs/zh-CN/agent-sdk/quickstart#%E5%85%B3%E9%94%AE%E6%A6%82%E5%BF%B5) 关键概念


**工具**控制你的代理可以做什么：


| 工具 | 代理可以做什么 |
| --- | --- |
| `Read`、 `Glob`、 `Grep` | 只读分析 |
| `Read`、 `Edit`、 `Glob` | 分析和修改代码 |
| `Read`、 `Edit`、 `Bash`、 `Glob`、 `Grep` | 完全自动化 |


**权限模式**控制你想要多少人工监督：


| 模式 | 行为 | 用例 |
| --- | --- | --- |
| `acceptEdits` | 自动批准文件编辑和常见文件系统命令，询问其他操作 | 受信任的开发工作流 |
| `dontAsk` | 拒绝不在 `allowedTools` 中的任何内容 | 锁定的无头代理 |
| `auto`（仅 TypeScript） | 模型分类器批准或拒绝每个工具调用 | 具有安全防护的自主代理 |
| `bypassPermissions` | 运行每个工具而不提示 | 沙箱 CI、完全受信任的环境 |
| `default` | 需要 `canUseTool` 回调来处理批准 | 自定义批准流程 |


上面的示例使用 `acceptEdits` 模式，它自动批准文件操作，以便代理可以在没有交互式提示的情况下运行。如果你想提示用户批准，使用 `default` 模式并提供一个 [`canUseTool` 回调](https://code.claude.com/docs/zh-CN/agent-sdk/user-input)来收集用户输入。为了获得更多控制，请参阅 [权限](https://code.claude.com/docs/zh-CN/agent-sdk/permissions)。


## [​](https://code.claude.com/docs/zh-CN/agent-sdk/quickstart#%E6%95%85%E9%9A%9C%E6%8E%92%E9%99%A4) 故障排除


### [​](https://code.claude.com/docs/zh-CN/agent-sdk/quickstart#api-%E9%94%99%E8%AF%AF-thinking-type-enabled-%E4%B8%8D%E6%94%AF%E6%8C%81%E6%AD%A4%E6%A8%A1%E5%9E%8B) API 错误 `thinking.type.enabled` 不支持此模型


Claude Opus 4.7 用 `thinking.type.adaptive` 替换了 `thinking.type.enabled`。当你选择 `claude-opus-4-7` 时，较旧的 Agent SDK 版本会失败，出现以下 API 错误：


```
API Error: 400 {"type":"invalid_request_error","message":"\"thinking.type.enabled\" is not supported for this model. Use \"thinking.type.adaptive\" and \"output_config.effort\" to control thinking behavior."}
```


升级到 Agent SDK v0.2.111 或更高版本以使用 Opus 4.7。


## [​](https://code.claude.com/docs/zh-CN/agent-sdk/quickstart#%E5%90%8E%E7%BB%AD%E6%AD%A5%E9%AA%A4) 后续步骤


现在你已经创建了你的第一个代理，学习如何扩展其功能并将其定制到你的用例：


- **[权限](https://code.claude.com/docs/zh-CN/agent-sdk/permissions)**：控制你的代理可以做什么以及何时需要批准
- **[Hooks](https://code.claude.com/docs/zh-CN/agent-sdk/hooks)**：在工具调用之前或之后运行自定义代码
- **[会话](https://code.claude.com/docs/zh-CN/agent-sdk/sessions)**：构建维护上下文的多轮代理
- **[MCP 服务器](https://code.claude.com/docs/zh-CN/agent-sdk/mcp)**：连接到数据库、浏览器、API 和其他外部系统
- **[托管](https://code.claude.com/docs/zh-CN/agent-sdk/hosting)**：将代理部署到 Docker、云和 CI/CD
- **[示例代理](https://github.com/anthropics/claude-agent-sdk-demos)**：查看完整示例：电子邮件助手、研究代理等[Claude Code Docs home page](https://code.claude.com/docs/zh-CN/overview)

[Privacy choices](https://code.claude.com/docs/zh-CN/agent-sdk/quickstart#)

