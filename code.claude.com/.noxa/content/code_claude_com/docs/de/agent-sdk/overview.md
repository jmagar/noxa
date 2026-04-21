# Agent SDK – Übersicht
## ​Erste Schritte
## ​Funktionen
## ​Vergleichen Sie das Agent SDK mit anderen Claude-Tools
## ​Änderungsprotokoll
## ​Fehler melden
## ​Richtlinien für die Markennutzung
## ​Lizenz und Bedingungen
## ​Nächste Schritte









Erstellen Sie produktive KI-Agenten mit Claude Code als Bibliothek

Das Claude Code SDK wurde in das Claude Agent SDK umbenannt. Wenn Sie vom alten SDK migrieren, siehe [Migrationsleitfaden](https://code.claude.com/docs/de/agent-sdk/migration-guide).
Erstellen Sie KI-Agenten, die autonom Dateien lesen, Befehle ausführen, das Web durchsuchen, Code bearbeiten und vieles mehr. Das Agent SDK bietet Ihnen die gleichen Tools, die Agent-Schleife und das Kontextmanagement, die Claude Code antreiben, programmierbar in Python und TypeScript.
Opus 4.7 ( `claude-opus-4-7`) erfordert Agent SDK v0.2.111 oder später. Wenn Sie einen `thinking.type.enabled` API-Fehler sehen, siehe [Fehlerbehebung](https://code.claude.com/docs/de/agent-sdk/quickstart#troubleshooting).
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


Das Agent SDK enthält integrierte Tools zum Lesen von Dateien, Ausführen von Befehlen und Bearbeiten von Code, sodass Ihr Agent sofort arbeiten kann, ohne dass Sie die Tool-Ausführung implementieren müssen. Tauchen Sie in den Schnellstart ein oder erkunden Sie echte Agenten, die mit dem SDK erstellt wurden:


## Schnellstart

Erstellen Sie einen Fehlerbereinigungsagenten in wenigen Minuten

## Beispielagenten

E-Mail-Assistent, Forschungsagent und mehr


## [​](https://code.claude.com/docs/de/agent-sdk/overview#erste-schritte) Erste Schritte


1

Installieren Sie das SDK


- TypeScript
- Python


```
npm install @anthropic-ai/claude-agent-sdk
```


```
pip install claude-agent-sdk
```

Das TypeScript SDK bündelt eine native Claude Code-Binärdatei für Ihre Plattform als optionale Abhängigkeit, sodass Sie Claude Code nicht separat installieren müssen. 2

Legen Sie Ihren API-Schlüssel fest

Rufen Sie einen API-Schlüssel aus der [Konsole](https://platform.claude.com/) ab und legen Sie ihn als Umgebungsvariable fest:

```
export ANTHROPIC_API_KEY=your-api-key
```

Das SDK unterstützt auch Authentifizierung über Drittanbieter-API-Anbieter:

- **Amazon Bedrock**: Setzen Sie die Umgebungsvariable `CLAUDE_CODE_USE_BEDROCK=1` und konfigurieren Sie AWS-Anmeldedaten
- **Google Vertex AI**: Setzen Sie die Umgebungsvariable `CLAUDE_CODE_USE_VERTEX=1` und konfigurieren Sie Google Cloud-Anmeldedaten
- **Microsoft Azure**: Setzen Sie die Umgebungsvariable `CLAUDE_CODE_USE_FOUNDRY=1` und konfigurieren Sie Azure-Anmeldedaten

Weitere Informationen finden Sie in den Einrichtungsleitfäden für [Bedrock](https://code.claude.com/docs/de/amazon-bedrock), [Vertex AI](https://code.claude.com/docs/de/google-vertex-ai) oder [Azure AI Foundry](https://code.claude.com/docs/de/microsoft-foundry). Sofern nicht zuvor genehmigt, erlaubt Anthropic Drittentwicklern nicht, claude.ai-Anmeldungen oder Ratenlimits für ihre Produkte anzubieten, einschließlich Agenten, die auf dem Claude Agent SDK basieren. Verwenden Sie stattdessen die in diesem Dokument beschriebenen API-Schlüssel-Authentifizierungsmethoden. 3

Führen Sie Ihren ersten Agenten aus

Dieses Beispiel erstellt einen Agenten, der Dateien in Ihrem aktuellen Verzeichnis mit integrierten Tools auflistet. Python TypeScript

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


**Bereit zum Erstellen?** Folgen Sie dem [Schnellstart](https://code.claude.com/docs/de/agent-sdk/quickstart), um einen Agenten zu erstellen, der Fehler in wenigen Minuten findet und behebt.


## [​](https://code.claude.com/docs/de/agent-sdk/overview#funktionen) Funktionen


Alles, was Claude Code leistungsstark macht, ist im SDK verfügbar:


- Integrierte Tools
- Hooks
- Subagenten
- MCP
- Berechtigungen
- Sitzungen

Ihr Agent kann Dateien lesen, Befehle ausführen und Codebases sofort durchsuchen. Wichtige Tools sind:

| Tool | Was es tut |
| --- | --- |
| **Read** | Lesen Sie jede Datei im Arbeitsverzeichnis |
| **Write** | Erstellen Sie neue Dateien |
| **Edit** | Nehmen Sie präzise Änderungen an vorhandenen Dateien vor |
| **Bash** | Führen Sie Terminalbefehle, Skripte und Git-Operationen aus |
| **Monitor** | Überwachen Sie ein Hintergrundskript und reagieren Sie auf jede Ausgabezeile als Ereignis |
| **Glob** | Suchen Sie Dateien nach Muster ( `**/*.ts`, `src/**/*.py`) |
| **Grep** | Durchsuchen Sie Dateiinhalte mit Regex |
| **WebSearch** | Durchsuchen Sie das Web nach aktuellen Informationen |
| **WebFetch** | Rufen Sie Webseiteninhalte ab und analysieren Sie sie |
| **[AskUserQuestion](https://code.claude.com/docs/de/agent-sdk/user-input#handle-clarifying-questions)** | Stellen Sie dem Benutzer Klärungsfragen mit Mehrfachauswahloptionen |

Dieses Beispiel erstellt einen Agenten, der Ihre Codebasis nach TODO-Kommentaren durchsucht: Python TypeScript

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

Führen Sie benutzerdefinierten Code an wichtigen Punkten im Agent-Lebenszyklus aus. SDK-Hooks verwenden Callback-Funktionen, um Agent-Verhalten zu validieren, zu protokollieren, zu blockieren oder zu transformieren. **Verfügbare Hooks:** `PreToolUse`, `PostToolUse`, `Stop`, `SessionStart`, `SessionEnd`, `UserPromptSubmit` und mehr. Dieses Beispiel protokolliert alle Dateiänderungen in einer Audit-Datei: Python TypeScript

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

[Weitere Informationen zu Hooks →](https://code.claude.com/docs/de/agent-sdk/hooks) Spawnen Sie spezialisierte Agenten, um fokussierte Teilaufgaben zu bewältigen. Ihr Hauptagent delegiert Arbeit, und Subagenten berichten mit Ergebnissen zurück. Definieren Sie benutzerdefinierte Agenten mit spezialisierten Anweisungen. Fügen Sie `Agent` in `allowedTools` ein, da Subagenten über das Agent-Tool aufgerufen werden: Python TypeScript

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

Nachrichten aus dem Kontext eines Subagenten enthalten ein `parent_tool_use_id`-Feld, mit dem Sie verfolgen können, welche Nachrichten zu welcher Subagenten-Ausführung gehören. [Weitere Informationen zu Subagenten →](https://code.claude.com/docs/de/agent-sdk/subagents) Verbinden Sie sich mit externen Systemen über das Model Context Protocol: Datenbanken, Browser, APIs und [hunderte mehr](https://github.com/modelcontextprotocol/servers). Dieses Beispiel verbindet den [Playwright MCP-Server](https://github.com/microsoft/playwright-mcp), um Ihrem Agenten Browser-Automatisierungsfunktionen zu geben: Python TypeScript

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

[Weitere Informationen zu MCP →](https://code.claude.com/docs/de/agent-sdk/mcp) Kontrollieren Sie genau, welche Tools Ihr Agent verwenden kann. Erlauben Sie sichere Operationen, blockieren Sie gefährliche oder erfordern Sie Genehmigung für sensible Aktionen. Für interaktive Genehmigungseingabeaufforderungen und das `AskUserQuestion`-Tool siehe [Genehmigungen und Benutzereingaben verarbeiten](https://code.claude.com/docs/de/agent-sdk/user-input). Dieses Beispiel erstellt einen schreibgeschützten Agenten, der Code analysieren, aber nicht ändern kann. `allowed_tools` genehmigt `Read`, `Glob` und `Grep` vorab. Python TypeScript

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

[Weitere Informationen zu Berechtigungen →](https://code.claude.com/docs/de/agent-sdk/permissions) Behalten Sie den Kontext über mehrere Austausche hinweg bei. Claude merkt sich gelesene Dateien, durchgeführte Analysen und Gesprächsverlauf. Setzen Sie Sitzungen später fort oder verzweigen Sie sie, um verschiedene Ansätze zu erkunden. Dieses Beispiel erfasst die Sitzungs-ID aus der ersten Abfrage und setzt sie dann fort, um mit vollständigem Kontext fortzufahren: Python TypeScript

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

[Weitere Informationen zu Sitzungen →](https://code.claude.com/docs/de/agent-sdk/sessions)


### [​](https://code.claude.com/docs/de/agent-sdk/overview#claude-code-funktionen) Claude Code-Funktionen


Das SDK unterstützt auch die dateisystembasierte Konfiguration von Claude Code. Mit Standardoptionen lädt das SDK diese aus `.claude/` in Ihrem Arbeitsverzeichnis und `~/.claude/`. Um einzuschränken, welche Quellen geladen werden, setzen Sie `setting_sources` (Python) oder `settingSources` (TypeScript) in Ihren Optionen.


| Funktion | Beschreibung | Speicherort |
| --- | --- | --- |
| [Skills](https://code.claude.com/docs/de/agent-sdk/skills) | Spezialisierte Funktionen, die in Markdown definiert sind | `.claude/skills/*/SKILL.md` |
| [Slash commands](https://code.claude.com/docs/de/agent-sdk/slash-commands) | Benutzerdefinierte Befehle für häufige Aufgaben | `.claude/commands/*.md` |
| [Memory](https://code.claude.com/docs/de/agent-sdk/modifying-system-prompts) | Projektkontext und Anweisungen | `CLAUDE.md` oder `.claude/CLAUDE.md` |
| [Plugins](https://code.claude.com/docs/de/agent-sdk/plugins) | Erweitern Sie mit benutzerdefinierten Befehlen, Agenten und MCP-Servern | Programmgesteuert über `plugins`-Option |


## [​](https://code.claude.com/docs/de/agent-sdk/overview#vergleichen-sie-das-agent-sdk-mit-anderen-claude-tools) Vergleichen Sie das Agent SDK mit anderen Claude-Tools


Die Claude-Plattform bietet mehrere Möglichkeiten, mit Claude zu erstellen. So passt das Agent SDK:


- Agent SDK vs Client SDK
- Agent SDK vs Claude Code CLI

Das [Anthropic Client SDK](https://platform.claude.com/docs/en/api/client-sdks) bietet Ihnen direkten API-Zugriff: Sie senden Eingabeaufforderungen und implementieren die Tool-Ausführung selbst. Das **Agent SDK** bietet Ihnen Claude mit integrierter Tool-Ausführung. Mit dem Client SDK implementieren Sie eine Tool-Schleife. Mit dem Agent SDK handhabt Claude es: Python TypeScript

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

Gleiche Funktionen, andere Schnittstelle:

| Anwendungsfall | Beste Wahl |
| --- | --- |
| Interaktive Entwicklung | CLI |
| CI/CD-Pipelines | SDK |
| Benutzerdefinierte Anwendungen | SDK |
| Einmalige Aufgaben | CLI |
| Produktionsautomatisierung | SDK |

Viele Teams verwenden beide: CLI für die tägliche Entwicklung, SDK für die Produktion. Workflows lassen sich direkt zwischen ihnen übersetzen.


## [​](https://code.claude.com/docs/de/agent-sdk/overview#%C3%A4nderungsprotokoll) Änderungsprotokoll


Sehen Sie sich das vollständige Änderungsprotokoll für SDK-Updates, Fehlerbehebungen und neue Funktionen an:


- **TypeScript SDK**: [CHANGELOG.md anzeigen](https://github.com/anthropics/claude-agent-sdk-typescript/blob/main/CHANGELOG.md)
- **Python SDK**: [CHANGELOG.md anzeigen](https://github.com/anthropics/claude-agent-sdk-python/blob/main/CHANGELOG.md)


## [​](https://code.claude.com/docs/de/agent-sdk/overview#fehler-melden) Fehler melden


Wenn Sie auf Fehler oder Probleme mit dem Agent SDK stoßen:


- **TypeScript SDK**: [Probleme auf GitHub melden](https://github.com/anthropics/claude-agent-sdk-typescript/issues)
- **Python SDK**: [Probleme auf GitHub melden](https://github.com/anthropics/claude-agent-sdk-python/issues)


## [​](https://code.claude.com/docs/de/agent-sdk/overview#richtlinien-f%C3%BCr-die-markennutzung) Richtlinien für die Markennutzung


Für Partner, die das Claude Agent SDK integrieren, ist die Verwendung von Claude-Branding optional. Wenn Sie Claude in Ihrem Produkt referenzieren:
**Erlaubt:**


- ‘Claude Agent” (bevorzugt für Dropdown-Menüs)
- „Claude” (wenn bereits in einem Menü mit der Bezeichnung „Agents”)
- „ Powered by Claude” (wenn Sie einen vorhandenen Agentennamen haben)


**Nicht erlaubt:**


- „Claude Code” oder „Claude Code Agent”
- Claude Code-Branding ASCII-Art oder visuelle Elemente, die Claude Code nachahmen


Ihr Produkt sollte sein eigenes Branding beibehalten und nicht wie Claude Code oder ein anderes Anthropic-Produkt aussehen. Wenden Sie sich bei Fragen zur Markenkonformität an das Anthropic- [Vertriebsteam](https://www.anthropic.com/contact-sales).


## [​](https://code.claude.com/docs/de/agent-sdk/overview#lizenz-und-bedingungen) Lizenz und Bedingungen


Die Verwendung des Claude Agent SDK unterliegt den [Anthropic Commercial Terms of Service](https://www.anthropic.com/legal/commercial-terms), auch wenn Sie es verwenden, um Produkte und Dienste bereitzustellen, die Sie Ihren eigenen Kunden und Endbenutzern zur Verfügung stellen, außer soweit eine bestimmte Komponente oder Abhängigkeit unter einer anderen Lizenz abgedeckt ist, wie in der LICENSE-Datei dieser Komponente angegeben.


## [​](https://code.claude.com/docs/de/agent-sdk/overview#n%C3%A4chste-schritte) Nächste Schritte


## Schnellstart

Erstellen Sie einen Agenten, der Fehler in wenigen Minuten findet und behebt

## Beispielagenten

E-Mail-Assistent, Forschungsagent und mehr

## TypeScript SDK

Vollständige TypeScript-API-Referenz und Beispiele

## Python SDK

Vollständige Python-API-Referenz und Beispiele[Claude Code Docs home page](https://code.claude.com/docs/de/overview)

[Privacy choices](https://code.claude.com/docs/de/agent-sdk/overview#)

