# TypeScript SDK V2 인터페이스 (미리보기
## ​설치
## ​빠른 시작
## ​API 참조
## ​기능 가용성
## ​피드백
## ​참고 항목







세션 기반 send/stream 패턴을 사용한 간소화된 V2 TypeScript Agent SDK의 미리보기로, 다중 턴 대화를 지원합니다.

V2 인터페이스는 **불안정한 미리보기**입니다. API는 안정화되기 전에 피드백에 따라 변경될 수 있습니다. 세션 포킹과 같은 일부 기능은 [V1 SDK](https://code.claude.com/docs/ko/agent-sdk/typescript)에서만 사용 가능합니다.
V2 Claude Agent TypeScript SDK는 비동기 생성기와 yield 조정의 필요성을 제거합니다. 이를 통해 다중 턴 대화가 더 간단해지며, 턴 전체에서 생성기 상태를 관리하는 대신 각 턴은 별도의 `send()`/ `stream()` 사이클입니다. API 표면은 세 가지 개념으로 축소됩니다:


- `createSession()` / `resumeSession()`: 대화 시작 또는 계속
- `session.send()`: 메시지 전송
- `session.stream()`: 응답 받기


## [​](https://code.claude.com/docs/ko/agent-sdk/typescript-v2-preview#%EC%84%A4%EC%B9%98) 설치


V2 인터페이스는 기존 SDK 패키지에 포함되어 있습니다:


```
npm install @anthropic-ai/claude-agent-sdk
```


SDK는 선택적 종속성으로 플랫폼용 네이티브 Claude Code 바이너리를 번들로 제공하므로 Claude Code를 별도로 설치할 필요가 없습니다.


## [​](https://code.claude.com/docs/ko/agent-sdk/typescript-v2-preview#%EB%B9%A0%EB%A5%B8-%EC%8B%9C%EC%9E%91) 빠른 시작


### [​](https://code.claude.com/docs/ko/agent-sdk/typescript-v2-preview#%EC%9D%BC%ED%9A%8C%EC%84%B1-%ED%94%84%EB%A1%AC%ED%94%84%ED%8A%B8) 일회성 프롬프트


세션을 유지할 필요가 없는 간단한 단일 턴 쿼리의 경우 `unstable_v2_prompt()`를 사용합니다. 이 예제는 수학 질문을 보내고 답변을 기록합니다:


```
import { unstable_v2_prompt } from "@anthropic-ai/claude-agent-sdk";

const result = await unstable_v2_prompt("What is 2 + 2?", {
  model: "claude-opus-4-7"
});
if (result.subtype === "success") {
  console.log(result.result);
}
```


### [​](https://code.claude.com/docs/ko/agent-sdk/typescript-v2-preview#%EA%B8%B0%EB%B3%B8-%EC%84%B8%EC%85%98) 기본 세션


단일 프롬프트를 넘어서는 상호작용의 경우 세션을 생성합니다. V2는 전송과 스트리밍을 별개의 단계로 분리합니다:


- `send()`는 메시지를 전달합니다
- `stream()`은 응답을 스트리밍합니다


이러한 명시적 분리를 통해 턴 사이에 로직을 추가하기가 더 쉬워집니다(예: 후속 메시지를 보내기 전에 응답 처리).
아래 예제는 세션을 생성하고, Claude에 “Hello!”를 보내고, 텍스트 응답을 출력합니다. 블록이 종료될 때 세션을 자동으로 닫기 위해 [`await using`](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-2.html#using-declarations-and-explicit-resource-management)(TypeScript 5.2+)을 사용합니다. `session.close()`를 수동으로 호출할 수도 있습니다.


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


### [​](https://code.claude.com/docs/ko/agent-sdk/typescript-v2-preview#%EB%8B%A4%EC%A4%91-%ED%84%B4-%EB%8C%80%ED%99%94) 다중 턴 대화


세션은 여러 교환 전체에서 컨텍스트를 유지합니다. 대화를 계속하려면 동일한 세션에서 `send()`를 다시 호출합니다. Claude는 이전 턴을 기억합니다.
이 예제는 수학 질문을 한 다음 이전 답변을 참조하는 후속 질문을 합니다:


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


### [​](https://code.claude.com/docs/ko/agent-sdk/typescript-v2-preview#%EC%84%B8%EC%85%98-%EC%9E%AC%EA%B0%9C) 세션 재개


이전 상호작용에서 세션 ID가 있는 경우 나중에 재개할 수 있습니다. 이는 장기 실행 워크플로우나 애플리케이션 재시작 전체에서 대화를 유지해야 할 때 유용합니다.
이 예제는 세션을 생성하고, ID를 저장하고, 닫은 다음 대화를 재개합니다:


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


### [​](https://code.claude.com/docs/ko/agent-sdk/typescript-v2-preview#%EC%A0%95%EB%A6%AC) 정리


세션은 수동으로 닫거나 [`await using`](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-2.html#using-declarations-and-explicit-resource-management)을 사용하여 자동으로 닫을 수 있습니다. 이는 자동 리소스 정리를 위한 TypeScript 5.2+ 기능입니다. 이전 TypeScript 버전을 사용 중이거나 호환성 문제가 발생하면 대신 수동 정리를 사용합니다.
**자동 정리 (TypeScript 5.2+):**


```
import { unstable_v2_createSession } from "@anthropic-ai/claude-agent-sdk";

await using session = unstable_v2_createSession({
  model: "claude-opus-4-7"
});
// Session closes automatically when the block exits
```


**수동 정리:**


```
import { unstable_v2_createSession } from "@anthropic-ai/claude-agent-sdk";

const session = unstable_v2_createSession({
  model: "claude-opus-4-7"
});
// ... use the session ...
session.close();
```


## [​](https://code.claude.com/docs/ko/agent-sdk/typescript-v2-preview#api-%EC%B0%B8%EC%A1%B0) API 참조


### [​](https://code.claude.com/docs/ko/agent-sdk/typescript-v2-preview#unstable_v2_createsession) `unstable_v2_createSession()`


다중 턴 대화를 위한 새 세션을 생성합니다.


```
function unstable_v2_createSession(options: {
  model: string;
  // Additional options supported
}): SDKSession;
```


### [​](https://code.claude.com/docs/ko/agent-sdk/typescript-v2-preview#unstable_v2_resumesession) `unstable_v2_resumeSession()`


ID로 기존 세션을 재개합니다.


```
function unstable_v2_resumeSession(
  sessionId: string,
  options: {
    model: string;
    // Additional options supported
  }
): SDKSession;
```


### [​](https://code.claude.com/docs/ko/agent-sdk/typescript-v2-preview#unstable_v2_prompt) `unstable_v2_prompt()`


단일 턴 쿼리를 위한 일회성 편의 함수입니다.


```
function unstable_v2_prompt(
  prompt: string,
  options: {
    model: string;
    // Additional options supported
  }
): Promise<SDKResultMessage>;
```


### [​](https://code.claude.com/docs/ko/agent-sdk/typescript-v2-preview#sdksession-%EC%9D%B8%ED%84%B0%ED%8E%98%EC%9D%B4%EC%8A%A4) SDKSession 인터페이스


```
interface SDKSession {
  readonly sessionId: string;
  send(message: string | SDKUserMessage): Promise<void>;
  stream(): AsyncGenerator<SDKMessage, void>;
  close(): void;
}
```


## [​](https://code.claude.com/docs/ko/agent-sdk/typescript-v2-preview#%EA%B8%B0%EB%8A%A5-%EA%B0%80%EC%9A%A9%EC%84%B1) 기능 가용성


모든 V1 기능이 V2에서 아직 사용 가능한 것은 아닙니다. 다음은 [V1 SDK](https://code.claude.com/docs/ko/agent-sdk/typescript)를 사용해야 합니다:


- 세션 포킹 (`forkSession` 옵션)
- 일부 고급 스트리밍 입력 패턴


## [​](https://code.claude.com/docs/ko/agent-sdk/typescript-v2-preview#%ED%94%BC%EB%93%9C%EB%B0%B1) 피드백


V2 인터페이스가 안정화되기 전에 피드백을 공유합니다. [GitHub Issues](https://github.com/anthropics/claude-code/issues)를 통해 문제와 제안을 보고합니다.


## [​](https://code.claude.com/docs/ko/agent-sdk/typescript-v2-preview#%EC%B0%B8%EA%B3%A0-%ED%95%AD%EB%AA%A9) 참고 항목


- [TypeScript SDK 참조 (V1)](https://code.claude.com/docs/ko/agent-sdk/typescript) - 전체 V1 SDK 문서
- [SDK 개요](https://code.claude.com/docs/ko/agent-sdk/overview) - 일반 SDK 개념
- [GitHub의 V2 예제](https://github.com/anthropics/claude-agent-sdk-demos/tree/main/hello-world-v2) - 작동하는 코드 예제[Claude Code Docs home page](https://code.claude.com/docs/ko/overview)

[Privacy choices](https://code.claude.com/docs/ko/agent-sdk/typescript-v2-preview#)

