# Panoramica dell'Agent SDK
## ​Inizia
## ​Capacità
## ​Confronta l’Agent SDK con altri strumenti Claude
## ​Changelog
## ​Segnalazione di bug
## ​Linee guida di branding
## ​Licenza e termini
## ​Passaggi successivi









Costruisci agenti AI di produzione con Claude Code come libreria

L’SDK di Claude Code è stato rinominato in Claude Agent SDK. Se stai migrando dal vecchio SDK, consulta la [Guida alla migrazione](https://code.claude.com/docs/it/agent-sdk/migration-guide).
Costruisci agenti AI che leggono autonomamente file, eseguono comandi, cercano sul web, modificano codice e molto altro. L’Agent SDK ti offre gli stessi strumenti, il ciclo dell’agente e la gestione del contesto che alimentano Claude Code, programmabili in Python e TypeScript.
Opus 4.7 ( `claude-opus-4-7`) richiede Agent SDK v0.2.111 o successivo. Se vedi un errore API `thinking.type.enabled`, consulta [Troubleshooting](https://code.claude.com/docs/it/agent-sdk/quickstart#troubleshooting).
Python TypeScript

```
import asyncio
from claude_agent_sdk import query, ClaudeAgentOptions


async def main():
    async for message in query(
        prompt="Find and fix the bug in auth.py",
        options=ClaudeAgentOptions(allowed_tools=["Read", "Edit", "Bash"]),
    ):
        print(message)  # Claude reads the file, finds the bug, edits it


asyncio.run(main())
```


L’Agent SDK include strumenti integrati per leggere file, eseguire comandi e modificare codice, quindi il tuo agente può iniziare a lavorare immediatamente senza che tu implementi l’esecuzione degli strumenti. Tuffati nella guida rapida o esplora agenti reali costruiti con l’SDK:


## Quickstart

Costruisci un agente di correzione dei bug in pochi minuti

## Example agents

Assistente email, agente di ricerca e altro ancora


## [​](https://code.claude.com/docs/it/agent-sdk/overview#inizia) Inizia


1

Installa l'SDK


- TypeScript
- Python


```
npm install @anthropic-ai/claude-agent-sdk
```


```
pip install claude-agent-sdk
```

L’SDK TypeScript raggruppa un binario nativo di Claude Code per la tua piattaforma come dipendenza opzionale, quindi non è necessario installare Claude Code separatamente. 2

Imposta la tua chiave API

Ottieni una chiave API dalla [Console](https://platform.claude.com/), quindi impostala come variabile di ambiente:

```
export ANTHROPIC_API_KEY=your-api-key
```

L’SDK supporta anche l’autenticazione tramite provider API di terze parti:

- **Amazon Bedrock**: imposta la variabile di ambiente `CLAUDE_CODE_USE_BEDROCK=1` e configura le credenziali AWS
- **Google Vertex AI**: imposta la variabile di ambiente `CLAUDE_CODE_USE_VERTEX=1` e configura le credenziali di Google Cloud
- **Microsoft Azure**: imposta la variabile di ambiente `CLAUDE_CODE_USE_FOUNDRY=1` e configura le credenziali di Azure

Consulta le guide di configurazione per [Bedrock](https://code.claude.com/docs/it/amazon-bedrock), [Vertex AI](https://code.claude.com/docs/it/google-vertex-ai) o [Azure AI Foundry](https://code.claude.com/docs/it/microsoft-foundry) per i dettagli. Se non precedentemente approvato, Anthropic non consente agli sviluppatori di terze parti di offrire l’accesso a claude.ai o limiti di velocità per i loro prodotti, inclusi gli agenti costruiti su Claude Agent SDK. Utilizza invece i metodi di autenticazione con chiave API descritti in questo documento. 3

Esegui il tuo primo agente

Questo esempio crea un agente che elenca i file nella tua directory corrente utilizzando strumenti integrati. Python TypeScript

```
import asyncio
from claude_agent_sdk import query, ClaudeAgentOptions


async def main():
    async for message in query(
        prompt="What files are in this directory?",
        options=ClaudeAgentOptions(allowed_tools=["Bash", "Glob"]),
    ):
        if hasattr(message, "result"):
            print(message.result)


asyncio.run(main())
```


**Pronto a costruire?** Segui il [Quickstart](https://code.claude.com/docs/it/agent-sdk/quickstart) per creare un agente che trova e corregge i bug in pochi minuti.


## [​](https://code.claude.com/docs/it/agent-sdk/overview#capacit%C3%A0) Capacità


Tutto ciò che rende Claude Code potente è disponibile nell’SDK:


- Built-in tools
- Hooks
- Subagents
- MCP
- Permissions
- Sessions

Il tuo agente può leggere file, eseguire comandi e cercare codebase subito. Gli strumenti chiave includono:

| Tool | Cosa fa |
| --- | --- |
| **Read** | Leggi qualsiasi file nella directory di lavoro |
| **Write** | Crea nuovi file |
| **Edit** | Apporta modifiche precise ai file esistenti |
| **Bash** | Esegui comandi di terminale, script, operazioni git |
| **Monitor** | Osserva uno script in background e reagisci a ogni riga di output come evento |
| **Glob** | Trova file per pattern ( `**/*.ts`, `src/**/*.py`) |
| **Grep** | Cerca contenuti di file con regex |
| **WebSearch** | Cerca sul web informazioni attuali |
| **WebFetch** | Recupera e analizza il contenuto della pagina web |
| **[AskUserQuestion](https://code.claude.com/docs/it/agent-sdk/user-input#handle-clarifying-questions)** | Poni all’utente domande di chiarimento con opzioni a scelta multipla |

Questo esempio crea un agente che cerca nella tua codebase i commenti TODO: Python TypeScript

```
import asyncio
from claude_agent_sdk import query, ClaudeAgentOptions


async def main():
    async for message in query(
        prompt="Find all TODO comments and create a summary",
        options=ClaudeAgentOptions(allowed_tools=["Read", "Glob", "Grep"]),
    ):
        if hasattr(message, "result"):
            print(message.result)


asyncio.run(main())
```

Esegui codice personalizzato in punti chiave del ciclo di vita dell’agente. Gli hooks dell’SDK utilizzano funzioni di callback per convalidare, registrare, bloccare o trasformare il comportamento dell’agente. **Hook disponibili:** `PreToolUse`, `PostToolUse`, `Stop`, `SessionStart`, `SessionEnd`, `UserPromptSubmit` e altri. Questo esempio registra tutte le modifiche ai file in un file di audit: Python TypeScript

```
import asyncio
from datetime import datetime
from claude_agent_sdk import query, ClaudeAgentOptions, HookMatcher


async def log_file_change(input_data, tool_use_id, context):
    file_path = input_data.get("tool_input", {}).get("file_path", "unknown")
    with open("./audit.log", "a") as f:
        f.write(f"{datetime.now()}: modified {file_path}\n")
    return {}


async def main():
    async for message in query(
        prompt="Refactor utils.py to improve readability",
        options=ClaudeAgentOptions(
            permission_mode="acceptEdits",
            hooks={
                "PostToolUse": [
                    HookMatcher(matcher="Edit|Write", hooks=[log_file_change])
                ]
            },
        ),
    ):
        if hasattr(message, "result"):
            print(message.result)


asyncio.run(main())
```

[Scopri di più su hooks →](https://code.claude.com/docs/it/agent-sdk/hooks) Genera agenti specializzati per gestire sottoattività mirate. Il tuo agente principale delega il lavoro e i subagenti riferiscono i risultati. Definisci agenti personalizzati con istruzioni specializzate. Includi `Agent` in `allowedTools` poiché i subagenti vengono invocati tramite lo strumento Agent: Python TypeScript

```
import asyncio
from claude_agent_sdk import query, ClaudeAgentOptions, AgentDefinition


async def main():
    async for message in query(
        prompt="Use the code-reviewer agent to review this codebase",
        options=ClaudeAgentOptions(
            allowed_tools=["Read", "Glob", "Grep", "Agent"],
            agents={
                "code-reviewer": AgentDefinition(
                    description="Expert code reviewer for quality and security reviews.",
                    prompt="Analyze code quality and suggest improvements.",
                    tools=["Read", "Glob", "Grep"],
                )
            },
        ),
    ):
        if hasattr(message, "result"):
            print(message.result)


asyncio.run(main())
```

I messaggi dal contesto di un subagente includono un campo `parent_tool_use_id`, che ti consente di tracciare quali messaggi appartengono a quale esecuzione di subagente. [Scopri di più su subagenti →](https://code.claude.com/docs/it/agent-sdk/subagents) Connettiti a sistemi esterni tramite il Model Context Protocol: database, browser, API e [centinaia di altri](https://github.com/modelcontextprotocol/servers). Questo esempio connette il [server Playwright MCP](https://github.com/microsoft/playwright-mcp) per dare al tuo agente capacità di automazione del browser: Python TypeScript

```
import asyncio
from claude_agent_sdk import query, ClaudeAgentOptions


async def main():
    async for message in query(
        prompt="Open example.com and describe what you see",
        options=ClaudeAgentOptions(
            mcp_servers={
                "playwright": {"command": "npx", "args": ["@playwright/mcp@latest"]}
            }
        ),
    ):
        if hasattr(message, "result"):
            print(message.result)


asyncio.run(main())
```

[Scopri di più su MCP →](https://code.claude.com/docs/it/agent-sdk/mcp) Controlla esattamente quali strumenti il tuo agente può utilizzare. Consenti operazioni sicure, blocca quelle pericolose o richiedi approvazione per azioni sensibili. Per prompt di approvazione interattivi e lo strumento `AskUserQuestion`, consulta [Gestisci approvazioni e input dell’utente](https://code.claude.com/docs/it/agent-sdk/user-input). Questo esempio crea un agente di sola lettura che può analizzare ma non modificare il codice. `allowed_tools` pre-approva `Read`, `Glob` e `Grep`. Python TypeScript

```
import asyncio
from claude_agent_sdk import query, ClaudeAgentOptions


async def main():
    async for message in query(
        prompt="Review this code for best practices",
        options=ClaudeAgentOptions(
            allowed_tools=["Read", "Glob", "Grep"],
        ),
    ):
        if hasattr(message, "result"):
            print(message.result)


asyncio.run(main())
```

[Scopri di più su permessi →](https://code.claude.com/docs/it/agent-sdk/permissions) Mantieni il contesto su più scambi. Claude ricorda i file letti, l’analisi eseguita e la cronologia della conversazione. Riprendi le sessioni in seguito o dividile per esplorare approcci diversi. Questo esempio acquisisce l’ID della sessione dalla prima query, quindi riprende per continuare con il contesto completo: Python TypeScript

```
import asyncio
from claude_agent_sdk import query, ClaudeAgentOptions, SystemMessage, ResultMessage


async def main():
    session_id = None

    # First query: capture the session ID
    async for message in query(
        prompt="Read the authentication module",
        options=ClaudeAgentOptions(allowed_tools=["Read", "Glob"]),
    ):
        if isinstance(message, SystemMessage) and message.subtype == "init":
            session_id = message.data["session_id"]

    # Resume with full context from the first query
    async for message in query(
        prompt="Now find all places that call it",  # "it" = auth module
        options=ClaudeAgentOptions(resume=session_id),
    ):
        if isinstance(message, ResultMessage):
            print(message.result)


asyncio.run(main())
```

[Scopri di più su sessioni →](https://code.claude.com/docs/it/agent-sdk/sessions)


### [​](https://code.claude.com/docs/it/agent-sdk/overview#funzionalit%C3%A0-di-claude-code) Funzionalità di Claude Code


L’SDK supporta anche la configurazione basata su filesystem di Claude Code. Con le opzioni predefinite, l’SDK carica questi da `.claude/` nella tua directory di lavoro e `~/.claude/`. Per limitare quali fonti caricare, imposta `setting_sources` (Python) o `settingSources` (TypeScript) nelle tue opzioni.


| Funzionalità | Descrizione | Posizione |
| --- | --- | --- |
| [Skills](https://code.claude.com/docs/it/agent-sdk/skills) | Capacità specializzate definite in Markdown | `.claude/skills/*/SKILL.md` |
| [Slash commands](https://code.claude.com/docs/it/agent-sdk/slash-commands) | Comandi personalizzati per attività comuni | `.claude/commands/*.md` |
| [Memory](https://code.claude.com/docs/it/agent-sdk/modifying-system-prompts) | Contesto del progetto e istruzioni | `CLAUDE.md` o `.claude/CLAUDE.md` |
| [Plugins](https://code.claude.com/docs/it/agent-sdk/plugins) | Estendi con comandi personalizzati, agenti e server MCP | Programmatico tramite opzione `plugins` |


## [​](https://code.claude.com/docs/it/agent-sdk/overview#confronta-l%E2%80%99agent-sdk-con-altri-strumenti-claude) Confronta l’Agent SDK con altri strumenti Claude


La piattaforma Claude offre più modi per costruire con Claude. Ecco come si inserisce l’Agent SDK:


- Agent SDK vs Client SDK
- Agent SDK vs Claude Code CLI

L’ [Anthropic Client SDK](https://platform.claude.com/docs/en/api/client-sdks) ti offre accesso diretto all’API: invii prompt e implementi tu stesso l’esecuzione degli strumenti. L’ **Agent SDK** ti offre Claude con esecuzione degli strumenti integrata. Con il Client SDK, implementi un ciclo di strumenti. Con l’Agent SDK, Claude lo gestisce: Python TypeScript

```
# Client SDK: You implement the tool loop
response = client.messages.create(...)
while response.stop_reason == "tool_use":
    result = your_tool_executor(response.tool_use)
    response = client.messages.create(tool_result=result, **params)

# Agent SDK: Claude handles tools autonomously
async for message in query(prompt="Fix the bug in auth.py"):
    print(message)
```

Stesse capacità, interfaccia diversa:

| Caso d’uso | Scelta migliore |
| --- | --- |
| Sviluppo interattivo | CLI |
| Pipeline CI/CD | SDK |
| Applicazioni personalizzate | SDK |
| Attività una tantum | CLI |
| Automazione di produzione | SDK |

Molti team utilizzano entrambi: CLI per lo sviluppo quotidiano, SDK per la produzione. I flussi di lavoro si traducono direttamente tra loro.


## [​](https://code.claude.com/docs/it/agent-sdk/overview#changelog) Changelog


Visualizza il changelog completo per gli aggiornamenti dell’SDK, le correzioni di bug e le nuove funzionalità:


- **TypeScript SDK**: [visualizza CHANGELOG.md](https://github.com/anthropics/claude-agent-sdk-typescript/blob/main/CHANGELOG.md)
- **Python SDK**: [visualizza CHANGELOG.md](https://github.com/anthropics/claude-agent-sdk-python/blob/main/CHANGELOG.md)


## [​](https://code.claude.com/docs/it/agent-sdk/overview#segnalazione-di-bug) Segnalazione di bug


Se riscontri bug o problemi con l’Agent SDK:


- **TypeScript SDK**: [segnala i problemi su GitHub](https://github.com/anthropics/claude-agent-sdk-typescript/issues)
- **Python SDK**: [segnala i problemi su GitHub](https://github.com/anthropics/claude-agent-sdk-python/issues)


## [​](https://code.claude.com/docs/it/agent-sdk/overview#linee-guida-di-branding) Linee guida di branding


Per i partner che integrano Claude Agent SDK, l’uso del branding Claude è facoltativo. Quando fai riferimento a Claude nel tuo prodotto:
**Consentito:**


- “Claude Agent” (preferito per i menu a discesa)
- “Claude” (quando già all’interno di un menu etichettato “Agents”)
- ” Powered by Claude” (se hai un nome di agente esistente)


**Non consentito:**


- “Claude Code” o “Claude Code Agent”
- Arte ASCII con branding Claude Code o elementi visivi che imitano Claude Code


Il tuo prodotto dovrebbe mantenere il suo proprio branding e non sembrare Claude Code o alcun prodotto Anthropic. Per domande sulla conformità del branding, contatta il [team di vendita](https://www.anthropic.com/contact-sales) di Anthropic.


## [​](https://code.claude.com/docs/it/agent-sdk/overview#licenza-e-termini) Licenza e termini


L’uso di Claude Agent SDK è disciplinato dai [Termini di servizio commerciali di Anthropic](https://www.anthropic.com/legal/commercial-terms), incluso quando lo utilizzi per alimentare prodotti e servizi che metti a disposizione dei tuoi clienti e utenti finali, tranne nella misura in cui un componente o una dipendenza specifica è coperta da una licenza diversa come indicato nel file LICENSE di quel componente.


## [​](https://code.claude.com/docs/it/agent-sdk/overview#passaggi-successivi) Passaggi successivi


## Quickstart

Costruisci un agente che trova e corregge i bug in pochi minuti

## Example agents

Assistente email, agente di ricerca e altro ancora

## TypeScript SDK

Riferimento API TypeScript completo ed esempi

## Python SDK

Riferimento API Python completo ed esempi[Claude Code Docs home page](https://code.claude.com/docs/it/overview)

[Privacy choices](https://code.claude.com/docs/it/agent-sdk/overview#)

