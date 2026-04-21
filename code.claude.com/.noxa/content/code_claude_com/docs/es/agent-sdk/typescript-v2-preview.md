# Interfaz TypeScript SDK V2 (vista previa
## ​Instalación
## ​Inicio rápido
## ​Referencia de API
## ​Disponibilidad de características
## ​Comentarios
## ​Véase también







Vista previa del SDK del Agente TypeScript V2 simplificado, con patrones de envío/transmisión basados en sesiones para conversaciones de múltiples turnos.

La interfaz V2 es una **vista previa inestable**. Las API pueden cambiar según los comentarios antes de volverse estables. Algunas características como la bifurcación de sesiones solo están disponibles en el [SDK V1](https://code.claude.com/docs/es/agent-sdk/typescript).
El SDK del Agente TypeScript V2 de Claude elimina la necesidad de generadores asincronos y coordinación de rendimiento. Esto hace que las conversaciones de múltiples turnos sean más simples; en lugar de gestionar el estado del generador entre turnos, cada turno es un ciclo `send()`/ `stream()` separado. La superficie de la API se reduce a tres conceptos:


- `createSession()` / `resumeSession()`: Iniciar o continuar una conversación
- `session.send()`: Enviar un mensaje
- `session.stream()`: Obtener la respuesta


## [​](https://code.claude.com/docs/es/agent-sdk/typescript-v2-preview#instalaci%C3%B3n) Instalación


La interfaz V2 se incluye en el paquete SDK existente:


```
npm install @anthropic-ai/claude-agent-sdk
```


El SDK incluye un binario nativo de Claude Code para su plataforma como una dependencia opcional, por lo que no necesita instalar Claude Code por separado.


## [​](https://code.claude.com/docs/es/agent-sdk/typescript-v2-preview#inicio-r%C3%A1pido) Inicio rápido


### [​](https://code.claude.com/docs/es/agent-sdk/typescript-v2-preview#solicitud-de-un-solo-turno) Solicitud de un solo turno


Para consultas simples de un solo turno donde no necesita mantener una sesión, use `unstable_v2_prompt()`. Este ejemplo envía una pregunta matemática y registra la respuesta:


```
import { unstable_v2_prompt } from "@anthropic-ai/claude-agent-sdk";

const result = await unstable_v2_prompt("What is 2 + 2?", {
  model: "claude-opus-4-7"
});
if (result.subtype === "success") {
  console.log(result.result);
}
```


### [​](https://code.claude.com/docs/es/agent-sdk/typescript-v2-preview#sesi%C3%B3n-b%C3%A1sica) Sesión básica


Para interacciones más allá de una solicitud única, cree una sesión. V2 separa el envío y la transmisión en pasos distintos:


- `send()` envía su mensaje
- `stream()` transmite la respuesta


Esta separación explícita facilita agregar lógica entre turnos (como procesar respuestas antes de enviar seguimientos).
El ejemplo a continuación crea una sesión, envía “¡Hola!” a Claude e imprime la respuesta de texto. Utiliza [`await using`](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-2.html#using-declarations-and-explicit-resource-management) (TypeScript 5.2+) para cerrar automáticamente la sesión cuando el bloque sale. También puede llamar a `session.close()` manualmente.


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


### [​](https://code.claude.com/docs/es/agent-sdk/typescript-v2-preview#conversaci%C3%B3n-de-m%C3%BAltiples-turnos) Conversación de múltiples turnos


Las sesiones persisten el contexto en múltiples intercambios. Para continuar una conversación, llame a `send()` nuevamente en la misma sesión. Claude recuerda los turnos anteriores.
Este ejemplo hace una pregunta matemática y luego hace un seguimiento que hace referencia a la respuesta anterior:


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


### [​](https://code.claude.com/docs/es/agent-sdk/typescript-v2-preview#reanudaci%C3%B3n-de-sesi%C3%B3n) Reanudación de sesión


Si tiene un ID de sesión de una interacción anterior, puede reanudarlo más tarde. Esto es útil para flujos de trabajo de larga duración o cuando necesita persistir conversaciones entre reinicios de aplicaciones.
Este ejemplo crea una sesión, almacena su ID, la cierra y luego reanuda la conversación:


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


### [​](https://code.claude.com/docs/es/agent-sdk/typescript-v2-preview#limpieza) Limpieza


Las sesiones se pueden cerrar manualmente o automáticamente usando [`await using`](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-2.html#using-declarations-and-explicit-resource-management), una característica de TypeScript 5.2+ para la limpieza automática de recursos. Si está utilizando una versión anterior de TypeScript o encuentra problemas de compatibilidad, use la limpieza manual en su lugar.
**Limpieza automática (TypeScript 5.2+):**


```
import { unstable_v2_createSession } from "@anthropic-ai/claude-agent-sdk";

await using session = unstable_v2_createSession({
  model: "claude-opus-4-7"
});
// Session closes automatically when the block exits
```


**Limpieza manual:**


```
import { unstable_v2_createSession } from "@anthropic-ai/claude-agent-sdk";

const session = unstable_v2_createSession({
  model: "claude-opus-4-7"
});
// ... use the session ...
session.close();
```


## [​](https://code.claude.com/docs/es/agent-sdk/typescript-v2-preview#referencia-de-api) Referencia de API


### [​](https://code.claude.com/docs/es/agent-sdk/typescript-v2-preview#unstable_v2_createsession) `unstable_v2_createSession()`


Crea una nueva sesión para conversaciones de múltiples turnos.


```
function unstable_v2_createSession(options: {
  model: string;
  // Additional options supported
}): SDKSession;
```


### [​](https://code.claude.com/docs/es/agent-sdk/typescript-v2-preview#unstable_v2_resumesession) `unstable_v2_resumeSession()`


Reanuda una sesión existente por ID.


```
function unstable_v2_resumeSession(
  sessionId: string,
  options: {
    model: string;
    // Additional options supported
  }
): SDKSession;
```


### [​](https://code.claude.com/docs/es/agent-sdk/typescript-v2-preview#unstable_v2_prompt) `unstable_v2_prompt()`


Función de conveniencia de un solo turno para consultas de un solo turno.


```
function unstable_v2_prompt(
  prompt: string,
  options: {
    model: string;
    // Additional options supported
  }
): Promise<SDKResultMessage>;
```


### [​](https://code.claude.com/docs/es/agent-sdk/typescript-v2-preview#interfaz-sdksession) Interfaz SDKSession


```
interface SDKSession {
  readonly sessionId: string;
  send(message: string | SDKUserMessage): Promise<void>;
  stream(): AsyncGenerator<SDKMessage, void>;
  close(): void;
}
```


## [​](https://code.claude.com/docs/es/agent-sdk/typescript-v2-preview#disponibilidad-de-caracter%C3%ADsticas) Disponibilidad de características


No todas las características de V1 están disponibles en V2 aún. Lo siguiente requiere usar el [SDK V1](https://code.claude.com/docs/es/agent-sdk/typescript):


- Bifurcación de sesiones (opción `forkSession`)
- Algunos patrones avanzados de entrada de transmisión


## [​](https://code.claude.com/docs/es/agent-sdk/typescript-v2-preview#comentarios) Comentarios


Comparta sus comentarios sobre la interfaz V2 antes de que se vuelva estable. Informe de problemas y sugerencias a través de [GitHub Issues](https://github.com/anthropics/claude-code/issues).


## [​](https://code.claude.com/docs/es/agent-sdk/typescript-v2-preview#v%C3%A9ase-tambi%C3%A9n) Véase también


- [Referencia del SDK TypeScript (V1)](https://code.claude.com/docs/es/agent-sdk/typescript) - Documentación completa del SDK V1
- [Descripción general del SDK](https://code.claude.com/docs/es/agent-sdk/overview) - Conceptos generales del SDK
- [Ejemplos de V2 en GitHub](https://github.com/anthropics/claude-agent-sdk-demos/tree/main/hello-world-v2) - Ejemplos de código funcionales[Claude Code Docs home page](https://code.claude.com/docs/es/overview)

[Privacy choices](https://code.claude.com/docs/es/agent-sdk/typescript-v2-preview#)

