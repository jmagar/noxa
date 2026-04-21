# TypeScript SDK V2 介面（預覽
## ​安裝
## ​快速開始
## ​API 參考
## ​功能可用性
## ​反饋
## ​另請參閱







簡化的 V2 TypeScript Agent SDK 預覽，具有用於多輪對話的基於會話的 send/stream 模式。

V2 介面是一個 **不穩定的預覽版本**。API 可能會根據反饋進行更改，然後才能變成穩定版本。某些功能（如會話分叉）僅在 [V1 SDK](https://code.claude.com/docs/zh-TW/agent-sdk/typescript) 中可用。
V2 Claude Agent TypeScript SDK 消除了對非同步生成器和 yield 協調的需求。這使多輪對話變得更簡單，而不是在各輪之間管理生成器狀態，每一輪都是一個單獨的 `send()`/ `stream()` 週期。API 表面縮減為三個概念：


- `createSession()` / `resumeSession()`：開始或繼續對話
- `session.send()`：發送訊息
- `session.stream()`：取得回應


## [​](https://code.claude.com/docs/zh-TW/agent-sdk/typescript-v2-preview#%E5%AE%89%E8%A3%9D) 安裝


V2 介面包含在現有的 SDK 套件中：


```
npm install @anthropic-ai/claude-agent-sdk
```


SDK 為您的平台捆綁了一個原生 Claude Code 二進位檔案作為可選依賴項，因此您無需單獨安裝 Claude Code。


## [​](https://code.claude.com/docs/zh-TW/agent-sdk/typescript-v2-preview#%E5%BF%AB%E9%80%9F%E9%96%8B%E5%A7%8B) 快速開始


### [​](https://code.claude.com/docs/zh-TW/agent-sdk/typescript-v2-preview#%E5%96%AE%E6%AC%A1%E6%8F%90%E7%A4%BA) 單次提示


對於不需要維護會話的簡單單輪查詢，請使用 `unstable_v2_prompt()`。此範例發送一個數學問題並記錄答案：


```
import { unstable_v2_prompt } from "@anthropic-ai/claude-agent-sdk";

const result = await unstable_v2_prompt("What is 2 + 2?", {
  model: "claude-opus-4-7"
});
if (result.subtype === "success") {
  console.log(result.result);
}
```


### [​](https://code.claude.com/docs/zh-TW/agent-sdk/typescript-v2-preview#%E5%9F%BA%E6%9C%AC%E6%9C%83%E8%A9%B1) 基本會話


對於超出單個提示的互動，請建立一個會話。V2 將發送和串流分為不同的步驟：


- `send()` 分派您的訊息
- `stream()` 串流回應


這種明確的分離使得在輪次之間添加邏輯變得更容易（例如在發送後續訊息之前處理回應）。
下面的範例建立一個會話，向 Claude 發送「Hello!」，並列印文字回應。它使用 [`await using`](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-2.html#using-declarations-and-explicit-resource-management)（TypeScript 5.2+）在區塊退出時自動關閉會話。您也可以手動呼叫 `session.close()`。


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


### [​](https://code.claude.com/docs/zh-TW/agent-sdk/typescript-v2-preview#%E5%A4%9A%E8%BC%AA%E5%B0%8D%E8%A9%B1) 多輪對話


會話在多次交換中保持上下文。要繼續對話，請在同一會話上再次呼叫 `send()`。Claude 會記住之前的輪次。
此範例詢問一個數學問題，然後詢問一個引用先前答案的後續問題：


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


### [​](https://code.claude.com/docs/zh-TW/agent-sdk/typescript-v2-preview#%E6%9C%83%E8%A9%B1%E6%81%A2%E5%BE%A9) 會話恢復


如果您有來自先前互動的會話 ID，您可以稍後恢復它。這對於長時間運行的工作流程或當您需要在應用程式重新啟動時保持對話時很有用。
此範例建立一個會話，儲存其 ID，關閉它，然後恢復對話：


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


### [​](https://code.claude.com/docs/zh-TW/agent-sdk/typescript-v2-preview#%E6%B8%85%E7%90%86) 清理


會話可以手動關閉或使用 [`await using`](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-2.html#using-declarations-and-explicit-resource-management)（TypeScript 5.2+ 功能用於自動資源清理）自動關閉。如果您使用的是較舊的 TypeScript 版本或遇到相容性問題，請改用手動清理。
**自動清理（TypeScript 5.2+）：**


```
import { unstable_v2_createSession } from "@anthropic-ai/claude-agent-sdk";

await using session = unstable_v2_createSession({
  model: "claude-opus-4-7"
});
// Session closes automatically when the block exits
```


**手動清理：**


```
import { unstable_v2_createSession } from "@anthropic-ai/claude-agent-sdk";

const session = unstable_v2_createSession({
  model: "claude-opus-4-7"
});
// ... use the session ...
session.close();
```


## [​](https://code.claude.com/docs/zh-TW/agent-sdk/typescript-v2-preview#api-%E5%8F%83%E8%80%83) API 參考


### [​](https://code.claude.com/docs/zh-TW/agent-sdk/typescript-v2-preview#unstable_v2_createsession) `unstable_v2_createSession()`


為多輪對話建立新會話。


```
function unstable_v2_createSession(options: {
  model: string;
  // Additional options supported
}): SDKSession;
```


### [​](https://code.claude.com/docs/zh-TW/agent-sdk/typescript-v2-preview#unstable_v2_resumesession) `unstable_v2_resumeSession()`


按 ID 恢復現有會話。


```
function unstable_v2_resumeSession(
  sessionId: string,
  options: {
    model: string;
    // Additional options supported
  }
): SDKSession;
```


### [​](https://code.claude.com/docs/zh-TW/agent-sdk/typescript-v2-preview#unstable_v2_prompt) `unstable_v2_prompt()`


用於單輪查詢的單次便利函數。


```
function unstable_v2_prompt(
  prompt: string,
  options: {
    model: string;
    // Additional options supported
  }
): Promise<SDKResultMessage>;
```


### [​](https://code.claude.com/docs/zh-TW/agent-sdk/typescript-v2-preview#sdksession-%E4%BB%8B%E9%9D%A2) SDKSession 介面


```
interface SDKSession {
  readonly sessionId: string;
  send(message: string | SDKUserMessage): Promise<void>;
  stream(): AsyncGenerator<SDKMessage, void>;
  close(): void;
}
```


## [​](https://code.claude.com/docs/zh-TW/agent-sdk/typescript-v2-preview#%E5%8A%9F%E8%83%BD%E5%8F%AF%E7%94%A8%E6%80%A7) 功能可用性


並非所有 V1 功能在 V2 中都可用。以下功能需要使用 [V1 SDK](https://code.claude.com/docs/zh-TW/agent-sdk/typescript)：


- 會話分叉（`forkSession` 選項）
- 某些進階串流輸入模式


## [​](https://code.claude.com/docs/zh-TW/agent-sdk/typescript-v2-preview#%E5%8F%8D%E9%A5%8B) 反饋


在 V2 介面變成穩定版本之前分享您的反饋。通過 [GitHub Issues](https://github.com/anthropics/claude-code/issues) 報告問題和建議。


## [​](https://code.claude.com/docs/zh-TW/agent-sdk/typescript-v2-preview#%E5%8F%A6%E8%AB%8B%E5%8F%83%E9%96%B1) 另請參閱


- [TypeScript SDK 參考（V1）](https://code.claude.com/docs/zh-TW/agent-sdk/typescript) - 完整的 V1 SDK 文件
- [SDK 概述](https://code.claude.com/docs/zh-TW/agent-sdk/overview) - 一般 SDK 概念
- [GitHub 上的 V2 範例](https://github.com/anthropics/claude-agent-sdk-demos/tree/main/hello-world-v2) - 工作程式碼範例[Claude Code Docs home page](https://code.claude.com/docs/zh-TW/overview)

[Privacy choices](https://code.claude.com/docs/zh-TW/agent-sdk/typescript-v2-preview#)

