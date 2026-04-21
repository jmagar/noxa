# TypeScript SDK V2 インターフェース（プレビュー
## ​インストール
## ​クイックスタート
## ​API リファレンス
## ​機能の可用性
## ​フィードバック
## ​関連項目







マルチターン会話向けのセッションベースの send/stream パターンを備えた、簡略化された V2 TypeScript Agent SDK のプレビュー。

V2 インターフェースは **不安定なプレビュー**です。安定化する前にフィードバックに基づいて API が変更される可能性があります。セッションフォーキングなどの一部の機能は、 [V1 SDK](https://code.claude.com/docs/ja/agent-sdk/typescript) でのみ利用可能です。
V2 Claude Agent TypeScript SDK は、非同期ジェネレータと yield 調整の必要性を排除します。これにより、マルチターン会話がより簡単になります。ターン間でジェネレータの状態を管理する代わりに、各ターンは個別の `send()`/ `stream()` サイクルになります。API サーフェスは 3 つの概念に縮小されます。


- `createSession()` / `resumeSession()`：会話を開始または継続する
- `session.send()`：メッセージを送信する
- `session.stream()`：レスポンスを取得する


## [​](https://code.claude.com/docs/ja/agent-sdk/typescript-v2-preview#%E3%82%A4%E3%83%B3%E3%82%B9%E3%83%88%E3%83%BC%E3%83%AB) インストール


V2 インターフェースは既存の SDK パッケージに含まれています。


```
npm install @anthropic-ai/claude-agent-sdk
```


SDK はオプションの依存関係として、プラットフォーム用のネイティブ Claude Code バイナリをバンドルしているため、Claude Code を別途インストールする必要はありません。


## [​](https://code.claude.com/docs/ja/agent-sdk/typescript-v2-preview#%E3%82%AF%E3%82%A4%E3%83%83%E3%82%AF%E3%82%B9%E3%82%BF%E3%83%BC%E3%83%88) クイックスタート


### [​](https://code.claude.com/docs/ja/agent-sdk/typescript-v2-preview#%E3%83%AF%E3%83%B3%E3%82%B7%E3%83%A7%E3%83%83%E3%83%88%E3%83%97%E3%83%AD%E3%83%B3%E3%83%97%E3%83%88) ワンショットプロンプト


セッションを維持する必要がない単純なシングルターンクエリの場合は、 `unstable_v2_prompt()` を使用します。この例は数学の質問を送信し、答えをログに出力します。


```
import { unstable_v2_prompt } from "@anthropic-ai/claude-agent-sdk";

const result = await unstable_v2_prompt("What is 2 + 2?", {
  model: "claude-opus-4-7"
});
if (result.subtype === "success") {
  console.log(result.result);
}
```


### [​](https://code.claude.com/docs/ja/agent-sdk/typescript-v2-preview#%E5%9F%BA%E6%9C%AC%E7%9A%84%E3%81%AA%E3%82%BB%E3%83%83%E3%82%B7%E3%83%A7%E3%83%B3) 基本的なセッション


単一のプロンプトを超えるインタラクションの場合は、セッションを作成します。V2 は送信とストリーミングを個別のステップに分離します。


- `send()` はメッセージをディスパッチします
- `stream()` はレスポンスをストリーミングします


この明示的な分離により、ターン間にロジックを追加しやすくなります（レスポンスを処理してからフォローアップを送信するなど）。
以下の例はセッションを作成し、「Hello!」を Claude に送信し、テキストレスポンスを出力します。 [`await using`](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-2.html#using-declarations-and-explicit-resource-management)（TypeScript 5.2 以降）を使用して、ブロックが終了するときにセッションを自動的に閉じます。 `session.close()` を手動で呼び出すこともできます。


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


### [​](https://code.claude.com/docs/ja/agent-sdk/typescript-v2-preview#%E3%83%9E%E3%83%AB%E3%83%81%E3%82%BF%E3%83%BC%E3%83%B3%E4%BC%9A%E8%A9%B1) マルチターン会話


セッションは複数の交換全体でコンテキストを保持します。会話を続けるには、同じセッションで `send()` を再度呼び出します。Claude は前のターンを記憶しています。
この例は数学の質問を尋ねてから、前の答えを参照するフォローアップを尋ねます。


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


### [​](https://code.claude.com/docs/ja/agent-sdk/typescript-v2-preview#%E3%82%BB%E3%83%83%E3%82%B7%E3%83%A7%E3%83%B3%E3%81%AE%E5%86%8D%E9%96%8B) セッションの再開


前のインタラクションからセッション ID がある場合は、後でそれを再開できます。これは長時間実行されるワークフローや、アプリケーションの再起動全体で会話を永続化する必要がある場合に便利です。
この例はセッションを作成し、その ID を保存し、それを閉じてから会話を再開します。


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


### [​](https://code.claude.com/docs/ja/agent-sdk/typescript-v2-preview#%E3%82%AF%E3%83%AA%E3%83%BC%E3%83%B3%E3%82%A2%E3%83%83%E3%83%97) クリーンアップ


セッションは手動で閉じるか、 [`await using`](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-2.html#using-declarations-and-explicit-resource-management)（自動リソースクリーンアップ用の TypeScript 5.2 以降の機能）を使用して自動的に閉じることができます。古い TypeScript バージョンを使用している場合や互換性の問題が発生した場合は、代わりに手動クリーンアップを使用してください。
**自動クリーンアップ（TypeScript 5.2 以降）：**


```
import { unstable_v2_createSession } from "@anthropic-ai/claude-agent-sdk";

await using session = unstable_v2_createSession({
  model: "claude-opus-4-7"
});
// Session closes automatically when the block exits
```


**手動クリーンアップ：**


```
import { unstable_v2_createSession } from "@anthropic-ai/claude-agent-sdk";

const session = unstable_v2_createSession({
  model: "claude-opus-4-7"
});
// ... use the session ...
session.close();
```


## [​](https://code.claude.com/docs/ja/agent-sdk/typescript-v2-preview#api-%E3%83%AA%E3%83%95%E3%82%A1%E3%83%AC%E3%83%B3%E3%82%B9) API リファレンス


### [​](https://code.claude.com/docs/ja/agent-sdk/typescript-v2-preview#unstable_v2_createsession) `unstable_v2_createSession()`


マルチターン会話用の新しいセッションを作成します。


```
function unstable_v2_createSession(options: {
  model: string;
  // Additional options supported
}): SDKSession;
```


### [​](https://code.claude.com/docs/ja/agent-sdk/typescript-v2-preview#unstable_v2_resumesession) `unstable_v2_resumeSession()`


ID で既存のセッションを再開します。


```
function unstable_v2_resumeSession(
  sessionId: string,
  options: {
    model: string;
    // Additional options supported
  }
): SDKSession;
```


### [​](https://code.claude.com/docs/ja/agent-sdk/typescript-v2-preview#unstable_v2_prompt) `unstable_v2_prompt()`


シングルターンクエリ用のワンショット便利関数。


```
function unstable_v2_prompt(
  prompt: string,
  options: {
    model: string;
    // Additional options supported
  }
): Promise<SDKResultMessage>;
```


### [​](https://code.claude.com/docs/ja/agent-sdk/typescript-v2-preview#sdksession-%E3%82%A4%E3%83%B3%E3%82%BF%E3%83%BC%E3%83%95%E3%82%A7%E3%83%BC%E3%82%B9) SDKSession インターフェース


```
interface SDKSession {
  readonly sessionId: string;
  send(message: string | SDKUserMessage): Promise<void>;
  stream(): AsyncGenerator<SDKMessage, void>;
  close(): void;
}
```


## [​](https://code.claude.com/docs/ja/agent-sdk/typescript-v2-preview#%E6%A9%9F%E8%83%BD%E3%81%AE%E5%8F%AF%E7%94%A8%E6%80%A7) 機能の可用性


すべての V1 機能が V2 でまだ利用可能ではありません。以下は [V1 SDK](https://code.claude.com/docs/ja/agent-sdk/typescript) を使用する必要があります。


- セッションフォーキング（`forkSession` オプション）
- 一部の高度なストリーミング入力パターン


## [​](https://code.claude.com/docs/ja/agent-sdk/typescript-v2-preview#%E3%83%95%E3%82%A3%E3%83%BC%E3%83%89%E3%83%90%E3%83%83%E3%82%AF) フィードバック


V2 インターフェースが安定化する前に、フィードバックを共有してください。 [GitHub Issues](https://github.com/anthropics/claude-code/issues) を通じて問題と提案を報告してください。


## [​](https://code.claude.com/docs/ja/agent-sdk/typescript-v2-preview#%E9%96%A2%E9%80%A3%E9%A0%85%E7%9B%AE) 関連項目


- [TypeScript SDK リファレンス（V1）](https://code.claude.com/docs/ja/agent-sdk/typescript) - 完全な V1 SDK ドキュメント
- [SDK 概要](https://code.claude.com/docs/ja/agent-sdk/overview) - 一般的な SDK の概念
- [GitHub 上の V2 例](https://github.com/anthropics/claude-agent-sdk-demos/tree/main/hello-world-v2) - 動作するコード例[Claude Code Docs home page](https://code.claude.com/docs/ja/overview)

[Privacy choices](https://code.claude.com/docs/ja/agent-sdk/typescript-v2-preview#)

