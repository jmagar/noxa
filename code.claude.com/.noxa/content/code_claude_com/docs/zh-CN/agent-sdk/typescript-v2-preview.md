# TypeScript SDK V2 interface (preview
## ​安装
## ​快速开始
## ​API 参考
## ​功能可用性
## ​反馈
## ​另请参阅







简化的 V2 TypeScript Agent SDK 预览，具有用于多轮对话的基于会话的 send/stream 模式。

V2 interface 是一个 **不稳定的预览版**。在变得稳定之前，API 可能会根据反馈而改变。某些功能（如会话分叉）仅在 [V1 SDK](https://code.claude.com/docs/zh-CN/agent-sdk/typescript) 中可用。
V2 Claude Agent TypeScript SDK 消除了对异步生成器和 yield 协调的需求。这使多轮对话更简单，而不是在各轮之间管理生成器状态，每一轮都是一个单独的 `send()`/ `stream()` 周期。API 表面简化为三个概念：


- `createSession()` / `resumeSession()`：启动或继续对话
- `session.send()`：发送消息
- `session.stream()`：获取响应


## [​](https://code.claude.com/docs/zh-CN/agent-sdk/typescript-v2-preview#%E5%AE%89%E8%A3%85) 安装


V2 interface 包含在现有的 SDK 包中：


```
npm install @anthropic-ai/claude-agent-sdk
```


SDK 为您的平台捆绑了一个本地 Claude Code 二进制文件作为可选依赖项，因此您无需单独安装 Claude Code。


## [​](https://code.claude.com/docs/zh-CN/agent-sdk/typescript-v2-preview#%E5%BF%AB%E9%80%9F%E5%BC%80%E5%A7%8B) 快速开始


### [​](https://code.claude.com/docs/zh-CN/agent-sdk/typescript-v2-preview#%E5%8D%95%E6%AC%A1%E6%8F%90%E7%A4%BA) 单次提示


对于不需要维护会话的简单单轮查询，使用 `unstable_v2_prompt()`。此示例发送一个数学问题并记录答案：


```
import { unstable_v2_prompt } from "@anthropic-ai/claude-agent-sdk";

const result = await unstable_v2_prompt("What is 2 + 2?", {
  model: "claude-opus-4-7"
});
if (result.subtype === "success") {
  console.log(result.result);
}
```


### [​](https://code.claude.com/docs/zh-CN/agent-sdk/typescript-v2-preview#%E5%9F%BA%E6%9C%AC%E4%BC%9A%E8%AF%9D) 基本会话


对于超出单个提示的交互，创建一个会话。V2 将发送和流式传输分为不同的步骤：


- `send()` 分派您的消息
- `stream()` 流式传输响应


这种明确的分离使得在轮次之间添加逻辑变得更容易（例如在发送后续消息之前处理响应）。
下面的示例创建一个会话，向 Claude 发送”Hello!”，并打印文本响应。它使用 [`await using`](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-2.html#using-declarations-and-explicit-resource-management)（TypeScript 5.2+）在块退出时自动关闭会话。您也可以手动调用 `session.close()`。


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


### [​](https://code.claude.com/docs/zh-CN/agent-sdk/typescript-v2-preview#%E5%A4%9A%E8%BD%AE%E5%AF%B9%E8%AF%9D) 多轮对话


会话在多个交换中保持上下文。要继续对话，请在同一会话上再次调用 `send()`。Claude 会记住之前的轮次。
此示例提出一个数学问题，然后提出一个引用前一个答案的后续问题：


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


### [​](https://code.claude.com/docs/zh-CN/agent-sdk/typescript-v2-preview#%E4%BC%9A%E8%AF%9D%E6%81%A2%E5%A4%8D) 会话恢复


如果您有来自之前交互的会话 ID，您可以稍后恢复它。这对于长时间运行的工作流或当您需要在应用程序重新启动时保持对话时很有用。
此示例创建一个会话，存储其 ID，关闭它，然后恢复对话：


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


### [​](https://code.claude.com/docs/zh-CN/agent-sdk/typescript-v2-preview#%E6%B8%85%E7%90%86) 清理


会话可以手动关闭或使用 [`await using`](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-2.html#using-declarations-and-explicit-resource-management)（TypeScript 5.2+ 功能用于自动资源清理）自动关闭。如果您使用的是较旧的 TypeScript 版本或遇到兼容性问题，请改用手动清理。
**自动清理（TypeScript 5.2+）：**


```
import { unstable_v2_createSession } from "@anthropic-ai/claude-agent-sdk";

await using session = unstable_v2_createSession({
  model: "claude-opus-4-7"
});
// Session closes automatically when the block exits
```


**手动清理：**


```
import { unstable_v2_createSession } from "@anthropic-ai/claude-agent-sdk";

const session = unstable_v2_createSession({
  model: "claude-opus-4-7"
});
// ... use the session ...
session.close();
```


## [​](https://code.claude.com/docs/zh-CN/agent-sdk/typescript-v2-preview#api-%E5%8F%82%E8%80%83) API 参考


### [​](https://code.claude.com/docs/zh-CN/agent-sdk/typescript-v2-preview#unstable_v2_createsession) `unstable_v2_createSession()`


为多轮对话创建新会话。


```
function unstable_v2_createSession(options: {
  model: string;
  // Additional options supported
}): SDKSession;
```


### [​](https://code.claude.com/docs/zh-CN/agent-sdk/typescript-v2-preview#unstable_v2_resumesession) `unstable_v2_resumeSession()`


按 ID 恢复现有会话。


```
function unstable_v2_resumeSession(
  sessionId: string,
  options: {
    model: string;
    // Additional options supported
  }
): SDKSession;
```


### [​](https://code.claude.com/docs/zh-CN/agent-sdk/typescript-v2-preview#unstable_v2_prompt) `unstable_v2_prompt()`


用于单轮查询的单次便利函数。


```
function unstable_v2_prompt(
  prompt: string,
  options: {
    model: string;
    // Additional options supported
  }
): Promise<SDKResultMessage>;
```


### [​](https://code.claude.com/docs/zh-CN/agent-sdk/typescript-v2-preview#sdksession-interface) SDKSession interface


```
interface SDKSession {
  readonly sessionId: string;
  send(message: string | SDKUserMessage): Promise<void>;
  stream(): AsyncGenerator<SDKMessage, void>;
  close(): void;
}
```


## [​](https://code.claude.com/docs/zh-CN/agent-sdk/typescript-v2-preview#%E5%8A%9F%E8%83%BD%E5%8F%AF%E7%94%A8%E6%80%A7) 功能可用性


并非所有 V1 功能在 V2 中都可用。以下功能需要使用 [V1 SDK](https://code.claude.com/docs/zh-CN/agent-sdk/typescript)：


- 会话分叉（`forkSession` 选项）
- 某些高级流式输入模式


## [​](https://code.claude.com/docs/zh-CN/agent-sdk/typescript-v2-preview#%E5%8F%8D%E9%A6%88) 反馈


在 V2 interface 变得稳定之前分享您的反馈。通过 [GitHub Issues](https://github.com/anthropics/claude-code/issues) 报告问题和建议。


## [​](https://code.claude.com/docs/zh-CN/agent-sdk/typescript-v2-preview#%E5%8F%A6%E8%AF%B7%E5%8F%82%E9%98%85) 另请参阅


- [TypeScript SDK 参考（V1）](https://code.claude.com/docs/zh-CN/agent-sdk/typescript) - 完整的 V1 SDK 文档
- [SDK 概述](https://code.claude.com/docs/zh-CN/agent-sdk/overview) - 常规 SDK 概念
- [GitHub 上的 V2 示例](https://github.com/anthropics/claude-agent-sdk-demos/tree/main/hello-world-v2) - 工作代码示例[Claude Code Docs home page](https://code.claude.com/docs/zh-CN/overview)

[Privacy choices](https://code.claude.com/docs/zh-CN/agent-sdk/typescript-v2-preview#)

