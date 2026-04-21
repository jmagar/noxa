# TypeScript SDK V2 Schnittstelle (Vorschau
## ​Installation
## ​Schnellstart
## ​API-Referenz
## ​Funktionsverfügbarkeit
## ​Feedback
## ​Siehe auch







Vorschau der vereinfachten V2 TypeScript Agent SDK mit sitzungsbasiertem Send/Stream-Muster für mehrteilige Gespräche.

Die V2-Schnittstelle ist eine **instabile Vorschau**. APIs können sich basierend auf Feedback ändern, bevor sie stabil werden. Einige Funktionen wie Session-Forking sind nur im [V1 SDK](https://code.claude.com/docs/de/agent-sdk/typescript) verfügbar.
Das V2 Claude Agent TypeScript SDK entfernt die Notwendigkeit für asynchrone Generatoren und Yield-Koordination. Dies macht mehrteilige Gespräche einfacher, anstatt den Generator-Status über Turns hinweg zu verwalten, ist jeder Turn ein separater `send()`/ `stream()`-Zyklus. Die API-Oberfläche reduziert sich auf drei Konzepte:


- `createSession()` / `resumeSession()`: Starten oder fortsetzen eines Gesprächs
- `session.send()`: Eine Nachricht senden
- `session.stream()`: Die Antwort abrufen


## [​](https://code.claude.com/docs/de/agent-sdk/typescript-v2-preview#installation) Installation


Die V2-Schnittstelle ist im bestehenden SDK-Paket enthalten:


```
npm install @anthropic-ai/claude-agent-sdk
```


Das SDK bündelt eine native Claude Code-Binärdatei für Ihre Plattform als optionale Abhängigkeit, sodass Sie Claude Code nicht separat installieren müssen.


## [​](https://code.claude.com/docs/de/agent-sdk/typescript-v2-preview#schnellstart) Schnellstart


### [​](https://code.claude.com/docs/de/agent-sdk/typescript-v2-preview#einmalige-anfrage) Einmalige Anfrage


Für einfache Einzelturn-Abfragen, bei denen Sie keine Sitzung beibehalten müssen, verwenden Sie `unstable_v2_prompt()`. Dieses Beispiel sendet eine Mathefrage und protokolliert die Antwort:


```
import { unstable_v2_prompt } from "@anthropic-ai/claude-agent-sdk";

const result = await unstable_v2_prompt("What is 2 + 2?", {
  model: "claude-opus-4-7"
});
if (result.subtype === "success") {
  console.log(result.result);
}
```


### [​](https://code.claude.com/docs/de/agent-sdk/typescript-v2-preview#grundlegende-sitzung) Grundlegende Sitzung


Für Interaktionen über eine einzelne Anfrage hinaus erstellen Sie eine Sitzung. V2 trennt das Senden und Streamen in unterschiedliche Schritte:


- `send()` sendet Ihre Nachricht
- `stream()` streamt die Antwort zurück


Diese explizite Trennung macht es einfacher, Logik zwischen Turns hinzuzufügen (wie das Verarbeiten von Antworten vor dem Senden von Folgefragen).
Das folgende Beispiel erstellt eine Sitzung, sendet „Hello!” an Claude und gibt die Textantwort aus. Es verwendet [`await using`](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-2.html#using-declarations-and-explicit-resource-management) (TypeScript 5.2+), um die Sitzung automatisch zu schließen, wenn der Block beendet wird. Sie können auch `session.close()` manuell aufrufen.


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


### [​](https://code.claude.com/docs/de/agent-sdk/typescript-v2-preview#mehrteiliges-gespr%C3%A4ch) Mehrteiliges Gespräch


Sitzungen behalten den Kontext über mehrere Austausche hinweg bei. Um ein Gespräch fortzusetzen, rufen Sie `send()` erneut in derselben Sitzung auf. Claude merkt sich die vorherigen Turns.
Dieses Beispiel stellt eine Mathefrage und stellt dann eine Folgefrage, die sich auf die vorherige Antwort bezieht:


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


### [​](https://code.claude.com/docs/de/agent-sdk/typescript-v2-preview#sitzung-fortsetzen) Sitzung fortsetzen


Wenn Sie eine Sitzungs-ID aus einer vorherigen Interaktion haben, können Sie diese später fortsetzen. Dies ist nützlich für langfristige Workflows oder wenn Sie Gespräche über Anwendungsneustarts hinweg beibehalten müssen.
Dieses Beispiel erstellt eine Sitzung, speichert ihre ID, schließt sie und setzt das Gespräch dann fort:


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


### [​](https://code.claude.com/docs/de/agent-sdk/typescript-v2-preview#bereinigung) Bereinigung


Sitzungen können manuell oder automatisch mit [`await using`](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-2.html#using-declarations-and-explicit-resource-management) geschlossen werden, einer TypeScript 5.2+-Funktion für automatische Ressourcenbereinigung. Wenn Sie eine ältere TypeScript-Version verwenden oder auf Kompatibilitätsprobleme stoßen, verwenden Sie stattdessen manuelle Bereinigung.
**Automatische Bereinigung (TypeScript 5.2+):**


```
import { unstable_v2_createSession } from "@anthropic-ai/claude-agent-sdk";

await using session = unstable_v2_createSession({
  model: "claude-opus-4-7"
});
// Session closes automatically when the block exits
```


**Manuelle Bereinigung:**


```
import { unstable_v2_createSession } from "@anthropic-ai/claude-agent-sdk";

const session = unstable_v2_createSession({
  model: "claude-opus-4-7"
});
// ... use the session ...
session.close();
```


## [​](https://code.claude.com/docs/de/agent-sdk/typescript-v2-preview#api-referenz) API-Referenz


### [​](https://code.claude.com/docs/de/agent-sdk/typescript-v2-preview#unstable_v2_createsession) `unstable_v2_createSession()`


Erstellt eine neue Sitzung für mehrteilige Gespräche.


```
function unstable_v2_createSession(options: {
  model: string;
  // Additional options supported
}): SDKSession;
```


### [​](https://code.claude.com/docs/de/agent-sdk/typescript-v2-preview#unstable_v2_resumesession) `unstable_v2_resumeSession()`


Setzt eine vorhandene Sitzung nach ID fort.


```
function unstable_v2_resumeSession(
  sessionId: string,
  options: {
    model: string;
    // Additional options supported
  }
): SDKSession;
```


### [​](https://code.claude.com/docs/de/agent-sdk/typescript-v2-preview#unstable_v2_prompt) `unstable_v2_prompt()`


Einmalige Komfortfunktion für Einzelturn-Abfragen.


```
function unstable_v2_prompt(
  prompt: string,
  options: {
    model: string;
    // Additional options supported
  }
): Promise<SDKResultMessage>;
```


### [​](https://code.claude.com/docs/de/agent-sdk/typescript-v2-preview#sdksession-schnittstelle) SDKSession-Schnittstelle


```
interface SDKSession {
  readonly sessionId: string;
  send(message: string | SDKUserMessage): Promise<void>;
  stream(): AsyncGenerator<SDKMessage, void>;
  close(): void;
}
```


## [​](https://code.claude.com/docs/de/agent-sdk/typescript-v2-preview#funktionsverf%C3%BCgbarkeit) Funktionsverfügbarkeit


Nicht alle V1-Funktionen sind in V2 noch verfügbar. Die folgenden erfordern die Verwendung des [V1 SDK](https://code.claude.com/docs/de/agent-sdk/typescript):


- Session-Forking (`forkSession`-Option)
- Einige erweiterte Streaming-Eingabemuster


## [​](https://code.claude.com/docs/de/agent-sdk/typescript-v2-preview#feedback) Feedback


Teilen Sie Ihr Feedback zur V2-Schnittstelle mit, bevor sie stabil wird. Melden Sie Probleme und Vorschläge über [GitHub Issues](https://github.com/anthropics/claude-code/issues).


## [​](https://code.claude.com/docs/de/agent-sdk/typescript-v2-preview#siehe-auch) Siehe auch


- [TypeScript SDK-Referenz (V1)](https://code.claude.com/docs/de/agent-sdk/typescript) - Vollständige V1 SDK-Dokumentation
- [SDK-Übersicht](https://code.claude.com/docs/de/agent-sdk/overview) - Allgemeine SDK-Konzepte
- [V2-Beispiele auf GitHub](https://github.com/anthropics/claude-agent-sdk-demos/tree/main/hello-world-v2) - Funktionierende Code-Beispiele[Claude Code Docs home page](https://code.claude.com/docs/de/overview)

[Privacy choices](https://code.claude.com/docs/de/agent-sdk/typescript-v2-preview#)

