# Быстрый старт
## ​Предварительные требования
## ​Настройка
## ​Создайте файл с ошибками
## ​Создайте агента, который находит и исправляет ошибки
## ​Ключевые концепции
## ​Устранение неполадок
## ​Следующие шаги








Начните работу с Python или TypeScript Agent SDK для создания AI-агентов, которые работают автономно

Используйте Agent SDK для создания AI-агента, который читает ваш код, находит ошибки и исправляет их, всё без ручного вмешательства.
**Что вы будете делать:**


1. Настроить проект с Agent SDK
2. Создать файл с некорректным кодом
3. Запустить агента, который автоматически находит и исправляет ошибки


## [​](https://code.claude.com/docs/ru/agent-sdk/quickstart#%D0%BF%D1%80%D0%B5%D0%B4%D0%B2%D0%B0%D1%80%D0%B8%D1%82%D0%B5%D0%BB%D1%8C%D0%BD%D1%8B%D0%B5-%D1%82%D1%80%D0%B5%D0%B1%D0%BE%D0%B2%D0%B0%D0%BD%D0%B8%D1%8F) Предварительные требования


- **Node.js 18+** или **Python 3.10+**
- **Учётная запись Anthropic** ([зарегистрируйтесь здесь](https://platform.claude.com/))


## [​](https://code.claude.com/docs/ru/agent-sdk/quickstart#%D0%BD%D0%B0%D1%81%D1%82%D1%80%D0%BE%D0%B9%D0%BA%D0%B0) Настройка


1

Создайте папку проекта

Создайте новый каталог для этого быстрого старта:

```
mkdir my-agent && cd my-agent
```

Для собственных проектов вы можете запустить SDK из любой папки; по умолчанию он будет иметь доступ к файлам в этом каталоге и его подкаталогах. 2

Установите SDK

Установите пакет Agent SDK для вашего языка:

- TypeScript
- Python (uv)
- Python (pip)


```
npm install @anthropic-ai/claude-agent-sdk
```

[uv Python package manager](https://docs.astral.sh/uv/) — это быстрый менеджер пакетов Python, который автоматически управляет виртуальными окружениями:

```
uv init && uv add claude-agent-sdk
```

Сначала создайте виртуальное окружение, затем установите:

```
python3 -m venv .venv && source .venv/bin/activate
pip3 install claude-agent-sdk
```

TypeScript SDK включает нативный бинарный файл Claude Code для вашей платформы в качестве опциональной зависимости, поэтому вам не нужно устанавливать Claude Code отдельно. 3

Установите ваш API ключ

Получите API ключ из [Claude Console](https://platform.claude.com/), затем создайте файл `.env` в каталоге вашего проекта:

```
ANTHROPIC_API_KEY=your-api-key
```

SDK также поддерживает аутентификацию через сторонних поставщиков API:

- **Amazon Bedrock**: установите переменную окружения `CLAUDE_CODE_USE_BEDROCK=1` и настройте учётные данные AWS
- **Google Vertex AI**: установите переменную окружения `CLAUDE_CODE_USE_VERTEX=1` и настройте учётные данные Google Cloud
- **Microsoft Azure**: установите переменную окружения `CLAUDE_CODE_USE_FOUNDRY=1` и настройте учётные данные Azure

Подробности см. в руководствах по настройке для [Bedrock](https://code.claude.com/docs/ru/amazon-bedrock), [Vertex AI](https://code.claude.com/docs/ru/google-vertex-ai) или [Azure AI Foundry](https://code.claude.com/docs/ru/microsoft-foundry). Если не было предварительного одобрения, Anthropic не разрешает сторонним разработчикам предлагать вход через claude.ai или ограничения скорости для своих продуктов, включая агентов, созданных на основе Claude Agent SDK. Вместо этого используйте методы аутентификации через API ключ, описанные в этом документе.


## [​](https://code.claude.com/docs/ru/agent-sdk/quickstart#%D1%81%D0%BE%D0%B7%D0%B4%D0%B0%D0%B9%D1%82%D0%B5-%D1%84%D0%B0%D0%B9%D0%BB-%D1%81-%D0%BE%D1%88%D0%B8%D0%B1%D0%BA%D0%B0%D0%BC%D0%B8) Создайте файл с ошибками


Этот быстрый старт проведёт вас через создание агента, который может находить и исправлять ошибки в коде. Сначала вам нужен файл с некоторыми намеренными ошибками для исправления агентом. Создайте `utils.py` в каталоге `my-agent` и вставьте следующий код:


```
def calculate_average(numbers):
    total = 0
    for num in numbers:
        total += num
    return total / len(numbers)


def get_user_name(user):
    return user["name"].upper()
```


Этот код содержит две ошибки:


1. `calculate_average([])` падает с ошибкой деления на ноль
2. `get_user_name(None)` падает с TypeError


## [​](https://code.claude.com/docs/ru/agent-sdk/quickstart#%D1%81%D0%BE%D0%B7%D0%B4%D0%B0%D0%B9%D1%82%D0%B5-%D0%B0%D0%B3%D0%B5%D0%BD%D1%82%D0%B0-%D0%BA%D0%BE%D1%82%D0%BE%D1%80%D1%8B%D0%B9-%D0%BD%D0%B0%D1%85%D0%BE%D0%B4%D0%B8%D1%82-%D0%B8-%D0%B8%D1%81%D0%BF%D1%80%D0%B0%D0%B2%D0%BB%D1%8F%D0%B5%D1%82-%D0%BE%D1%88%D0%B8%D0%B1%D0%BA%D0%B8) Создайте агента, который находит и исправляет ошибки


Создайте `agent.py`, если вы используете Python SDK, или `agent.ts` для TypeScript:
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


Этот код состоит из трёх основных частей:


1. **`query`**: основная точка входа, которая создаёт цикл агента. Она возвращает асинхронный итератор, поэтому вы используете `async for` для потоковой передачи сообщений по мере работы Claude. Полный API см. в справочнике [Python](https://code.claude.com/docs/ru/agent-sdk/python#query) или [TypeScript](https://code.claude.com/docs/ru/agent-sdk/typescript#query) SDK.
2. **`prompt`**: то, что вы хотите, чтобы сделал Claude. Claude определяет, какие инструменты использовать, на основе задачи.
3. **`options`**: конфигурация для агента. В этом примере используется `allowedTools` для предварительного одобрения `Read`, `Edit` и `Glob`, а также `permissionMode: "acceptEdits"` для автоматического одобрения изменений файлов. Другие опции включают `systemPrompt`, `mcpServers` и многое другое. Все опции для [Python](https://code.claude.com/docs/ru/agent-sdk/python#claude-agent-options) или [TypeScript](https://code.claude.com/docs/ru/agent-sdk/typescript#options).


Цикл `async for` продолжает работать, пока Claude думает, вызывает инструменты, наблюдает результаты и решает, что делать дальше. Каждая итерация выдаёт сообщение: рассуждение Claude, вызов инструмента, результат инструмента или окончательный результат. SDK обрабатывает оркестровку (выполнение инструментов, управление контекстом, повторные попытки), поэтому вы просто потребляете поток. Цикл заканчивается, когда Claude завершает задачу или возникает ошибка.
Обработка сообщений внутри цикла фильтрует удобочитаемый вывод. Без фильтрации вы увидите необработанные объекты сообщений, включая инициализацию системы и внутреннее состояние, что полезно для отладки, но в остальном шумно.
В этом примере используется потоковая передача для отображения прогресса в реальном времени. Если вам не нужен живой вывод (например, для фоновых заданий или конвейеров CI), вы можете собрать все сообщения сразу. Подробности см. в разделе [Потоковая передача и однооборотный режим](https://code.claude.com/docs/ru/agent-sdk/streaming-vs-single-mode).


### [​](https://code.claude.com/docs/ru/agent-sdk/quickstart#%D0%B7%D0%B0%D0%BF%D1%83%D1%81%D1%82%D0%B8%D1%82%D0%B5-%D0%B2%D0%B0%D1%88%D0%B5%D0%B3%D0%BE-%D0%B0%D0%B3%D0%B5%D0%BD%D1%82%D0%B0) Запустите вашего агента


Ваш агент готов. Запустите его с помощью следующей команды:


- Python
- TypeScript


```
python3 agent.py
```


```
npx tsx agent.ts
```


После запуска проверьте `utils.py`. Вы увидите защитный код, обрабатывающий пустые списки и нулевых пользователей. Ваш агент автономно:


1. **Прочитал** `utils.py` для понимания кода
2. **Проанализировал** логику и определил граничные случаи, которые вызовут сбой
3. **Отредактировал** файл для добавления надлежащей обработки ошибок


Это то, что отличает Agent SDK: Claude выполняет инструменты напрямую вместо того, чтобы просить вас их реализовать.
Если вы видите “API key not found”, убедитесь, что вы установили переменную окружения `ANTHROPIC_API_KEY` в файле `.env` или окружении оболочки. Подробнее см. в [полном руководстве по устранению неполадок](https://code.claude.com/docs/ru/troubleshooting).


### [​](https://code.claude.com/docs/ru/agent-sdk/quickstart#%D0%BF%D0%BE%D0%BF%D1%80%D0%BE%D0%B1%D1%83%D0%B9%D1%82%D0%B5-%D0%B4%D1%80%D1%83%D0%B3%D0%B8%D0%B5-%D0%BF%D0%BE%D0%B4%D1%81%D0%BA%D0%B0%D0%B7%D0%BA%D0%B8) Попробуйте другие подсказки


Теперь, когда ваш агент настроен, попробуйте некоторые другие подсказки:


- `"Add docstrings to all functions in utils.py"`
- `"Add type hints to all functions in utils.py"`
- `"Create a README.md documenting the functions in utils.py"`


### [​](https://code.claude.com/docs/ru/agent-sdk/quickstart#%D0%BD%D0%B0%D1%81%D1%82%D1%80%D0%BE%D0%B9%D1%82%D0%B5-%D0%B2%D0%B0%D1%88%D0%B5%D0%B3%D0%BE-%D0%B0%D0%B3%D0%B5%D0%BD%D1%82%D0%B0) Настройте вашего агента


Вы можете изменить поведение вашего агента, изменив опции. Вот несколько примеров:
**Добавьте возможность веб-поиска:**
Python TypeScript

```
options = ClaudeAgentOptions(
    allowed_tools=["Read", "Edit", "Glob", "WebSearch"], permission_mode="acceptEdits"
)
```


**Дайте Claude пользовательскую системную подсказку:**
Python TypeScript

```
options = ClaudeAgentOptions(
    allowed_tools=["Read", "Edit", "Glob"],
    permission_mode="acceptEdits",
    system_prompt="You are a senior Python developer. Always follow PEP 8 style guidelines.",
)
```


**Запускайте команды в терминале:**
Python TypeScript

```
options = ClaudeAgentOptions(
    allowed_tools=["Read", "Edit", "Glob", "Bash"], permission_mode="acceptEdits"
)
```


С включённым `Bash` попробуйте: `"Write unit tests for utils.py, run them, and fix any failures"`


## [​](https://code.claude.com/docs/ru/agent-sdk/quickstart#%D0%BA%D0%BB%D1%8E%D1%87%D0%B5%D0%B2%D1%8B%D0%B5-%D0%BA%D0%BE%D0%BD%D1%86%D0%B5%D0%BF%D1%86%D0%B8%D0%B8) Ключевые концепции


**Инструменты** контролируют, что может делать ваш агент:


| Инструменты | Что может делать агент |
| --- | --- |
| `Read`, `Glob`, `Grep` | Анализ только для чтения |
| `Read`, `Edit`, `Glob` | Анализ и изменение кода |
| `Read`, `Edit`, `Bash`, `Glob`, `Grep` | Полная автоматизация |


**Режимы разрешений** контролируют, сколько человеческого надзора вы хотите:


| Режим | Поведение | Вариант использования |
| --- | --- | --- |
| `acceptEdits` | Автоматически одобряет редактирование файлов и общие команды файловой системы, запрашивает другие действия | Надёжные рабочие процессы разработки |
| `dontAsk` | Отклоняет всё, что не в `allowedTools` | Заблокированные автономные агенты |
| `auto` (только TypeScript) | Классификатор модели одобряет или отклоняет каждый вызов инструмента | Автономные агенты с защитой безопасности |
| `bypassPermissions` | Запускает каждый инструмент без подсказок | Изолированный CI, полностью доверенные окружения |
| `default` | Требует обратного вызова `canUseTool` для обработки одобрения | Пользовательские потоки одобрения |


Приведённый выше пример использует режим `acceptEdits`, который автоматически одобряет файловые операции, чтобы агент мог работать без интерактивных подсказок. Если вы хотите запрашивать у пользователей одобрение, используйте режим `default` и предоставьте обратный вызов [`canUseTool`](https://code.claude.com/docs/ru/agent-sdk/user-input), который собирает пользовательский ввод. Для большего контроля см. [Разрешения](https://code.claude.com/docs/ru/agent-sdk/permissions).


## [​](https://code.claude.com/docs/ru/agent-sdk/quickstart#%D1%83%D1%81%D1%82%D1%80%D0%B0%D0%BD%D0%B5%D0%BD%D0%B8%D0%B5-%D0%BD%D0%B5%D0%BF%D0%BE%D0%BB%D0%B0%D0%B4%D0%BE%D0%BA) Устранение неполадок


### [​](https://code.claude.com/docs/ru/agent-sdk/quickstart#%D0%BE%D1%88%D0%B8%D0%B1%D0%BA%D0%B0-api-thinking-type-enabled-%D0%BD%D0%B5-%D0%BF%D0%BE%D0%B4%D0%B4%D0%B5%D1%80%D0%B6%D0%B8%D0%B2%D0%B0%D0%B5%D1%82%D1%81%D1%8F-%D0%B4%D0%BB%D1%8F-%D1%8D%D1%82%D0%BE%D0%B9-%D0%BC%D0%BE%D0%B4%D0%B5%D0%BB%D0%B8) Ошибка API `thinking.type.enabled` не поддерживается для этой модели


Claude Opus 4.7 заменяет `thinking.type.enabled` на `thinking.type.adaptive`. Старые версии Agent SDK падают со следующей ошибкой API при выборе `claude-opus-4-7`:


```
API Error: 400 {"type":"invalid_request_error","message":"\"thinking.type.enabled\" is not supported for this model. Use \"thinking.type.adaptive\" and \"output_config.effort\" to control thinking behavior."}
```


Обновитесь до Agent SDK v0.2.111 или позже для использования Opus 4.7.


## [​](https://code.claude.com/docs/ru/agent-sdk/quickstart#%D1%81%D0%BB%D0%B5%D0%B4%D1%83%D1%8E%D1%89%D0%B8%D0%B5-%D1%88%D0%B0%D0%B3%D0%B8) Следующие шаги


Теперь, когда вы создали своего первого агента, узнайте, как расширить его возможности и адаптировать его к вашему варианту использования:


- **[Разрешения](https://code.claude.com/docs/ru/agent-sdk/permissions)**: контролируйте, что может делать ваш агент и когда ему нужно одобрение
- **[Hooks](https://code.claude.com/docs/ru/agent-sdk/hooks)**: запускайте пользовательский код до или после вызовов инструментов
- **[Сессии](https://code.claude.com/docs/ru/agent-sdk/sessions)**: создавайте многооборотных агентов, которые сохраняют контекст
- **[MCP servers](https://code.claude.com/docs/ru/agent-sdk/mcp)**: подключайтесь к базам данных, браузерам, API и другим внешним системам
- **[Хостинг](https://code.claude.com/docs/ru/agent-sdk/hosting)**: развёртывайте агентов в Docker, облако и CI/CD
- **[Примеры агентов](https://github.com/anthropics/claude-agent-sdk-demos)**: см. полные примеры: помощник по электронной почте, исследовательский агент и многое другое[Claude Code Docs home page](https://code.claude.com/docs/ru/overview)

[Privacy choices](https://code.claude.com/docs/ru/agent-sdk/quickstart#)

