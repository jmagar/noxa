# Interface TypeScript SDK V2 (aperçu
## ​Installation
## ​Démarrage rapide
## ​Référence API
## ​Disponibilité des fonctionnalités
## ​Commentaires
## ​Voir aussi







Aperçu du SDK Agent TypeScript V2 simplifié, avec des modèles send/stream basés sur les sessions pour les conversations multi-tours.

L’interface V2 est un **aperçu instable**. Les API peuvent changer en fonction des commentaires avant de devenir stables. Certaines fonctionnalités comme le forking de session ne sont disponibles que dans le [SDK V1](https://code.claude.com/docs/fr/agent-sdk/typescript).
Le SDK Agent TypeScript V2 de Claude supprime le besoin de générateurs asynchrones et de coordination de rendement. Cela rend les conversations multi-tours plus simples, au lieu de gérer l’état du générateur entre les tours, chaque tour est un cycle `send()`/ `stream()` séparé. La surface de l’API se réduit à trois concepts :


- `createSession()` / `resumeSession()` : Démarrer ou continuer une conversation
- `session.send()` : Envoyer un message
- `session.stream()` : Obtenir la réponse


## [​](https://code.claude.com/docs/fr/agent-sdk/typescript-v2-preview#installation) Installation


L’interface V2 est incluse dans le package SDK existant :


```
npm install @anthropic-ai/claude-agent-sdk
```


Le SDK regroupe un binaire Claude Code natif pour votre plateforme en tant que dépendance optionnelle, vous n’avez donc pas besoin d’installer Claude Code séparément.


## [​](https://code.claude.com/docs/fr/agent-sdk/typescript-v2-preview#d%C3%A9marrage-rapide) Démarrage rapide


### [​](https://code.claude.com/docs/fr/agent-sdk/typescript-v2-preview#invite-unique) Invite unique


Pour les requêtes simples à un seul tour où vous n’avez pas besoin de maintenir une session, utilisez `unstable_v2_prompt()`. Cet exemple envoie une question mathématique et enregistre la réponse :


```
import { unstable_v2_prompt } from "@anthropic-ai/claude-agent-sdk";

const result = await unstable_v2_prompt("What is 2 + 2?", {
  model: "claude-opus-4-7"
});
if (result.subtype === "success") {
  console.log(result.result);
}
```


### [​](https://code.claude.com/docs/fr/agent-sdk/typescript-v2-preview#session-de-base) Session de base


Pour les interactions au-delà d’une seule invite, créez une session. V2 sépare l’envoi et la diffusion en étapes distinctes :


- `send()` envoie votre message
- `stream()` diffuse la réponse


Cette séparation explicite facilite l’ajout de logique entre les tours (comme le traitement des réponses avant d’envoyer des suites).
L’exemple ci-dessous crée une session, envoie « Hello ! » à Claude et imprime la réponse textuelle. Il utilise [`await using`](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-2.html#using-declarations-and-explicit-resource-management) (TypeScript 5.2+) pour fermer automatiquement la session lorsque le bloc se termine. Vous pouvez également appeler `session.close()` manuellement.


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


### [​](https://code.claude.com/docs/fr/agent-sdk/typescript-v2-preview#conversation-multi-tours) Conversation multi-tours


Les sessions persistent le contexte à travers plusieurs échanges. Pour continuer une conversation, appelez `send()` à nouveau sur la même session. Claude se souvient des tours précédents.
Cet exemple pose une question mathématique, puis pose une suite qui fait référence à la réponse précédente :


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


### [​](https://code.claude.com/docs/fr/agent-sdk/typescript-v2-preview#reprise-de-session) Reprise de session


Si vous avez un ID de session d’une interaction précédente, vous pouvez le reprendre plus tard. Ceci est utile pour les flux de travail de longue durée ou lorsque vous devez persister les conversations entre les redémarrages d’application.
Cet exemple crée une session, stocke son ID, la ferme, puis reprend la conversation :


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


### [​](https://code.claude.com/docs/fr/agent-sdk/typescript-v2-preview#nettoyage) Nettoyage


Les sessions peuvent être fermées manuellement ou automatiquement en utilisant [`await using`](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-2.html#using-declarations-and-explicit-resource-management), une fonctionnalité TypeScript 5.2+ pour le nettoyage automatique des ressources. Si vous utilisez une version TypeScript plus ancienne ou rencontrez des problèmes de compatibilité, utilisez plutôt le nettoyage manuel.
**Nettoyage automatique (TypeScript 5.2+) :**


```
import { unstable_v2_createSession } from "@anthropic-ai/claude-agent-sdk";

await using session = unstable_v2_createSession({
  model: "claude-opus-4-7"
});
// Session closes automatically when the block exits
```


**Nettoyage manuel :**


```
import { unstable_v2_createSession } from "@anthropic-ai/claude-agent-sdk";

const session = unstable_v2_createSession({
  model: "claude-opus-4-7"
});
// ... use the session ...
session.close();
```


## [​](https://code.claude.com/docs/fr/agent-sdk/typescript-v2-preview#r%C3%A9f%C3%A9rence-api) Référence API


### [​](https://code.claude.com/docs/fr/agent-sdk/typescript-v2-preview#unstable_v2_createsession) `unstable_v2_createSession()`


Crée une nouvelle session pour les conversations multi-tours.


```
function unstable_v2_createSession(options: {
  model: string;
  // Additional options supported
}): SDKSession;
```


### [​](https://code.claude.com/docs/fr/agent-sdk/typescript-v2-preview#unstable_v2_resumesession) `unstable_v2_resumeSession()`


Reprend une session existante par ID.


```
function unstable_v2_resumeSession(
  sessionId: string,
  options: {
    model: string;
    // Additional options supported
  }
): SDKSession;
```


### [​](https://code.claude.com/docs/fr/agent-sdk/typescript-v2-preview#unstable_v2_prompt) `unstable_v2_prompt()`


Fonction de commodité unique pour les requêtes à un seul tour.


```
function unstable_v2_prompt(
  prompt: string,
  options: {
    model: string;
    // Additional options supported
  }
): Promise<SDKResultMessage>;
```


### [​](https://code.claude.com/docs/fr/agent-sdk/typescript-v2-preview#interface-sdksession) Interface SDKSession


```
interface SDKSession {
  readonly sessionId: string;
  send(message: string | SDKUserMessage): Promise<void>;
  stream(): AsyncGenerator<SDKMessage, void>;
  close(): void;
}
```


## [​](https://code.claude.com/docs/fr/agent-sdk/typescript-v2-preview#disponibilit%C3%A9-des-fonctionnalit%C3%A9s) Disponibilité des fonctionnalités


Toutes les fonctionnalités V1 ne sont pas encore disponibles en V2. Les éléments suivants nécessitent l’utilisation du [SDK V1](https://code.claude.com/docs/fr/agent-sdk/typescript) :


- Forking de session (option `forkSession`)
- Certains modèles de flux d’entrée avancés


## [​](https://code.claude.com/docs/fr/agent-sdk/typescript-v2-preview#commentaires) Commentaires


Partagez vos commentaires sur l’interface V2 avant qu’elle ne devienne stable. Signalez les problèmes et les suggestions via [GitHub Issues](https://github.com/anthropics/claude-code/issues).


## [​](https://code.claude.com/docs/fr/agent-sdk/typescript-v2-preview#voir-aussi) Voir aussi


- [Référence SDK TypeScript (V1)](https://code.claude.com/docs/fr/agent-sdk/typescript) - Documentation complète du SDK V1
- [Aperçu SDK](https://code.claude.com/docs/fr/agent-sdk/overview) - Concepts généraux du SDK
- [Exemples V2 sur GitHub](https://github.com/anthropics/claude-agent-sdk-demos/tree/main/hello-world-v2) - Exemples de code fonctionnels[Claude Code Docs home page](https://code.claude.com/docs/fr/overview)

[Privacy choices](https://code.claude.com/docs/fr/agent-sdk/typescript-v2-preview#)

