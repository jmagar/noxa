# Guida rapida
## ​Prerequisiti
## ​Configurazione
## ​Crea un file buggy
## ​Costruisci un agente che trova e corregge i bug
## ​Concetti chiave
## ​Risoluzione dei problemi
## ​Passaggi successivi








Inizia con l’Agent SDK per Python o TypeScript per creare agenti AI che funzionano autonomamente

Utilizza l’Agent SDK per creare un agente AI che legge il tuo codice, trova i bug e li corregge, il tutto senza intervento manuale.
**Quello che farai:**


1. Configurare un progetto con l’Agent SDK
2. Creare un file con del codice buggy
3. Eseguire un agente che trova e corregge automaticamente i bug


## [​](https://code.claude.com/docs/it/agent-sdk/quickstart#prerequisiti) Prerequisiti


- **Node.js 18+** o **Python 3.10+**
- Un **account Anthropic** ([iscriviti qui](https://platform.claude.com/))


## [​](https://code.claude.com/docs/it/agent-sdk/quickstart#configurazione) Configurazione


1

Crea una cartella di progetto

Crea una nuova directory per questa guida rapida:

```
mkdir my-agent && cd my-agent
```

Per i tuoi progetti, puoi eseguire l’SDK da qualsiasi cartella; avrà accesso ai file in quella directory e nelle sue sottodirectory per impostazione predefinita. 2

Installa l'SDK

Installa il pacchetto Agent SDK per il tuo linguaggio:

- TypeScript
- Python (uv)
- Python (pip)


```
npm install @anthropic-ai/claude-agent-sdk
```

[uv Python package manager](https://docs.astral.sh/uv/) è un gestore di pacchetti Python veloce che gestisce automaticamente gli ambienti virtuali:

```
uv init && uv add claude-agent-sdk
```

Crea prima un ambiente virtuale, quindi installa:

```
python3 -m venv .venv && source .venv/bin/activate
pip3 install claude-agent-sdk
```

L’SDK TypeScript raggruppa un binario Claude Code nativo per la tua piattaforma come dipendenza opzionale, quindi non è necessario installare Claude Code separatamente. 3

Imposta la tua chiave API

Ottieni una chiave API dalla [Claude Console](https://platform.claude.com/), quindi crea un file `.env` nella directory del tuo progetto:

```
ANTHROPIC_API_KEY=your-api-key
```

L’SDK supporta anche l’autenticazione tramite provider API di terze parti:

- **Amazon Bedrock**: imposta la variabile di ambiente `CLAUDE_CODE_USE_BEDROCK=1` e configura le credenziali AWS
- **Google Vertex AI**: imposta la variabile di ambiente `CLAUDE_CODE_USE_VERTEX=1` e configura le credenziali Google Cloud
- **Microsoft Azure**: imposta la variabile di ambiente `CLAUDE_CODE_USE_FOUNDRY=1` e configura le credenziali Azure

Consulta le guide di configurazione per [Bedrock](https://code.claude.com/docs/it/amazon-bedrock), [Vertex AI](https://code.claude.com/docs/it/google-vertex-ai), o [Azure AI Foundry](https://code.claude.com/docs/it/microsoft-foundry) per i dettagli. Se non precedentemente approvato, Anthropic non consente agli sviluppatori di terze parti di offrire il login claude.ai o limiti di velocità per i loro prodotti, inclusi gli agenti costruiti su Agent SDK di Claude. Utilizza invece i metodi di autenticazione con chiave API descritti in questo documento.


## [​](https://code.claude.com/docs/it/agent-sdk/quickstart#crea-un-file-buggy) Crea un file buggy


Questa guida rapida ti guida attraverso la creazione di un agente che può trovare e correggere i bug nel codice. Per prima cosa, hai bisogno di un file con alcuni bug intenzionali che l’agente possa correggere. Crea `utils.py` nella directory `my-agent` e incolla il seguente codice:


```
def calculate_average(numbers):
    total = 0
    for num in numbers:
        total += num
    return total / len(numbers)


def get_user_name(user):
    return user["name"].upper()
```


Questo codice ha due bug:


1. `calculate_average([])` si arresta in modo anomalo con una divisione per zero
2. `get_user_name(None)` si arresta in modo anomalo con un TypeError


## [​](https://code.claude.com/docs/it/agent-sdk/quickstart#costruisci-un-agente-che-trova-e-corregge-i-bug) Costruisci un agente che trova e corregge i bug


Crea `agent.py` se stai utilizzando l’SDK Python, o `agent.ts` per TypeScript:
Python TypeScript

```
import asyncio
from claude_agent_sdk import query, ClaudeAgentOptions, AssistantMessage, ResultMessage


async def main():
    # Agentic loop: streams messages as Claude works
    async for message in query(
        prompt="Review utils.py for bugs that would cause crashes. Fix any issues you find.",
        options=ClaudeAgentOptions(
            allowed_tools=["Read", "Edit", "Glob"],  # Tools Claude can use
            permission_mode="acceptEdits",  # Auto-approve file edits
        ),
    ):
        # Print human-readable output
        if isinstance(message, AssistantMessage):
            for block in message.content:
                if hasattr(block, "text"):
                    print(block.text)  # Claude's reasoning
                elif hasattr(block, "name"):
                    print(f"Tool: {block.name}")  # Tool being called
        elif isinstance(message, ResultMessage):
            print(f"Done: {message.subtype}")  # Final result


asyncio.run(main())
```


Questo codice ha tre parti principali:


1. **`query`**: il punto di ingresso principale che crea il loop agentico. Restituisce un iteratore asincrono, quindi usi `async for` per trasmettere i messaggi mentre Claude lavora. Vedi l’API completa nel riferimento SDK [Python](https://code.claude.com/docs/it/agent-sdk/python#query) o [TypeScript](https://code.claude.com/docs/it/agent-sdk/typescript#query).
2. **`prompt`**: quello che vuoi che Claude faccia. Claude capisce quali strumenti usare in base al compito.
3. **`options`**: configurazione per l’agente. Questo esempio utilizza `allowedTools` per pre-approvare `Read`, `Edit` e `Glob`, e `permissionMode: "acceptEdits"` per auto-approvare i cambiamenti ai file. Altre opzioni includono `systemPrompt`, `mcpServers` e altro. Vedi tutte le opzioni per [Python](https://code.claude.com/docs/it/agent-sdk/python#claude-agent-options) o [TypeScript](https://code.claude.com/docs/it/agent-sdk/typescript#options).


Il loop `async for` continua a funzionare mentre Claude pensa, chiama strumenti, osserva i risultati e decide cosa fare dopo. Ogni iterazione produce un messaggio: il ragionamento di Claude, una chiamata a uno strumento, un risultato dello strumento, o il risultato finale. L’SDK gestisce l’orchestrazione (esecuzione dello strumento, gestione del contesto, tentativi) quindi consumi semplicemente il flusso. Il loop termina quando Claude completa il compito o incontra un errore.
La gestione dei messaggi all’interno del loop filtra l’output leggibile dall’uomo. Senza filtraggio, vedresti oggetti messaggio grezzi inclusa l’inizializzazione del sistema e lo stato interno, il che è utile per il debug ma rumoroso altrimenti.
Questo esempio utilizza lo streaming per mostrare i progressi in tempo reale. Se non hai bisogno di output dal vivo (ad esempio per lavori in background o pipeline CI), puoi raccogliere tutti i messaggi contemporaneamente. Vedi [Streaming vs. modalità single-turn](https://code.claude.com/docs/it/agent-sdk/streaming-vs-single-mode) per i dettagli.


### [​](https://code.claude.com/docs/it/agent-sdk/quickstart#esegui-il-tuo-agente) Esegui il tuo agente


Il tuo agente è pronto. Eseguilo con il seguente comando:


- Python
- TypeScript


```
python3 agent.py
```


```
npx tsx agent.ts
```


Dopo l’esecuzione, controlla `utils.py`. Vedrai codice difensivo che gestisce elenchi vuoti e utenti nulli. Il tuo agente autonomamente:


1. **Ha letto** `utils.py` per comprendere il codice
2. **Ha analizzato** la logica e identificato i casi limite che causerebbero arresti anomali
3. **Ha modificato** il file per aggiungere la gestione corretta degli errori


Questo è ciò che rende diverso l’Agent SDK: Claude esegue gli strumenti direttamente invece di chiederti di implementarli.
Se vedi “API key not found”, assicurati di aver impostato la variabile di ambiente `ANTHROPIC_API_KEY` nel tuo file `.env` o nell’ambiente della shell. Vedi la [guida completa alla risoluzione dei problemi](https://code.claude.com/docs/it/troubleshooting) per ulteriore aiuto.


### [​](https://code.claude.com/docs/it/agent-sdk/quickstart#prova-altri-prompt) Prova altri prompt


Ora che il tuo agente è configurato, prova alcuni prompt diversi:


- `"Add docstrings to all functions in utils.py"`
- `"Add type hints to all functions in utils.py"`
- `"Create a README.md documenting the functions in utils.py"`


### [​](https://code.claude.com/docs/it/agent-sdk/quickstart#personalizza-il-tuo-agente) Personalizza il tuo agente


Puoi modificare il comportamento del tuo agente cambiando le opzioni. Ecco alcuni esempi:
**Aggiungi capacità di ricerca web:**
Python TypeScript

```
options = ClaudeAgentOptions(
    allowed_tools=["Read", "Edit", "Glob", "WebSearch"], permission_mode="acceptEdits"
)
```


**Dai a Claude un prompt di sistema personalizzato:**
Python TypeScript

```
options = ClaudeAgentOptions(
    allowed_tools=["Read", "Edit", "Glob"],
    permission_mode="acceptEdits",
    system_prompt="You are a senior Python developer. Always follow PEP 8 style guidelines.",
)
```


**Esegui comandi nel terminale:**
Python TypeScript

```
options = ClaudeAgentOptions(
    allowed_tools=["Read", "Edit", "Glob", "Bash"], permission_mode="acceptEdits"
)
```


Con `Bash` abilitato, prova: `"Write unit tests for utils.py, run them, and fix any failures"`


## [​](https://code.claude.com/docs/it/agent-sdk/quickstart#concetti-chiave) Concetti chiave


**Tools** controllano cosa può fare il tuo agente:


| Tools | Cosa può fare l’agente |
| --- | --- |
| `Read`, `Glob`, `Grep` | Analisi di sola lettura |
| `Read`, `Edit`, `Glob` | Analizzare e modificare il codice |
| `Read`, `Edit`, `Bash`, `Glob`, `Grep` | Automazione completa |


**Permission modes** controllano quanto controllo umano desideri:


| Mode | Comportamento | Caso d’uso |
| --- | --- | --- |
| `acceptEdits` | Auto-approva le modifiche ai file e i comandi comuni del file system, chiede per altre azioni | Flussi di lavoro di sviluppo affidabili |
| `dontAsk` | Nega tutto ciò che non è in `allowedTools` | Agenti headless bloccati |
| `auto` (solo TypeScript) | Un classificatore di modelli approva o nega ogni chiamata di strumento | Agenti autonomi con protezioni di sicurezza |
| `bypassPermissions` | Esegue ogni strumento senza prompt | CI sandbox, ambienti completamente affidabili |
| `default` | Richiede un callback `canUseTool` per gestire l’approvazione | Flussi di approvazione personalizzati |


L’esempio sopra utilizza la modalità `acceptEdits`, che auto-approva le operazioni sui file in modo che l’agente possa funzionare senza prompt interattivi. Se desideri richiedere agli utenti l’approvazione, utilizza la modalità `default` e fornisci un callback [`canUseTool`](https://code.claude.com/docs/it/agent-sdk/user-input) che raccoglie l’input dell’utente. Per un maggiore controllo, vedi [Permissions](https://code.claude.com/docs/it/agent-sdk/permissions).


## [​](https://code.claude.com/docs/it/agent-sdk/quickstart#risoluzione-dei-problemi) Risoluzione dei problemi


### [​](https://code.claude.com/docs/it/agent-sdk/quickstart#errore-api-thinking-type-enabled-non-%C3%A8-supportato-per-questo-modello) Errore API `thinking.type.enabled` non è supportato per questo modello


Claude Opus 4.7 sostituisce `thinking.type.enabled` con `thinking.type.adaptive`. Le versioni precedenti di Agent SDK falliscono con il seguente errore API quando selezioni `claude-opus-4-7`:


```
API Error: 400 {"type":"invalid_request_error","message":"\"thinking.type.enabled\" is not supported for this model. Use \"thinking.type.adaptive\" and \"output_config.effort\" to control thinking behavior."}
```


Aggiorna a Agent SDK v0.2.111 o successivo per utilizzare Opus 4.7.


## [​](https://code.claude.com/docs/it/agent-sdk/quickstart#passaggi-successivi) Passaggi successivi


Ora che hai creato il tuo primo agente, scopri come estendere le sue capacità e adattarlo al tuo caso d’uso:


- **[Permissions](https://code.claude.com/docs/it/agent-sdk/permissions)**: controlla cosa può fare il tuo agente e quando ha bisogno di approvazione
- **[Hooks](https://code.claude.com/docs/it/agent-sdk/hooks)**: esegui codice personalizzato prima o dopo le chiamate agli strumenti
- **[Sessions](https://code.claude.com/docs/it/agent-sdk/sessions)**: costruisci agenti multi-turn che mantengono il contesto
- **[MCP servers](https://code.claude.com/docs/it/agent-sdk/mcp)**: connettiti a database, browser, API e altri sistemi esterni
- **[Hosting](https://code.claude.com/docs/it/agent-sdk/hosting)**: distribuisci agenti a Docker, cloud e CI/CD
- **[Example agents](https://github.com/anthropics/claude-agent-sdk-demos)**: vedi esempi completi: assistente email, agente di ricerca e altro[Claude Code Docs home page](https://code.claude.com/docs/it/overview)

[Privacy choices](https://code.claude.com/docs/it/agent-sdk/quickstart#)

