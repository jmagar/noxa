# Hosting dell'Agent SDK
## ​Requisiti di Hosting
## ​Comprensione dell’Architettura dell’SDK
## ​Opzioni di Provider Sandbox
## ​Pattern di Distribuzione in Produzione
## ​FAQ
## ​Passaggi Successivi







Distribuisci e ospita Claude Agent SDK in ambienti di produzione

Claude Agent SDK differisce dalle tradizionali API LLM senza stato in quanto mantiene lo stato conversazionale ed esegue comandi in un ambiente persistente. Questa guida copre l’architettura, le considerazioni di hosting e le best practice per distribuire agenti basati su SDK in produzione.
Per l’indurimento della sicurezza oltre il sandboxing di base (inclusi i controlli di rete, la gestione delle credenziali e le opzioni di isolamento), vedi [Secure Deployment](https://code.claude.com/docs/it/agent-sdk/secure-deployment).


## [​](https://code.claude.com/docs/it/agent-sdk/hosting#requisiti-di-hosting) Requisiti di Hosting


### [​](https://code.claude.com/docs/it/agent-sdk/hosting#sandboxing-basato-su-container) Sandboxing Basato su Container


Per la sicurezza e l’isolamento, l’SDK dovrebbe essere eseguito all’interno di un ambiente container sandbox. Questo fornisce isolamento dei processi, limiti di risorse, controllo di rete e filesystem effimeri.
L’SDK supporta anche [configurazione sandbox programmatica](https://code.claude.com/docs/it/agent-sdk/typescript#sandbox-settings) per l’esecuzione dei comandi.


### [​](https://code.claude.com/docs/it/agent-sdk/hosting#requisiti-di-sistema) Requisiti di Sistema


Ogni istanza SDK richiede:


- **Dipendenze di runtime**
  - Python 3.10+ per Python SDK, o Node.js 18+ per TypeScript SDK
  - Entrambi i pacchetti SDK includono un binario Claude Code nativo per la piattaforma host, quindi non è necessaria un’installazione separata di Claude Code o Node.js per la CLI generata
- **Allocazione di risorse**
  - Consigliato: 1GiB RAM, 5GiB di disco e 1 CPU (varia questo in base al tuo compito secondo necessità)
- **Accesso di rete**
  - HTTPS in uscita a `api.anthropic.com`
  - Opzionale: Accesso ai server MCP o strumenti esterni


## [​](https://code.claude.com/docs/it/agent-sdk/hosting#comprensione-dell%E2%80%99architettura-dell%E2%80%99sdk) Comprensione dell’Architettura dell’SDK


A differenza delle chiamate API senza stato, Claude Agent SDK opera come un **processo a lunga esecuzione** che:


- **Esegue comandi** in un ambiente shell persistente
- **Gestisce operazioni su file** all’interno di una directory di lavoro
- **Gestisce l’esecuzione di strumenti** con contesto dalle interazioni precedenti


## [​](https://code.claude.com/docs/it/agent-sdk/hosting#opzioni-di-provider-sandbox) Opzioni di Provider Sandbox


Diversi provider si specializzano in ambienti container sicuri per l’esecuzione di codice AI:


- **[Modal Sandbox](https://modal.com/docs/guide/sandbox)** - [implementazione demo](https://modal.com/docs/examples/claude-slack-gif-creator)
- **[Cloudflare Sandboxes](https://github.com/cloudflare/sandbox-sdk)**
- **[Daytona](https://www.daytona.io/)**
- **[E2B](https://e2b.dev/)**
- **[Fly Machines](https://fly.io/docs/machines/)**
- **[Vercel Sandbox](https://vercel.com/docs/functions/sandbox)**


Per opzioni self-hosted (Docker, gVisor, Firecracker) e configurazione dettagliata dell’isolamento, vedi [Isolation Technologies](https://code.claude.com/docs/it/agent-sdk/secure-deployment#isolation-technologies).


## [​](https://code.claude.com/docs/it/agent-sdk/hosting#pattern-di-distribuzione-in-produzione) Pattern di Distribuzione in Produzione


### [​](https://code.claude.com/docs/it/agent-sdk/hosting#pattern-1-sessioni-effimere) Pattern 1: Sessioni Effimere


Crea un nuovo container per ogni compito dell’utente, quindi distruggilo al completamento.
Ideale per compiti una tantum, l’utente può ancora interagire con l’AI mentre il compito è in corso di completamento, ma una volta completato il container viene distrutto.
**Esempi:**


- Bug Investigation & Fix: Esegui il debug e risolvi un problema specifico con contesto rilevante
- Invoice Processing: Estrai e struttura i dati da ricevute/fatture per i sistemi contabili
- Translation Tasks: Traduci documenti o batch di contenuti tra lingue
- Image/Video Processing: Applica trasformazioni, ottimizzazioni o estrai metadati da file multimediali


### [​](https://code.claude.com/docs/it/agent-sdk/hosting#pattern-2-sessioni-a-lunga-esecuzione) Pattern 2: Sessioni a Lunga Esecuzione


Mantieni istanze di container persistenti per compiti a lunga esecuzione. Spesso esegui *molteplici* processi Claude Agent all’interno del container in base alla domanda.
Ideale per agenti proattivi che agiscono senza l’input dell’utente, agenti che servono contenuti o agenti che elaborano grandi quantità di messaggi.
**Esempi:**


- Email Agent: Monitora i messaggi di posta in arrivo e autonomamente li smista, risponde o intraprende azioni in base al contenuto
- Site Builder: Ospita siti web personalizzati per utente con capacità di editing dal vivo servite attraverso porte container
- High-Frequency Chat Bots: Gestisce flussi di messaggi continui da piattaforme come Slack dove i tempi di risposta rapidi sono critici


### [​](https://code.claude.com/docs/it/agent-sdk/hosting#pattern-3-sessioni-ibride) Pattern 3: Sessioni Ibride


Container effimeri che vengono idratati con cronologia e stato, possibilmente da un database o dalle funzionalità di ripresa della sessione dell’SDK.
Ideale per container con interazione intermittente dall’utente che avvia il lavoro e si spegne quando il lavoro è completato ma può essere continuato.
**Esempi:**


- Personal Project Manager: Aiuta a gestire progetti in corso con check-in intermittenti, mantiene il contesto di compiti, decisioni e progressi
- Deep Research: Conduce compiti di ricerca multi-ora, salva i risultati e riprende l’indagine quando l’utente ritorna
- Customer Support Agent: Gestisce ticket di supporto che si estendono su più interazioni, carica la cronologia dei ticket e il contesto del cliente


### [​](https://code.claude.com/docs/it/agent-sdk/hosting#pattern-4-container-singoli) Pattern 4: Container Singoli


Esegui molteplici processi Claude Agent SDK in un container globale.
Ideale per agenti che devono collaborare strettamente insieme. Questo è probabilmente il pattern meno popolare perché dovrai impedire agli agenti di sovrascrivere l’uno l’altro.
**Esempi:**


- **Simulations**: Agenti che interagiscono tra loro in simulazioni come videogiochi.


## [​](https://code.claude.com/docs/it/agent-sdk/hosting#faq) FAQ


### [​](https://code.claude.com/docs/it/agent-sdk/hosting#come-comunico-con-i-miei-sandbox) Come comunico con i miei sandbox?


Quando ospiti in container, esponi porte per comunicare con le tue istanze SDK. La tua applicazione può esporre endpoint HTTP/WebSocket per client esterni mentre l’SDK viene eseguito internamente all’interno del container.


### [​](https://code.claude.com/docs/it/agent-sdk/hosting#qual-%C3%A8-il-costo-dell%E2%80%99hosting-di-un-container) Qual è il costo dell’hosting di un container?


Il costo dominante della gestione di agenti è i token; i container variano in base a quello che provisioni, ma un costo minimo è approssimativamente 5 centesimi all’ora di esecuzione.


### [​](https://code.claude.com/docs/it/agent-sdk/hosting#quando-dovrei-spegnere-i-container-inattivi-rispetto-a-mantenerli-caldi) Quando dovrei spegnere i container inattivi rispetto a mantenerli caldi?


Questo è probabilmente dipendente dal provider, diversi provider sandbox ti permetteranno di impostare criteri diversi per i timeout di inattività dopo i quali un sandbox potrebbe spegnersi.
Vorrai sintonizzare questo timeout in base a quanto frequentemente pensi che la risposta dell’utente potrebbe essere.


### [​](https://code.claude.com/docs/it/agent-sdk/hosting#con-quale-frequenza-dovrei-aggiornare-claude-code-cli) Con quale frequenza dovrei aggiornare Claude Code CLI?


Claude Code CLI è versionato con semver, quindi eventuali modifiche di rottura saranno versionate.


### [​](https://code.claude.com/docs/it/agent-sdk/hosting#come-monitoro-la-salute-del-container-e-le-prestazioni-dell%E2%80%99agente) Come monitoro la salute del container e le prestazioni dell’agente?


Poiché i container sono solo server, la stessa infrastruttura di logging che usi per il backend funzionerà per i container.


### [​](https://code.claude.com/docs/it/agent-sdk/hosting#quanto-tempo-pu%C3%B2-durare-una-sessione-di-agente-prima-del-timeout) Quanto tempo può durare una sessione di agente prima del timeout?


Una sessione di agente non scadrà, ma considera di impostare una proprietà ‘maxTurns’ per impedire a Claude di rimanere bloccato in un ciclo.


## [​](https://code.claude.com/docs/it/agent-sdk/hosting#passaggi-successivi) Passaggi Successivi


- [Secure Deployment](https://code.claude.com/docs/it/agent-sdk/secure-deployment) - Controlli di rete, gestione delle credenziali e indurimento dell’isolamento
- [TypeScript SDK - Sandbox Settings](https://code.claude.com/docs/it/agent-sdk/typescript#sandbox-settings) - Configura sandbox programmaticamente
- [Sessions Guide](https://code.claude.com/docs/it/agent-sdk/sessions) - Scopri la gestione delle sessioni
- [Permissions](https://code.claude.com/docs/it/agent-sdk/permissions) - Configura i permessi degli strumenti
- [Cost Tracking](https://code.claude.com/docs/it/agent-sdk/cost-tracking) - Monitora l’utilizzo dell’API
- [MCP Integration](https://code.claude.com/docs/it/agent-sdk/mcp) - Estendi con strumenti personalizzati[Claude Code Docs home page](https://code.claude.com/docs/it/overview)

[Privacy choices](https://code.claude.com/docs/it/agent-sdk/hosting#)

