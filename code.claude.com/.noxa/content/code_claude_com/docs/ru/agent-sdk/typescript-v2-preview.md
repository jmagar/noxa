# TypeScript SDK V2 interface (preview
## ​Установка
## ​Быстрый старт
## ​Справочник API
## ​Доступность функций
## ​Обратная связь
## ​См. также







Предпросмотр упрощённого V2 TypeScript Agent SDK с паттернами отправки/потока на основе сессий для многооборотных разговоров.

Интерфейс V2 является **нестабильным предпросмотром**. API могут измениться на основе обратной связи перед тем, как стать стабильными. Некоторые функции, такие как разветвление сессий, доступны только в [V1 SDK](https://code.claude.com/docs/ru/agent-sdk/typescript).
V2 Claude Agent TypeScript SDK устраняет необходимость в асинхронных генераторах и координации yield. Это делает многооборотные разговоры проще — вместо управления состоянием генератора между оборотами, каждый оборот представляет собой отдельный цикл `send()`/ `stream()`. Поверхность API сводится к трём концепциям:


- `createSession()` / `resumeSession()`: Начать или продолжить разговор
- `session.send()`: Отправить сообщение
- `session.stream()`: Получить ответ


## [​](https://code.claude.com/docs/ru/agent-sdk/typescript-v2-preview#%D1%83%D1%81%D1%82%D0%B0%D0%BD%D0%BE%D0%B2%D0%BA%D0%B0) Установка


Интерфейс V2 включён в существующий пакет SDK:


```
npm install @anthropic-ai/claude-agent-sdk
```


SDK поставляется с нативным бинарным файлом Claude Code для вашей платформы в качестве опциональной зависимости, поэтому вам не нужно устанавливать Claude Code отдельно.


## [​](https://code.claude.com/docs/ru/agent-sdk/typescript-v2-preview#%D0%B1%D1%8B%D1%81%D1%82%D1%80%D1%8B%D0%B9-%D1%81%D1%82%D0%B0%D1%80%D1%82) Быстрый старт


### [​](https://code.claude.com/docs/ru/agent-sdk/typescript-v2-preview#%D0%BE%D0%B4%D0%BD%D0%BE%D0%BA%D1%80%D0%B0%D1%82%D0%BD%D1%8B%D0%B9-%D0%B7%D0%B0%D0%BF%D1%80%D0%BE%D1%81) Однократный запрос


Для простых однооборотных запросов, когда вам не нужно поддерживать сессию, используйте `unstable_v2_prompt()`. Этот пример отправляет математический вопрос и логирует ответ:


```
import { unstable_v2_prompt } from "@anthropic-ai/claude-agent-sdk";

const result = await unstable_v2_prompt("What is 2 + 2?", {
  model: "claude-opus-4-7"
});
if (result.subtype === "success") {
  console.log(result.result);
}
```


### [​](https://code.claude.com/docs/ru/agent-sdk/typescript-v2-preview#%D0%B1%D0%B0%D0%B7%D0%BE%D0%B2%D0%B0%D1%8F-%D1%81%D0%B5%D1%81%D1%81%D0%B8%D1%8F) Базовая сессия


Для взаимодействий, выходящих за рамки одного запроса, создайте сессию. V2 разделяет отправку и потоковую передачу на отдельные шаги:


- `send()` отправляет ваше сообщение
- `stream()` передаёт ответ потоком


Это явное разделение облегчает добавление логики между оборотами (например, обработка ответов перед отправкой последующих сообщений).
Пример ниже создаёт сессию, отправляет “Hello!” в Claude и выводит текстовый ответ. Он использует [`await using`](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-2.html#using-declarations-and-explicit-resource-management) (TypeScript 5.2+) для автоматического закрытия сессии при выходе из блока. Вы также можете вызвать `session.close()` вручную.


```
import { unstable_v2_createSession } from "@anthropic-ai/claude-agent-sdk";

await using session = unstable_v2_createSession({
  model: "claude-opus-4-7"
});

await session.send("Hello!");
for await (const msg of session.stream()) {
  // Filter for assistant messages to get human-readable output
  if (msg.type === "assistant") {
    const text = msg.message.content
      .filter((block) => block.type === "text")
      .map((block) => block.text)
      .join("");
    console.log(text);
  }
}
```


### [​](https://code.claude.com/docs/ru/agent-sdk/typescript-v2-preview#%D0%BC%D0%BD%D0%BE%D0%B3%D0%BE%D0%BE%D0%B1%D0%BE%D1%80%D0%BE%D1%82%D0%BD%D1%8B%D0%B9-%D1%80%D0%B0%D0%B7%D0%B3%D0%BE%D0%B2%D0%BE%D1%80) Многооборотный разговор


Сессии сохраняют контекст между несколькими обменами. Чтобы продолжить разговор, вызовите `send()` снова на той же сессии. Claude помнит предыдущие обороты.
Этот пример задаёт математический вопрос, а затем задаёт последующий вопрос, который ссылается на предыдущий ответ:


```
import { unstable_v2_createSession } from "@anthropic-ai/claude-agent-sdk";

await using session = unstable_v2_createSession({
  model: "claude-opus-4-7"
});

// Turn 1
await session.send("What is 5 + 3?");
for await (const msg of session.stream()) {
  // Filter for assistant messages to get human-readable output
  if (msg.type === "assistant") {
    const text = msg.message.content
      .filter((block) => block.type === "text")
      .map((block) => block.text)
      .join("");
    console.log(text);
  }
}

// Turn 2
await session.send("Multiply that by 2");
for await (const msg of session.stream()) {
  if (msg.type === "assistant") {
    const text = msg.message.content
      .filter((block) => block.type === "text")
      .map((block) => block.text)
      .join("");
    console.log(text);
  }
}
```


### [​](https://code.claude.com/docs/ru/agent-sdk/typescript-v2-preview#%D0%B2%D0%BE%D0%B7%D0%BE%D0%B1%D0%BD%D0%BE%D0%B2%D0%BB%D0%B5%D0%BD%D0%B8%D0%B5-%D1%81%D0%B5%D1%81%D1%81%D0%B8%D0%B8) Возобновление сессии


Если у вас есть ID сессии из предыдущего взаимодействия, вы можете возобновить её позже. Это полезно для долгоживущих рабочих процессов или когда вам нужно сохранить разговоры между перезагрузками приложения.
Этот пример создаёт сессию, сохраняет её ID, закрывает её, а затем возобновляет разговор:


```
import {
  unstable_v2_createSession,
  unstable_v2_resumeSession,
  type SDKMessage
} from "@anthropic-ai/claude-agent-sdk";

// Helper to extract text from assistant messages
function getAssistantText(msg: SDKMessage): string | null {
  if (msg.type !== "assistant") return null;
  return msg.message.content
    .filter((block) => block.type === "text")
    .map((block) => block.text)
    .join("");
}

// Create initial session and have a conversation
const session = unstable_v2_createSession({
  model: "claude-opus-4-7"
});

await session.send("Remember this number: 42");

// Get the session ID from any received message
let sessionId: string | undefined;
for await (const msg of session.stream()) {
  sessionId = msg.session_id;
  const text = getAssistantText(msg);
  if (text) console.log("Initial response:", text);
}

console.log("Session ID:", sessionId);
session.close();

// Later: resume the session using the stored ID
await using resumedSession = unstable_v2_resumeSession(sessionId!, {
  model: "claude-opus-4-7"
});

await resumedSession.send("What number did I ask you to remember?");
for await (const msg of resumedSession.stream()) {
  const text = getAssistantText(msg);
  if (text) console.log("Resumed response:", text);
}
```


### [​](https://code.claude.com/docs/ru/agent-sdk/typescript-v2-preview#%D0%BE%D1%87%D0%B8%D1%81%D1%82%D0%BA%D0%B0) Очистка


Сессии можно закрывать вручную или автоматически, используя [`await using`](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-2.html#using-declarations-and-explicit-resource-management), функцию TypeScript 5.2+ для автоматической очистки ресурсов. Если вы используете более старую версию TypeScript или столкнулись с проблемами совместимости, используйте вместо этого ручную очистку.
**Автоматическая очистка (TypeScript 5.2+):**


```
import { unstable_v2_createSession } from "@anthropic-ai/claude-agent-sdk";

await using session = unstable_v2_createSession({
  model: "claude-opus-4-7"
});
// Session closes automatically when the block exits
```


**Ручная очистка:**


```
import { unstable_v2_createSession } from "@anthropic-ai/claude-agent-sdk";

const session = unstable_v2_createSession({
  model: "claude-opus-4-7"
});
// ... use the session ...
session.close();
```


## [​](https://code.claude.com/docs/ru/agent-sdk/typescript-v2-preview#%D1%81%D0%BF%D1%80%D0%B0%D0%B2%D0%BE%D1%87%D0%BD%D0%B8%D0%BA-api) Справочник API


### [​](https://code.claude.com/docs/ru/agent-sdk/typescript-v2-preview#unstable_v2_createsession) `unstable_v2_createSession()`


Создаёт новую сессию для многооборотных разговоров.


```
function unstable_v2_createSession(options: {
  model: string;
  // Additional options supported
}): SDKSession;
```


### [​](https://code.claude.com/docs/ru/agent-sdk/typescript-v2-preview#unstable_v2_resumesession) `unstable_v2_resumeSession()`


Возобновляет существующую сессию по ID.


```
function unstable_v2_resumeSession(
  sessionId: string,
  options: {
    model: string;
    // Additional options supported
  }
): SDKSession;
```


### [​](https://code.claude.com/docs/ru/agent-sdk/typescript-v2-preview#unstable_v2_prompt) `unstable_v2_prompt()`


Однократная удобная функция для однооборотных запросов.


```
function unstable_v2_prompt(
  prompt: string,
  options: {
    model: string;
    // Additional options supported
  }
): Promise<SDKResultMessage>;
```


### [​](https://code.claude.com/docs/ru/agent-sdk/typescript-v2-preview#%D0%B8%D0%BD%D1%82%D0%B5%D1%80%D1%84%D0%B5%D0%B9%D1%81-sdksession) Интерфейс SDKSession


```
interface SDKSession {
  readonly sessionId: string;
  send(message: string | SDKUserMessage): Promise<void>;
  stream(): AsyncGenerator<SDKMessage, void>;
  close(): void;
}
```


## [​](https://code.claude.com/docs/ru/agent-sdk/typescript-v2-preview#%D0%B4%D0%BE%D1%81%D1%82%D1%83%D0%BF%D0%BD%D0%BE%D1%81%D1%82%D1%8C-%D1%84%D1%83%D0%BD%D0%BA%D1%86%D0%B8%D0%B9) Доступность функций


Не все функции V1 доступны в V2 пока. Следующие требуют использования [V1 SDK](https://code.claude.com/docs/ru/agent-sdk/typescript):


- Разветвление сессий (опция `forkSession`)
- Некоторые продвинутые паттерны потокового ввода


## [​](https://code.claude.com/docs/ru/agent-sdk/typescript-v2-preview#%D0%BE%D0%B1%D1%80%D0%B0%D1%82%D0%BD%D0%B0%D1%8F-%D1%81%D0%B2%D1%8F%D0%B7%D1%8C) Обратная связь


Поделитесь своей обратной связью по интерфейсу V2 перед тем, как он станет стабильным. Сообщайте о проблемах и предложениях через [GitHub Issues](https://github.com/anthropics/claude-code/issues).


## [​](https://code.claude.com/docs/ru/agent-sdk/typescript-v2-preview#%D1%81%D0%BC-%D1%82%D0%B0%D0%BA%D0%B6%D0%B5) См. также


- [Справочник TypeScript SDK (V1)](https://code.claude.com/docs/ru/agent-sdk/typescript) - Полная документация V1 SDK
- [Обзор SDK](https://code.claude.com/docs/ru/agent-sdk/overview) - Общие концепции SDK
- [Примеры V2 на GitHub](https://github.com/anthropics/claude-agent-sdk-demos/tree/main/hello-world-v2) - Рабочие примеры кода[Claude Code Docs home page](https://code.claude.com/docs/ru/overview)

[Privacy choices](https://code.claude.com/docs/ru/agent-sdk/typescript-v2-preview#)

