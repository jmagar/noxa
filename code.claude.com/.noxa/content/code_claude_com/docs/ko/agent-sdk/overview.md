# Agent SDK 개요
## ​시작하기
## ​기능
## ​Agent SDK를 다른 Claude 도구와 비교
## ​변경 로그
## ​버그 보고
## ​브랜딩 지침
## ​라이선스 및 약관
## ​다음 단계









Claude Code를 라이브러리로 사용하여 프로덕션 AI 에이전트 구축하기

Claude Code SDK의 이름이 Claude Agent SDK로 변경되었습니다. 이전 SDK에서 마이그레이션하는 경우 [마이그레이션 가이드](https://code.claude.com/docs/ko/agent-sdk/migration-guide)를 참조하십시오.
자율적으로 파일을 읽고, 명령을 실행하고, 웹을 검색하고, 코드를 편집하는 등의 작업을 수행하는 AI 에이전트를 구축하십시오. Agent SDK는 Claude Code를 강화하는 동일한 도구, 에이전트 루프 및 컨텍스트 관리를 Python 및 TypeScript로 프로그래밍할 수 있도록 제공합니다.
Opus 4.7( `claude-opus-4-7`)은 Agent SDK v0.2.111 이상이 필요합니다. `thinking.type.enabled` API 오류가 표시되면 [문제 해결](https://code.claude.com/docs/ko/agent-sdk/quickstart#troubleshooting)을 참조하십시오.
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


Agent SDK에는 파일 읽기, 명령 실행 및 코드 편집을 위한 기본 제공 도구가 포함되어 있으므로 도구 실행을 직접 구현하지 않고도 에이전트가 즉시 작업을 시작할 수 있습니다. 빠른 시작을 살펴보거나 SDK로 구축한 실제 에이전트를 탐색하십시오:


## 빠른 시작

몇 분 안에 버그 수정 에이전트 구축하기

## 예제 에이전트

이메일 어시스턴트, 연구 에이전트 등


## [​](https://code.claude.com/docs/ko/agent-sdk/overview#%EC%8B%9C%EC%9E%91%ED%95%98%EA%B8%B0) 시작하기


1

SDK 설치


- TypeScript
- Python


```
npm install @anthropic-ai/claude-agent-sdk
```


```
pip install claude-agent-sdk
```

TypeScript SDK는 선택적 종속성으로 플랫폼용 네이티브 Claude Code 바이너리를 번들로 제공하므로 Claude Code를 별도로 설치할 필요가 없습니다. 2

API 키 설정

[콘솔](https://platform.claude.com/)에서 API 키를 가져온 다음 환경 변수로 설정하십시오:

```
export ANTHROPIC_API_KEY=your-api-key
```

SDK는 또한 타사 API 제공자를 통한 인증을 지원합니다:

- **Amazon Bedrock**: `CLAUDE_CODE_USE_BEDROCK=1` 환경 변수를 설정하고 AWS 자격 증명을 구성합니다
- **Google Vertex AI**: `CLAUDE_CODE_USE_VERTEX=1` 환경 변수를 설정하고 Google Cloud 자격 증명을 구성합니다
- **Microsoft Azure**: `CLAUDE_CODE_USE_FOUNDRY=1` 환경 변수를 설정하고 Azure 자격 증명을 구성합니다

[Bedrock](https://code.claude.com/docs/ko/amazon-bedrock), [Vertex AI](https://code.claude.com/docs/ko/google-vertex-ai) 또는 [Azure AI Foundry](https://code.claude.com/docs/ko/microsoft-foundry)의 설정 가이드를 참조하십시오. 이전에 승인되지 않은 경우, Anthropic은 타사 개발자가 Claude Agent SDK로 구축한 에이전트를 포함하여 자신의 제품에 대해 claude.ai 로그인 또는 속도 제한을 제공하도록 허용하지 않습니다. 대신 이 문서에 설명된 API 키 인증 방법을 사용하십시오. 3

첫 번째 에이전트 실행

이 예제는 기본 제공 도구를 사용하여 현재 디렉토리의 파일을 나열하는 에이전트를 만듭니다. Python TypeScript

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


**구축할 준비가 되셨나요?** [빠른 시작](https://code.claude.com/docs/ko/agent-sdk/quickstart)을 따라 몇 분 안에 버그를 찾고 수정하는 에이전트를 만드십시오.


## [​](https://code.claude.com/docs/ko/agent-sdk/overview#%EA%B8%B0%EB%8A%A5) 기능


Claude Code를 강력하게 만드는 모든 것이 SDK에서 사용 가능합니다:


- 기본 제공 도구
- Hooks
- 서브에이전트
- MCP
- 권한
- 세션

에이전트는 기본적으로 파일을 읽고, 명령을 실행하고, 코드베이스를 검색할 수 있습니다. 주요 도구는 다음과 같습니다:

| 도구 | 기능 |
| --- | --- |
| **Read** | 작업 디렉토리의 모든 파일 읽기 |
| **Write** | 새 파일 생성 |
| **Edit** | 기존 파일에 정확한 편집 수행 |
| **Bash** | 터미널 명령, 스크립트, git 작업 실행 |
| **Monitor** | 백그라운드 스크립트를 감시하고 각 출력 라인에 이벤트로 반응 |
| **Glob** | 패턴으로 파일 찾기( `**/*.ts`, `src/**/*.py`) |
| **Grep** | 정규식으로 파일 내용 검색 |
| **WebSearch** | 현재 정보를 위해 웹 검색 |
| **WebFetch** | 웹 페이지 내용 가져오기 및 구문 분석 |
| **[AskUserQuestion](https://code.claude.com/docs/ko/agent-sdk/user-input#handle-clarifying-questions)** | 여러 선택 옵션으로 사용자에게 명확히 하는 질문 하기 |

이 예제는 코드베이스에서 TODO 주석을 검색하는 에이전트를 만듭니다: Python TypeScript

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

에이전트 라이프사이클의 주요 지점에서 사용자 정의 코드를 실행합니다. SDK 훅은 콜백 함수를 사용하여 에이전트 동작을 검증, 로깅, 차단 또는 변환합니다. **사용 가능한 훅:** `PreToolUse`, `PostToolUse`, `Stop`, `SessionStart`, `SessionEnd`, `UserPromptSubmit` 등. 이 예제는 모든 파일 변경 사항을 감사 파일에 기록합니다: Python TypeScript

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

[훅에 대해 자세히 알아보기 →](https://code.claude.com/docs/ko/agent-sdk/hooks) 특화된 에이전트를 생성하여 집중된 부작업을 처리합니다. 주 에이전트가 작업을 위임하고 서브에이전트가 결과를 보고합니다. 특화된 지침으로 사용자 정의 에이전트를 정의합니다. 서브에이전트가 Agent 도구를 통해 호출되므로 `allowedTools`에 `Agent`를 포함하십시오: Python TypeScript

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

서브에이전트의 컨텍스트 내의 메시지에는 `parent_tool_use_id` 필드가 포함되어 있어 어떤 메시지가 어떤 서브에이전트 실행에 속하는지 추적할 수 있습니다. [서브에이전트에 대해 자세히 알아보기 →](https://code.claude.com/docs/ko/agent-sdk/subagents) Model Context Protocol을 통해 외부 시스템에 연결합니다: 데이터베이스, 브라우저, API 및 [수백 개 이상](https://github.com/modelcontextprotocol/servers). 이 예제는 [Playwright MCP 서버](https://github.com/microsoft/playwright-mcp)를 연결하여 에이전트에 브라우저 자동화 기능을 제공합니다: Python TypeScript

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

[MCP에 대해 자세히 알아보기 →](https://code.claude.com/docs/ko/agent-sdk/mcp) 에이전트가 사용할 수 있는 도구를 정확히 제어합니다. 안전한 작업을 허용하고, 위험한 작업을 차단하거나, 민감한 작업에 대한 승인을 요구합니다. 대화형 승인 프롬프트 및 `AskUserQuestion` 도구는 [승인 및 사용자 입력 처리](https://code.claude.com/docs/ko/agent-sdk/user-input)를 참조하십시오. 이 예제는 코드를 분석할 수 있지만 수정할 수 없는 읽기 전용 에이전트를 만듭니다. `allowed_tools`는 `Read`, `Glob` 및 `Grep`을 사전 승인합니다. Python TypeScript

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

[권한에 대해 자세히 알아보기 →](https://code.claude.com/docs/ko/agent-sdk/permissions) 여러 교환에 걸쳐 컨텍스트를 유지합니다. Claude는 읽은 파일, 수행한 분석 및 대화 기록을 기억합니다. 나중에 세션을 재개하거나 다양한 접근 방식을 탐색하기 위해 포크합니다. 이 예제는 첫 번째 쿼리에서 세션 ID를 캡처한 다음 전체 컨텍스트로 계속하기 위해 재개합니다: Python TypeScript

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

[세션에 대해 자세히 알아보기 →](https://code.claude.com/docs/ko/agent-sdk/sessions)


### [​](https://code.claude.com/docs/ko/agent-sdk/overview#claude-code-%EA%B8%B0%EB%8A%A5) Claude Code 기능


SDK는 또한 Claude Code의 파일 시스템 기반 구성을 지원합니다. 기본 옵션을 사용하면 SDK는 작업 디렉토리의 `.claude/` 및 `~/.claude/`에서 이를 로드합니다. 로드되는 소스를 제한하려면 옵션에서 `setting_sources`(Python) 또는 `settingSources`(TypeScript)를 설정하십시오.


| 기능 | 설명 | 위치 |
| --- | --- | --- |
| [Skills](https://code.claude.com/docs/ko/agent-sdk/skills) | Markdown에 정의된 특화된 기능 | `.claude/skills/*/SKILL.md` |
| [Slash commands](https://code.claude.com/docs/ko/agent-sdk/slash-commands) | 일반적인 작업을 위한 사용자 정의 명령 | `.claude/commands/*.md` |
| [Memory](https://code.claude.com/docs/ko/agent-sdk/modifying-system-prompts) | 프로젝트 컨텍스트 및 지침 | `CLAUDE.md` 또는 `.claude/CLAUDE.md` |
| [Plugins](https://code.claude.com/docs/ko/agent-sdk/plugins) | 사용자 정의 명령, 에이전트 및 MCP 서버로 확장 | `plugins` 옵션을 통한 프로그래밍 방식 |


## [​](https://code.claude.com/docs/ko/agent-sdk/overview#agent-sdk%EB%A5%BC-%EB%8B%A4%EB%A5%B8-claude-%EB%8F%84%EA%B5%AC%EC%99%80-%EB%B9%84%EA%B5%90) Agent SDK를 다른 Claude 도구와 비교


Claude 플랫폼은 Claude로 구축하는 여러 방법을 제공합니다. Agent SDK가 어떻게 적합한지 다음과 같습니다:


- Agent SDK vs Client SDK
- Agent SDK vs Claude Code CLI

[Anthropic Client SDK](https://platform.claude.com/docs/en/api/client-sdks)는 직접 API 액세스를 제공합니다: 프롬프트를 보내고 도구 실행을 직접 구현합니다. **Agent SDK**는 기본 제공 도구 실행이 있는 Claude를 제공합니다. Client SDK를 사용하면 도구 루프를 구현합니다. Agent SDK를 사용하면 Claude가 처리합니다: Python TypeScript

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

동일한 기능, 다른 인터페이스:

| 사용 사례 | 최선의 선택 |
| --- | --- |
| 대화형 개발 | CLI |
| CI/CD 파이프라인 | SDK |
| 사용자 정의 애플리케이션 | SDK |
| 일회성 작업 | CLI |
| 프로덕션 자동화 | SDK |

많은 팀이 둘 다 사용합니다: 일일 개발을 위한 CLI, 프로덕션을 위한 SDK. 워크플로우는 둘 사이에서 직접 변환됩니다.


## [​](https://code.claude.com/docs/ko/agent-sdk/overview#%EB%B3%80%EA%B2%BD-%EB%A1%9C%EA%B7%B8) 변경 로그


SDK 업데이트, 버그 수정 및 새로운 기능에 대한 전체 변경 로그를 보십시오:


- **TypeScript SDK**: [CHANGELOG.md 보기](https://github.com/anthropics/claude-agent-sdk-typescript/blob/main/CHANGELOG.md)
- **Python SDK**: [CHANGELOG.md 보기](https://github.com/anthropics/claude-agent-sdk-python/blob/main/CHANGELOG.md)


## [​](https://code.claude.com/docs/ko/agent-sdk/overview#%EB%B2%84%EA%B7%B8-%EB%B3%B4%EA%B3%A0) 버그 보고


Agent SDK에서 버그 또는 문제가 발생하면:


- **TypeScript SDK**: [GitHub에서 문제 보고](https://github.com/anthropics/claude-agent-sdk-typescript/issues)
- **Python SDK**: [GitHub에서 문제 보고](https://github.com/anthropics/claude-agent-sdk-python/issues)


## [​](https://code.claude.com/docs/ko/agent-sdk/overview#%EB%B8%8C%EB%9E%9C%EB%94%A9-%EC%A7%80%EC%B9%A8) 브랜딩 지침


Claude Agent SDK를 통합하는 파트너의 경우 Claude 브랜딩 사용은 선택 사항입니다. 제품에서 Claude를 참조할 때:
**허용됨:**


- “Claude Agent” (드롭다운 메뉴에 권장)
- “Claude” (이미 “Agents”로 표시된 메뉴 내)
- ” Powered by Claude” (기존 에이전트 이름이 있는 경우)


**허용되지 않음:**


- “Claude Code” 또는 “Claude Code Agent”
- Claude Code 브랜드 ASCII 아트 또는 Claude Code를 모방하는 시각적 요소


제품은 자체 브랜딩을 유지해야 하며 Claude Code 또는 Anthropic 제품으로 보이지 않아야 합니다. 브랜딩 준수에 대한 질문은 Anthropic [영업팀](https://www.anthropic.com/contact-sales)에 문의하십시오.


## [​](https://code.claude.com/docs/ko/agent-sdk/overview#%EB%9D%BC%EC%9D%B4%EC%84%A0%EC%8A%A4-%EB%B0%8F-%EC%95%BD%EA%B4%80) 라이선스 및 약관


Claude Agent SDK의 사용은 [Anthropic의 상용 서비스 약관](https://www.anthropic.com/legal/commercial-terms)에 의해 관리되며, 이는 자신의 고객 및 최종 사용자가 사용할 수 있도록 제공하는 제품 및 서비스를 강화하기 위해 사용할 때도 포함됩니다. 단, 특정 구성 요소 또는 종속성이 해당 구성 요소의 LICENSE 파일에 표시된 대로 다른 라이선스로 적용되는 경우는 제외합니다.


## [​](https://code.claude.com/docs/ko/agent-sdk/overview#%EB%8B%A4%EC%9D%8C-%EB%8B%A8%EA%B3%84) 다음 단계


## 빠른 시작

몇 분 안에 버그를 찾고 수정하는 에이전트 구축하기

## 예제 에이전트

이메일 어시스턴트, 연구 에이전트 등

## TypeScript SDK

전체 TypeScript API 참조 및 예제

## Python SDK

전체 Python API 참조 및 예제[Claude Code Docs home page](https://code.claude.com/docs/ko/overview)

[Privacy choices](https://code.claude.com/docs/ko/agent-sdk/overview#)

