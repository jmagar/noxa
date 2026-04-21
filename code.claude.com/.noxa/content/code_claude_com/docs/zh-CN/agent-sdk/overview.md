# Agent SDK 概览
## ​开始使用
## ​功能
## ​将 Agent SDK 与其他 Claude 工具进行比较
## ​更新日志
## ​报告 bug
## ​品牌指南
## ​许可证和条款
## ​后续步骤









使用 Claude Code 作为库构建生产级 AI 代理

Claude Code SDK 已重命名为 Claude Agent SDK。如果您正在从旧 SDK 迁移，请参阅 [迁移指南](https://code.claude.com/docs/zh-CN/agent-sdk/migration-guide)。
构建能够自主读取文件、运行命令、搜索网络、编辑代码等的 AI 代理。Agent SDK 为您提供了与 Claude Code 相同的工具、代理循环和上下文管理，可在 Python 和 TypeScript 中编程。
Opus 4.7 ( `claude-opus-4-7`) 需要 Agent SDK v0.2.111 或更高版本。如果您看到 `thinking.type.enabled` API 错误，请参阅 [故障排除](https://code.claude.com/docs/zh-CN/agent-sdk/quickstart#troubleshooting)。
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


Agent SDK 包含用于读取文件、运行命令和编辑代码的内置工具，因此您的代理可以立即开始工作，无需您实现工具执行。深入了解快速入门或探索使用 SDK 构建的真实代理：


## 快速入门

在几分钟内构建一个 bug 修复代理

## 示例代理

电子邮件助手、研究代理等


## [​](https://code.claude.com/docs/zh-CN/agent-sdk/overview#%E5%BC%80%E5%A7%8B%E4%BD%BF%E7%94%A8) 开始使用


1

安装 SDK


- TypeScript
- Python


```
npm install @anthropic-ai/claude-agent-sdk
```


```
pip install claude-agent-sdk
```

TypeScript SDK 为您的平台捆绑了一个本地 Claude Code 二进制文件作为可选依赖项，因此您无需单独安装 Claude Code。 2

设置您的 API 密钥

从 [控制台](https://platform.claude.com/)获取 API 密钥，然后将其设置为环境变量：

```
export ANTHROPIC_API_KEY=your-api-key
```

SDK 还支持通过第三方 API 提供商进行身份验证：

- **Amazon Bedrock**：设置 `CLAUDE_CODE_USE_BEDROCK=1` 环境变量并配置 AWS 凭证
- **Google Vertex AI**：设置 `CLAUDE_CODE_USE_VERTEX=1` 环境变量并配置 Google Cloud 凭证
- **Microsoft Azure**：设置 `CLAUDE_CODE_USE_FOUNDRY=1` 环境变量并配置 Azure 凭证

有关详细信息，请参阅 [Bedrock](https://code.claude.com/docs/zh-CN/amazon-bedrock)、 [Vertex AI](https://code.claude.com/docs/zh-CN/google-vertex-ai) 或 [Azure AI Foundry](https://code.claude.com/docs/zh-CN/microsoft-foundry) 的设置指南。 除非事先获得批准，否则 Anthropic 不允许第三方开发人员为其产品（包括基于 Claude Agent SDK 构建的代理）提供 claude.ai 登录或速率限制。请改用本文档中描述的 API 密钥身份验证方法。 3

运行您的第一个代理

此示例创建一个代理，该代理使用内置工具列出当前目录中的文件。 Python TypeScript

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


**准备好构建了吗？** 按照 [快速入门](https://code.claude.com/docs/zh-CN/agent-sdk/quickstart)在几分钟内创建一个查找和修复 bug 的代理。


## [​](https://code.claude.com/docs/zh-CN/agent-sdk/overview#%E5%8A%9F%E8%83%BD) 功能


使 Claude Code 强大的一切都可在 SDK 中使用：


- 内置工具
- Hooks
- 子代理
- MCP
- 权限
- 会话

您的代理可以开箱即用地读取文件、运行命令和搜索代码库。关键工具包括：

| 工具 | 功能 |
| --- | --- |
| **Read** | 读取工作目录中的任何文件 |
| **Write** | 创建新文件 |
| **Edit** | 对现有文件进行精确编辑 |
| **Bash** | 运行终端命令、脚本、git 操作 |
| **Monitor** | 监视后台脚本并对每个输出行作为事件做出反应 |
| **Glob** | 按模式查找文件（ `**/*.ts`、 `src/**/*.py`） |
| **Grep** | 使用正则表达式搜索文件内容 |
| **WebSearch** | 搜索网络以获取当前信息 |
| **WebFetch** | 获取并解析网页内容 |
| **[AskUserQuestion](https://code.claude.com/docs/zh-CN/agent-sdk/user-input#handle-clarifying-questions)** | 向用户提出带有多选选项的澄清问题 |

此示例创建一个代理，该代理在您的代码库中搜索 TODO 注释： Python TypeScript

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

在代理生命周期的关键点运行自定义代码。SDK hooks 使用回调函数来验证、记录、阻止或转换代理行为。 **可用 hooks：** `PreToolUse`、 `PostToolUse`、 `Stop`、 `SessionStart`、 `SessionEnd`、 `UserPromptSubmit` 等。 此示例将所有文件更改记录到审计文件： Python TypeScript

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

[了解更多关于 hooks →](https://code.claude.com/docs/zh-CN/agent-sdk/hooks) 生成专门的代理来处理专注的子任务。您的主代理委派工作，子代理报告结果。 定义具有专门说明的自定义代理。在 `allowedTools` 中包含 `Agent`，因为子代理通过 Agent 工具调用： Python TypeScript

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

来自子代理上下文内的消息包含 `parent_tool_use_id` 字段，让您可以跟踪哪些消息属于哪个子代理执行。 [了解更多关于子代理 →](https://code.claude.com/docs/zh-CN/agent-sdk/subagents) 通过 Model Context Protocol 连接到外部系统：数据库、浏览器、API 和 [数百个更多](https://github.com/modelcontextprotocol/servers)。 此示例连接 [Playwright MCP 服务器](https://github.com/microsoft/playwright-mcp)以为您的代理提供浏览器自动化功能： Python TypeScript

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

[了解更多关于 MCP →](https://code.claude.com/docs/zh-CN/agent-sdk/mcp) 精确控制您的代理可以使用哪些工具。允许安全操作、阻止危险操作或要求对敏感操作进行批准。 对于交互式批准提示和 `AskUserQuestion` 工具，请参阅 [处理批准和用户输入](https://code.claude.com/docs/zh-CN/agent-sdk/user-input)。 此示例创建一个只读代理，可以分析但不能修改代码。 `allowed_tools` 预先批准 `Read`、 `Glob` 和 `Grep`。 Python TypeScript

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

[了解更多关于权限 →](https://code.claude.com/docs/zh-CN/agent-sdk/permissions) 在多次交换中保持上下文。Claude 记住读取的文件、完成的分析和对话历史。稍后恢复会话，或分叉它们以探索不同的方法。 此示例从第一个查询中捕获会话 ID，然后恢复以继续完整上下文： Python TypeScript

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

[了解更多关于会话 →](https://code.claude.com/docs/zh-CN/agent-sdk/sessions)


### [​](https://code.claude.com/docs/zh-CN/agent-sdk/overview#claude-code-%E5%8A%9F%E8%83%BD) Claude Code 功能


SDK 还支持 Claude Code 的基于文件系统的配置。使用默认选项，SDK 从您的工作目录中的 `.claude/` 和 `~/.claude/` 加载这些。要限制加载哪些源，请在您的选项中设置 `setting_sources`（Python）或 `settingSources`（TypeScript）。


| 功能 | 描述 | 位置 |
| --- | --- | --- |
| [Skills](https://code.claude.com/docs/zh-CN/agent-sdk/skills) | 在 Markdown 中定义的专门功能 | `.claude/skills/*/SKILL.md` |
| [Slash commands](https://code.claude.com/docs/zh-CN/agent-sdk/slash-commands) | 用于常见任务的自定义命令 | `.claude/commands/*.md` |
| [Memory](https://code.claude.com/docs/zh-CN/agent-sdk/modifying-system-prompts) | 项目上下文和说明 | `CLAUDE.md` 或 `.claude/CLAUDE.md` |
| [Plugins](https://code.claude.com/docs/zh-CN/agent-sdk/plugins) | 使用自定义命令、代理和 MCP 服务器扩展 | 通过 `plugins` 选项编程 |


## [​](https://code.claude.com/docs/zh-CN/agent-sdk/overview#%E5%B0%86-agent-sdk-%E4%B8%8E%E5%85%B6%E4%BB%96-claude-%E5%B7%A5%E5%85%B7%E8%BF%9B%E8%A1%8C%E6%AF%94%E8%BE%83) 将 Agent SDK 与其他 Claude 工具进行比较


Claude 平台提供了多种使用 Claude 构建的方式。以下是 Agent SDK 的适用场景：


- Agent SDK vs Client SDK
- Agent SDK vs Claude Code CLI

[Anthropic Client SDK](https://platform.claude.com/docs/en/api/client-sdks) 为您提供直接 API 访问：您发送提示并自己实现工具执行。 **Agent SDK** 为您提供具有内置工具执行的 Claude。 使用 Client SDK，您实现工具循环。使用 Agent SDK，Claude 处理它： Python TypeScript

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

相同的功能，不同的界面：

| 用例 | 最佳选择 |
| --- | --- |
| 交互式开发 | CLI |
| CI/CD 管道 | SDK |
| 自定义应用程序 | SDK |
| 一次性任务 | CLI |
| 生产自动化 | SDK |

许多团队同时使用两者：CLI 用于日常开发，SDK 用于生产。工作流在它们之间直接转换。


## [​](https://code.claude.com/docs/zh-CN/agent-sdk/overview#%E6%9B%B4%E6%96%B0%E6%97%A5%E5%BF%97) 更新日志


查看完整的更新日志以了解 SDK 更新、bug 修复和新功能：


- **TypeScript SDK**：[查看 CHANGELOG.md](https://github.com/anthropics/claude-agent-sdk-typescript/blob/main/CHANGELOG.md)
- **Python SDK**：[查看 CHANGELOG.md](https://github.com/anthropics/claude-agent-sdk-python/blob/main/CHANGELOG.md)


## [​](https://code.claude.com/docs/zh-CN/agent-sdk/overview#%E6%8A%A5%E5%91%8A-bug) 报告 bug


如果您在 Agent SDK 中遇到 bug 或问题：


- **TypeScript SDK**：[在 GitHub 上报告问题](https://github.com/anthropics/claude-agent-sdk-typescript/issues)
- **Python SDK**：[在 GitHub 上报告问题](https://github.com/anthropics/claude-agent-sdk-python/issues)


## [​](https://code.claude.com/docs/zh-CN/agent-sdk/overview#%E5%93%81%E7%89%8C%E6%8C%87%E5%8D%97) 品牌指南


对于集成 Claude Agent SDK 的合作伙伴，使用 Claude 品牌是可选的。在您的产品中引用 Claude 时：
**允许：**


- “Claude Agent”（首选用于下拉菜单）
- “Claude”（当已在标记为”Agents”的菜单中时）
- ” Powered by Claude”（如果您有现有的代理名称）


**不允许：**


- “Claude Code” 或 “Claude Code Agent”
- Claude Code 品牌的 ASCII 艺术或模仿 Claude Code 的视觉元素


您的产品应保持自己的品牌，不应显示为 Claude Code 或任何 Anthropic 产品。如有关于品牌合规性的问题，请联系 Anthropic [销售团队](https://www.anthropic.com/contact-sales)。


## [​](https://code.claude.com/docs/zh-CN/agent-sdk/overview#%E8%AE%B8%E5%8F%AF%E8%AF%81%E5%92%8C%E6%9D%A1%E6%AC%BE) 许可证和条款


Claude Agent SDK 的使用受 [Anthropic 商业服务条款](https://www.anthropic.com/legal/commercial-terms)管制，包括当您使用它为您自己的客户和最终用户提供的产品和服务时，除非特定组件或依赖项由该组件的 LICENSE 文件中指示的不同许可证覆盖。


## [​](https://code.claude.com/docs/zh-CN/agent-sdk/overview#%E5%90%8E%E7%BB%AD%E6%AD%A5%E9%AA%A4) 后续步骤


## 快速入门

构建一个在几分钟内查找和修复 bug 的代理

## 示例代理

电子邮件助手、研究代理等

## TypeScript SDK

完整的 TypeScript API 参考和示例

## Python SDK

完整的 Python API 参考和示例[Claude Code Docs home page](https://code.claude.com/docs/zh-CN/overview)

[Privacy choices](https://code.claude.com/docs/zh-CN/agent-sdk/overview#)

