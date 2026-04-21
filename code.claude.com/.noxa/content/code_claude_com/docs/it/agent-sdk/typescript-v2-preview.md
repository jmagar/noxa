# Interfaccia TypeScript SDK V2 (anteprima
## ​Installazione
## ​Avvio rapido
## ​Riferimento API
## ​Disponibilità delle funzionalità
## ​Feedback
## ​Vedere anche







Anteprima dell’SDK Agent TypeScript V2 semplificato, con pattern send/stream basati su sessione per conversazioni multi-turno.

L’interfaccia V2 è un’ **anteprima instabile**. Le API potrebbero cambiare in base al feedback prima di diventare stabili. Alcune funzionalità come il forking della sessione sono disponibili solo nell’ [SDK V1](https://code.claude.com/docs/it/agent-sdk/typescript).
L’SDK Agent TypeScript V2 di Claude rimuove la necessità di generatori asincroni e coordinamento yield. Questo rende le conversazioni multi-turno più semplici; invece di gestire lo stato del generatore tra i turni, ogni turno è un ciclo `send()`/ `stream()` separato. La superficie API si riduce a tre concetti:


- `createSession()` / `resumeSession()`: Avviare o continuare una conversazione
- `session.send()`: Inviare un messaggio
- `session.stream()`: Ottenere la risposta


## [​](https://code.claude.com/docs/it/agent-sdk/typescript-v2-preview#installazione) Installazione


L’interfaccia V2 è inclusa nel pacchetto SDK esistente:


```
npm install @anthropic-ai/claude-agent-sdk
```


L’SDK raggruppa un binario Claude Code nativo per la vostra piattaforma come dipendenza opzionale, quindi non è necessario installare Claude Code separatamente.


## [​](https://code.claude.com/docs/it/agent-sdk/typescript-v2-preview#avvio-rapido) Avvio rapido


### [​](https://code.claude.com/docs/it/agent-sdk/typescript-v2-preview#prompt-singolo) Prompt singolo


Per semplici query a turno singolo dove non è necessario mantenere una sessione, utilizzare `unstable_v2_prompt()`. Questo esempio invia una domanda matematica e registra la risposta:


```
import { unstable_v2_prompt } from "@anthropic-ai/claude-agent-sdk";

const result = await unstable_v2_prompt("What is 2 + 2?", {
  model: "claude-opus-4-7"
});
if (result.subtype === "success") {
  console.log(result.result);
}
```


### [​](https://code.claude.com/docs/it/agent-sdk/typescript-v2-preview#sessione-di-base) Sessione di base


Per interazioni oltre un singolo prompt, creare una sessione. V2 separa l’invio e lo streaming in passaggi distinti:


- `send()` invia il vostro messaggio
- `stream()` trasmette la risposta


Questa separazione esplicita rende più facile aggiungere logica tra i turni (come elaborare le risposte prima di inviare i follow-up).
L’esempio seguente crea una sessione, invia “Hello!” a Claude e stampa la risposta di testo. Utilizza [`await using`](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-2.html#using-declarations-and-explicit-resource-management) (TypeScript 5.2+) per chiudere automaticamente la sessione quando il blocco esce. Potete anche chiamare `session.close()` manualmente.


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


### [​](https://code.claude.com/docs/it/agent-sdk/typescript-v2-preview#conversazione-multi-turno) Conversazione multi-turno


Le sessioni mantengono il contesto attraverso più scambi. Per continuare una conversazione, chiamare `send()` di nuovo sulla stessa sessione. Claude ricorda i turni precedenti.
Questo esempio pone una domanda matematica, quindi pone un follow-up che fa riferimento alla risposta precedente:


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


### [​](https://code.claude.com/docs/it/agent-sdk/typescript-v2-preview#ripresa-della-sessione) Ripresa della sessione


Se avete un ID di sessione da un’interazione precedente, potete riprenderla in seguito. Questo è utile per flussi di lavoro di lunga durata o quando è necessario persistere le conversazioni tra i riavvii dell’applicazione.
Questo esempio crea una sessione, memorizza il suo ID, la chiude, quindi riprende la conversazione:


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


### [​](https://code.claude.com/docs/it/agent-sdk/typescript-v2-preview#pulizia) Pulizia


Le sessioni possono essere chiuse manualmente o automaticamente utilizzando [`await using`](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-2.html#using-declarations-and-explicit-resource-management), una funzionalità di TypeScript 5.2+ per la pulizia automatica delle risorse. Se state utilizzando una versione precedente di TypeScript o riscontrate problemi di compatibilità, utilizzate invece la pulizia manuale.
**Pulizia automatica (TypeScript 5.2+):**


```
import { unstable_v2_createSession } from "@anthropic-ai/claude-agent-sdk";

await using session = unstable_v2_createSession({
  model: "claude-opus-4-7"
});
// Session closes automatically when the block exits
```


**Pulizia manuale:**


```
import { unstable_v2_createSession } from "@anthropic-ai/claude-agent-sdk";

const session = unstable_v2_createSession({
  model: "claude-opus-4-7"
});
// ... use the session ...
session.close();
```


## [​](https://code.claude.com/docs/it/agent-sdk/typescript-v2-preview#riferimento-api) Riferimento API


### [​](https://code.claude.com/docs/it/agent-sdk/typescript-v2-preview#unstable_v2_createsession) `unstable_v2_createSession()`


Crea una nuova sessione per conversazioni multi-turno.


```
function unstable_v2_createSession(options: {
  model: string;
  // Additional options supported
}): SDKSession;
```


### [​](https://code.claude.com/docs/it/agent-sdk/typescript-v2-preview#unstable_v2_resumesession) `unstable_v2_resumeSession()`


Riprende una sessione esistente per ID.


```
function unstable_v2_resumeSession(
  sessionId: string,
  options: {
    model: string;
    // Additional options supported
  }
): SDKSession;
```


### [​](https://code.claude.com/docs/it/agent-sdk/typescript-v2-preview#unstable_v2_prompt) `unstable_v2_prompt()`


Funzione di convenienza one-shot per query a turno singolo.


```
function unstable_v2_prompt(
  prompt: string,
  options: {
    model: string;
    // Additional options supported
  }
): Promise<SDKResultMessage>;
```


### [​](https://code.claude.com/docs/it/agent-sdk/typescript-v2-preview#interfaccia-sdksession) Interfaccia SDKSession


```
interface SDKSession {
  readonly sessionId: string;
  send(message: string | SDKUserMessage): Promise<void>;
  stream(): AsyncGenerator<SDKMessage, void>;
  close(): void;
}
```


## [​](https://code.claude.com/docs/it/agent-sdk/typescript-v2-preview#disponibilit%C3%A0-delle-funzionalit%C3%A0) Disponibilità delle funzionalità


Non tutte le funzionalità V1 sono ancora disponibili in V2. Le seguenti richiedono l’utilizzo dell’ [SDK V1](https://code.claude.com/docs/it/agent-sdk/typescript):


- Forking della sessione (opzione `forkSession`)
- Alcuni pattern di input streaming avanzati


## [​](https://code.claude.com/docs/it/agent-sdk/typescript-v2-preview#feedback) Feedback


Condividete il vostro feedback sull’interfaccia V2 prima che diventi stabile. Segnalate problemi e suggerimenti tramite [GitHub Issues](https://github.com/anthropics/claude-code/issues).


## [​](https://code.claude.com/docs/it/agent-sdk/typescript-v2-preview#vedere-anche) Vedere anche


- [Riferimento SDK TypeScript (V1)](https://code.claude.com/docs/it/agent-sdk/typescript) - Documentazione completa dell’SDK V1
- [Panoramica SDK](https://code.claude.com/docs/it/agent-sdk/overview) - Concetti generali dell’SDK
- [Esempi V2 su GitHub](https://github.com/anthropics/claude-agent-sdk-demos/tree/main/hello-world-v2) - Esempi di codice funzionanti[Claude Code Docs home page](https://code.claude.com/docs/it/overview)

[Privacy choices](https://code.claude.com/docs/it/agent-sdk/typescript-v2-preview#)

