# Interface TypeScript SDK V2 (visualização
## ​Instalação
## ​Início rápido
## ​Referência da API
## ​Disponibilidade de recursos
## ​Feedback
## ​Veja também







Visualização do SDK do Agent TypeScript V2 simplificado, com padrões de envio/stream baseados em sessão para conversas multi-turno.

A interface V2 é uma **visualização instável**. As APIs podem mudar com base em feedback antes de se tornarem estáveis. Alguns recursos como bifurcação de sessão estão disponíveis apenas no [SDK V1](https://code.claude.com/docs/pt/agent-sdk/typescript).
O SDK do Agent TypeScript Claude V2 remove a necessidade de geradores assíncronos e coordenação de yield. Isso torna as conversas multi-turno mais simples, em vez de gerenciar o estado do gerador entre turnos, cada turno é um ciclo `send()`/ `stream()` separado. A superfície da API se reduz a três conceitos:


- `createSession()` / `resumeSession()`: Iniciar ou continuar uma conversa
- `session.send()`: Enviar uma mensagem
- `session.stream()`: Obter a resposta


## [​](https://code.claude.com/docs/pt/agent-sdk/typescript-v2-preview#instala%C3%A7%C3%A3o) Instalação


A interface V2 está incluída no pacote SDK existente:


```
npm install @anthropic-ai/claude-agent-sdk
```


O SDK agrupa um binário nativo do Claude Code para sua plataforma como uma dependência opcional, portanto você não precisa instalar o Claude Code separadamente.


## [​](https://code.claude.com/docs/pt/agent-sdk/typescript-v2-preview#in%C3%ADcio-r%C3%A1pido) Início rápido


### [​](https://code.claude.com/docs/pt/agent-sdk/typescript-v2-preview#prompt-%C3%BAnico) Prompt único


Para consultas simples de turno único onde você não precisa manter uma sessão, use `unstable_v2_prompt()`. Este exemplo envia uma pergunta de matemática e registra a resposta:


```
import { unstable_v2_prompt } from "@anthropic-ai/claude-agent-sdk";

const result = await unstable_v2_prompt("What is 2 + 2?", {
  model: "claude-opus-4-7"
});
if (result.subtype === "success") {
  console.log(result.result);
}
```


### [​](https://code.claude.com/docs/pt/agent-sdk/typescript-v2-preview#sess%C3%A3o-b%C3%A1sica) Sessão básica


Para interações além de um único prompt, crie uma sessão. V2 separa envio e streaming em etapas distintas:


- `send()` envia sua mensagem
- `stream()` transmite a resposta


Esta separação explícita torna mais fácil adicionar lógica entre turnos (como processar respostas antes de enviar acompanhamentos).
O exemplo abaixo cria uma sessão, envia “Hello!” para Claude e imprime a resposta de texto. Ele usa [`await using`](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-2.html#using-declarations-and-explicit-resource-management) (TypeScript 5.2+) para fechar automaticamente a sessão quando o bloco sai. Você também pode chamar `session.close()` manualmente.


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


### [​](https://code.claude.com/docs/pt/agent-sdk/typescript-v2-preview#conversa-multi-turno) Conversa multi-turno


As sessões persistem contexto em múltiplas trocas. Para continuar uma conversa, chame `send()` novamente na mesma sessão. Claude se lembra dos turnos anteriores.
Este exemplo faz uma pergunta de matemática e depois faz um acompanhamento que referencia a resposta anterior:


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


### [​](https://code.claude.com/docs/pt/agent-sdk/typescript-v2-preview#retomada-de-sess%C3%A3o) Retomada de sessão


Se você tiver um ID de sessão de uma interação anterior, poderá retomá-lo mais tarde. Isso é útil para fluxos de trabalho de longa duração ou quando você precisa persistir conversas entre reinicializações de aplicativo.
Este exemplo cria uma sessão, armazena seu ID, a fecha e depois retoma a conversa:


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


### [​](https://code.claude.com/docs/pt/agent-sdk/typescript-v2-preview#limpeza) Limpeza


As sessões podem ser fechadas manualmente ou automaticamente usando [`await using`](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-2.html#using-declarations-and-explicit-resource-management), um recurso do TypeScript 5.2+ para limpeza automática de recursos. Se você estiver usando uma versão mais antiga do TypeScript ou encontrar problemas de compatibilidade, use limpeza manual.
**Limpeza automática (TypeScript 5.2+):**


```
import { unstable_v2_createSession } from "@anthropic-ai/claude-agent-sdk";

await using session = unstable_v2_createSession({
  model: "claude-opus-4-7"
});
// Session closes automatically when the block exits
```


**Limpeza manual:**


```
import { unstable_v2_createSession } from "@anthropic-ai/claude-agent-sdk";

const session = unstable_v2_createSession({
  model: "claude-opus-4-7"
});
// ... use the session ...
session.close();
```


## [​](https://code.claude.com/docs/pt/agent-sdk/typescript-v2-preview#refer%C3%AAncia-da-api) Referência da API


### [​](https://code.claude.com/docs/pt/agent-sdk/typescript-v2-preview#unstable_v2_createsession) `unstable_v2_createSession()`


Cria uma nova sessão para conversas multi-turno.


```
function unstable_v2_createSession(options: {
  model: string;
  // Additional options supported
}): SDKSession;
```


### [​](https://code.claude.com/docs/pt/agent-sdk/typescript-v2-preview#unstable_v2_resumesession) `unstable_v2_resumeSession()`


Retoma uma sessão existente por ID.


```
function unstable_v2_resumeSession(
  sessionId: string,
  options: {
    model: string;
    // Additional options supported
  }
): SDKSession;
```


### [​](https://code.claude.com/docs/pt/agent-sdk/typescript-v2-preview#unstable_v2_prompt) `unstable_v2_prompt()`


Função de conveniência única para consultas de turno único.


```
function unstable_v2_prompt(
  prompt: string,
  options: {
    model: string;
    // Additional options supported
  }
): Promise<SDKResultMessage>;
```


### [​](https://code.claude.com/docs/pt/agent-sdk/typescript-v2-preview#interface-sdksession) Interface SDKSession


```
interface SDKSession {
  readonly sessionId: string;
  send(message: string | SDKUserMessage): Promise<void>;
  stream(): AsyncGenerator<SDKMessage, void>;
  close(): void;
}
```


## [​](https://code.claude.com/docs/pt/agent-sdk/typescript-v2-preview#disponibilidade-de-recursos) Disponibilidade de recursos


Nem todos os recursos V1 estão disponíveis em V2 ainda. Os seguintes requerem o uso do [SDK V1](https://code.claude.com/docs/pt/agent-sdk/typescript):


- Bifurcação de sessão (opção `forkSession`)
- Alguns padrões avançados de entrada de streaming


## [​](https://code.claude.com/docs/pt/agent-sdk/typescript-v2-preview#feedback) Feedback


Compartilhe seu feedback sobre a interface V2 antes que ela se torne estável. Relate problemas e sugestões através de [GitHub Issues](https://github.com/anthropics/claude-code/issues).


## [​](https://code.claude.com/docs/pt/agent-sdk/typescript-v2-preview#veja-tamb%C3%A9m) Veja também


- [Referência do SDK TypeScript (V1)](https://code.claude.com/docs/pt/agent-sdk/typescript) - Documentação completa do SDK V1
- [Visão geral do SDK](https://code.claude.com/docs/pt/agent-sdk/overview) - Conceitos gerais do SDK
- [Exemplos V2 no GitHub](https://github.com/anthropics/claude-agent-sdk-demos/tree/main/hello-world-v2) - Exemplos de código funcionando[Claude Code Docs home page](https://code.claude.com/docs/pt/overview)

[Privacy choices](https://code.claude.com/docs/pt/agent-sdk/typescript-v2-preview#)

