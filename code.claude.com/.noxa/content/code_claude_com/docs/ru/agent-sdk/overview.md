# Обзор Agent SDK
## ​Начало работы
## ​Возможности
## ​Сравнение Agent SDK с другими инструментами Claude
## ​Журнал изменений
## ​Сообщение об ошибках
## ​Рекомендации по брендингу
## ​Лицензия и условия
## ​Следующие шаги









Создавайте производственные AI-агентов с Claude Code как библиотеку

Claude Code SDK был переименован в Claude Agent SDK. Если вы переходите со старого SDK, см. [Руководство по миграции](https://code.claude.com/docs/ru/agent-sdk/migration-guide).
Создавайте AI-агентов, которые автономно читают файлы, запускают команды, ищут в интернете, редактируют код и многое другое. Agent SDK предоставляет вам те же инструменты, цикл агента и управление контекстом, которые питают Claude Code, программируемые на Python и TypeScript.
Opus 4.7 ( `claude-opus-4-7`) требует Agent SDK v0.2.111 или позже. Если вы видите ошибку API `thinking.type.enabled`, см. [Troubleshooting](https://code.claude.com/docs/ru/agent-sdk/quickstart#troubleshooting).
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


Agent SDK включает встроенные инструменты для чтения файлов, запуска команд и редактирования кода, поэтому ваш агент может начать работу немедленно без необходимости реализации выполнения инструментов. Погрузитесь в быстрый старт или изучите реальных агентов, созданных с помощью SDK:


## Быстрый старт

Создайте агента по исправлению ошибок за несколько минут

## Примеры агентов

Помощник по электронной почте, исследовательский агент и многое другое


## [​](https://code.claude.com/docs/ru/agent-sdk/overview#%D0%BD%D0%B0%D1%87%D0%B0%D0%BB%D0%BE-%D1%80%D0%B0%D0%B1%D0%BE%D1%82%D1%8B) Начало работы


1

Установите SDK


- TypeScript
- Python


```
npm install @anthropic-ai/claude-agent-sdk
```


```
pip install claude-agent-sdk
```

TypeScript SDK поставляется с собственным бинарным файлом Claude Code для вашей платформы в качестве дополнительной зависимости, поэтому вам не нужно устанавливать Claude Code отдельно. 2

Установите ваш API ключ

Получите API ключ из [Console](https://platform.claude.com/), затем установите его как переменную окружения:

```
export ANTHROPIC_API_KEY=your-api-key
```

SDK также поддерживает аутентификацию через сторонних поставщиков API:

- **Amazon Bedrock**: установите переменную окружения `CLAUDE_CODE_USE_BEDROCK=1` и настройте учетные данные AWS
- **Google Vertex AI**: установите переменную окружения `CLAUDE_CODE_USE_VERTEX=1` и настройте учетные данные Google Cloud
- **Microsoft Azure**: установите переменную окружения `CLAUDE_CODE_USE_FOUNDRY=1` и настройте учетные данные Azure

См. руководства по настройке для [Bedrock](https://code.claude.com/docs/ru/amazon-bedrock), [Vertex AI](https://code.claude.com/docs/ru/google-vertex-ai) или [Azure AI Foundry](https://code.claude.com/docs/ru/microsoft-foundry) для получения подробной информации. Если не одобрено ранее, Anthropic не разрешает сторонним разработчикам предлагать вход в claude.ai или ограничения скорости для своих продуктов, включая агентов, созданных на Claude Agent SDK. Вместо этого используйте методы аутентификации по API ключу, описанные в этом документе. 3

Запустите вашего первого агента

Этот пример создает агента, который перечисляет файлы в вашем текущем каталоге, используя встроенные инструменты. Python TypeScript

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


**Готовы к разработке?** Следуйте [Быстрому старту](https://code.claude.com/docs/ru/agent-sdk/quickstart), чтобы создать агента, который находит и исправляет ошибки за несколько минут.


## [​](https://code.claude.com/docs/ru/agent-sdk/overview#%D0%B2%D0%BE%D0%B7%D0%BC%D0%BE%D0%B6%D0%BD%D0%BE%D1%81%D1%82%D0%B8) Возможности


Все, что делает Claude Code мощным, доступно в SDK:


- Встроенные инструменты
- hooks
- Subagents
- MCP
- Permissions
- Sessions

Ваш агент может читать файлы, запускать команды и искать в кодовых базах из коробки. Ключевые инструменты включают:

| Инструмент | Что он делает |
| --- | --- |
| **Read** | Читать любой файл в рабочем каталоге |
| **Write** | Создавать новые файлы |
| **Edit** | Делать точные правки в существующих файлах |
| **Bash** | Запускать команды терминала, скрипты, операции git |
| **Monitor** | Наблюдать фоновый скрипт и реагировать на каждую строку вывода как на событие |
| **Glob** | Находить файлы по шаблону ( `**/*.ts`, `src/**/*.py`) |
| **Grep** | Искать содержимое файлов с помощью regex |
| **WebSearch** | Искать в интернете текущую информацию |
| **WebFetch** | Получать и анализировать содержимое веб-страниц |
| **[AskUserQuestion](https://code.claude.com/docs/ru/agent-sdk/user-input#handle-clarifying-questions)** | Задавать пользователю уточняющие вопросы с вариантами множественного выбора |

Этот пример создает агента, который ищет в вашей кодовой базе комментарии TODO: Python TypeScript

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

Запускайте пользовательский код в ключевых точках жизненного цикла агента. SDK hooks используют функции обратного вызова для проверки, логирования, блокирования или преобразования поведения агента. **Доступные hooks:** `PreToolUse`, `PostToolUse`, `Stop`, `SessionStart`, `SessionEnd`, `UserPromptSubmit` и другие. Этот пример логирует все изменения файлов в файл аудита: Python TypeScript

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

[Узнайте больше о hooks →](https://code.claude.com/docs/ru/agent-sdk/hooks) Создавайте специализированных агентов для обработки сосредоточенных подзадач. Ваш основной агент делегирует работу, а подагенты сообщают результаты. Определите пользовательских агентов со специализированными инструкциями. Включите `Agent` в `allowedTools`, так как подагенты вызываются через инструмент Agent: Python TypeScript

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

Сообщения из контекста подагента включают поле `parent_tool_use_id`, позволяющее отследить, какие сообщения принадлежат какому выполнению подагента. [Узнайте больше о subagents →](https://code.claude.com/docs/ru/agent-sdk/subagents) Подключайтесь к внешним системам через Model Context Protocol: базы данных, браузеры, API и [сотни других](https://github.com/modelcontextprotocol/servers). Этот пример подключает [Playwright MCP server](https://github.com/microsoft/playwright-mcp), чтобы дать вашему агенту возможности автоматизации браузера: Python TypeScript

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

[Узнайте больше о MCP →](https://code.claude.com/docs/ru/agent-sdk/mcp) Контролируйте точно, какие инструменты может использовать ваш агент. Разрешите безопасные операции, заблокируйте опасные или требуйте одобрения для чувствительных действий. Для интерактивных подсказок одобрения и инструмента `AskUserQuestion`, см. [Обработка одобрений и ввода пользователя](https://code.claude.com/docs/ru/agent-sdk/user-input). Этот пример создает агента только для чтения, который может анализировать, но не изменять код. `allowed_tools` предварительно одобряет `Read`, `Glob` и `Grep`. Python TypeScript

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

[Узнайте больше о permissions →](https://code.claude.com/docs/ru/agent-sdk/permissions) Сохраняйте контекст между несколькими обменами. Claude помнит прочитанные файлы, выполненный анализ и историю разговора. Возобновляйте сеансы позже или разветвляйте их, чтобы исследовать различные подходы. Этот пример захватывает ID сеанса из первого запроса, затем возобновляет работу с полным контекстом: Python TypeScript

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

[Узнайте больше о sessions →](https://code.claude.com/docs/ru/agent-sdk/sessions)


### [​](https://code.claude.com/docs/ru/agent-sdk/overview#%D1%84%D1%83%D0%BD%D0%BA%D1%86%D0%B8%D0%B8-claude-code) Функции Claude Code


SDK также поддерживает конфигурацию на основе файловой системы Claude Code. С параметрами по умолчанию SDK загружает их из `.claude/` в вашем рабочем каталоге и `~/.claude/`. Чтобы ограничить, какие источники загружаются, установите `setting_sources` (Python) или `settingSources` (TypeScript) в ваших параметрах.


| Функция | Описание | Местоположение |
| --- | --- | --- |
| [Skills](https://code.claude.com/docs/ru/agent-sdk/skills) | Специализированные возможности, определенные в Markdown | `.claude/skills/*/SKILL.md` |
| [Slash commands](https://code.claude.com/docs/ru/agent-sdk/slash-commands) | Пользовательские команды для общих задач | `.claude/commands/*.md` |
| [Memory](https://code.claude.com/docs/ru/agent-sdk/modifying-system-prompts) | Контекст проекта и инструкции | `CLAUDE.md` или `.claude/CLAUDE.md` |
| [Plugins](https://code.claude.com/docs/ru/agent-sdk/plugins) | Расширяйте пользовательскими командами, агентами и MCP серверами | Программно через опцию `plugins` |


## [​](https://code.claude.com/docs/ru/agent-sdk/overview#%D1%81%D1%80%D0%B0%D0%B2%D0%BD%D0%B5%D0%BD%D0%B8%D0%B5-agent-sdk-%D1%81-%D0%B4%D1%80%D1%83%D0%B3%D0%B8%D0%BC%D0%B8-%D0%B8%D0%BD%D1%81%D1%82%D1%80%D1%83%D0%BC%D0%B5%D0%BD%D1%82%D0%B0%D0%BC%D0%B8-claude) Сравнение Agent SDK с другими инструментами Claude


Claude Platform предлагает несколько способов разработки с Claude. Вот как Agent SDK вписывается:


- Agent SDK vs Client SDK
- Agent SDK vs Claude Code CLI

[Anthropic Client SDK](https://platform.claude.com/docs/en/api/client-sdks) дает вам прямой доступ к API: вы отправляете подсказки и реализуете выполнение инструментов самостоятельно. **Agent SDK** дает вам Claude со встроенным выполнением инструментов. С Client SDK вы реализуете цикл инструментов. С Agent SDK Claude обрабатывает это: Python TypeScript

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

Те же возможности, другой интерфейс:

| Вариант использования | Лучший выбор |
| --- | --- |
| Интерактивная разработка | CLI |
| CI/CD конвейеры | SDK |
| Пользовательские приложения | SDK |
| Одноразовые задачи | CLI |
| Производственная автоматизация | SDK |

Многие команды используют оба: CLI для ежедневной разработки, SDK для производства. Рабочие процессы напрямую переводятся между ними.


## [​](https://code.claude.com/docs/ru/agent-sdk/overview#%D0%B6%D1%83%D1%80%D0%BD%D0%B0%D0%BB-%D0%B8%D0%B7%D0%BC%D0%B5%D0%BD%D0%B5%D0%BD%D0%B8%D0%B9) Журнал изменений


Просмотрите полный журнал изменений для обновлений SDK, исправлений ошибок и новых функций:


- **TypeScript SDK**: [просмотреть CHANGELOG.md](https://github.com/anthropics/claude-agent-sdk-typescript/blob/main/CHANGELOG.md)
- **Python SDK**: [просмотреть CHANGELOG.md](https://github.com/anthropics/claude-agent-sdk-python/blob/main/CHANGELOG.md)


## [​](https://code.claude.com/docs/ru/agent-sdk/overview#%D1%81%D0%BE%D0%BE%D0%B1%D1%89%D0%B5%D0%BD%D0%B8%D0%B5-%D0%BE%D0%B1-%D0%BE%D1%88%D0%B8%D0%B1%D0%BA%D0%B0%D1%85) Сообщение об ошибках


Если вы столкнулись с ошибками или проблемами с Agent SDK:


- **TypeScript SDK**: [сообщить об ошибках на GitHub](https://github.com/anthropics/claude-agent-sdk-typescript/issues)
- **Python SDK**: [сообщить об ошибках на GitHub](https://github.com/anthropics/claude-agent-sdk-python/issues)


## [​](https://code.claude.com/docs/ru/agent-sdk/overview#%D1%80%D0%B5%D0%BA%D0%BE%D0%BC%D0%B5%D0%BD%D0%B4%D0%B0%D1%86%D0%B8%D0%B8-%D0%BF%D0%BE-%D0%B1%D1%80%D0%B5%D0%BD%D0%B4%D0%B8%D0%BD%D0%B3%D1%83) Рекомендации по брендингу


Для партнеров, интегрирующих Claude Agent SDK, использование брендинга Claude является необязательным. При ссылке на Claude в вашем продукте:
**Разрешено:**


- “Claude Agent” (предпочтительно для раскрывающихся меню)
- “Claude” (когда находится в меню, уже помеченном как “Agents”)
- ” Powered by Claude” (если у вас есть существующее имя агента)


**Не разрешено:**


- “Claude Code” или “Claude Code Agent”
- ASCII-арт с брендингом Claude Code или визуальные элементы, которые имитируют Claude Code


Ваш продукт должен сохранять свой собственный брендинг и не должен выглядеть как Claude Code или любой продукт Anthropic. Для вопросов о соответствии брендингу свяжитесь с командой Anthropic [sales team](https://www.anthropic.com/contact-sales).


## [​](https://code.claude.com/docs/ru/agent-sdk/overview#%D0%BB%D0%B8%D1%86%D0%B5%D0%BD%D0%B7%D0%B8%D1%8F-%D0%B8-%D1%83%D1%81%D0%BB%D0%BE%D0%B2%D0%B8%D1%8F) Лицензия и условия


Использование Claude Agent SDK регулируется [Коммерческими условиями обслуживания Anthropic](https://www.anthropic.com/legal/commercial-terms), включая случаи, когда вы используете его для питания продуктов и услуг, которые вы предоставляете своим собственным клиентам и конечным пользователям, за исключением случаев, когда конкретный компонент или зависимость покрыты другой лицензией, как указано в файле LICENSE этого компонента.


## [​](https://code.claude.com/docs/ru/agent-sdk/overview#%D1%81%D0%BB%D0%B5%D0%B4%D1%83%D1%8E%D1%89%D0%B8%D0%B5-%D1%88%D0%B0%D0%B3%D0%B8) Следующие шаги


## Быстрый старт

Создайте агента, который находит и исправляет ошибки за несколько минут

## Примеры агентов

Помощник по электронной почте, исследовательский агент и многое другое

## TypeScript SDK

Полная справка API TypeScript и примеры

## Python SDK

Полная справка API Python и примеры[Claude Code Docs home page](https://code.claude.com/docs/ru/overview)

[Privacy choices](https://code.claude.com/docs/ru/agent-sdk/overview#)

