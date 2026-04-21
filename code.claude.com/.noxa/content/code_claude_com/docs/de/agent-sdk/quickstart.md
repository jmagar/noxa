Verwenden Sie das Agent SDK, um einen KI-Agenten zu erstellen, der Ihren Code liest, Fehler findet und behebt – alles ohne manuelle Eingriffe.
**Das werden Sie tun:**


1. Ein Projekt mit dem Agent SDK einrichten
2. Eine Datei mit fehlerhaftem Code erstellen
3. Einen Agenten ausführen, der Fehler automatisch findet und behebt


## [​](https://code.claude.com/docs/de/agent-sdk/quickstart#voraussetzungen) Voraussetzungen


- **Node.js 18+** oder **Python 3.10+**
- Ein **Anthropic-Konto** ([hier registrieren](https://platform.claude.com/))


## [​](https://code.claude.com/docs/de/agent-sdk/quickstart#einrichtung) Einrichtung
## ​Voraussetzungen
## ​Einrichtung
## ​Erstellen Sie eine fehlerhafte Datei
## ​Erstellen Sie einen Agenten, der Fehler findet und behebt
## ​Wichtige Konzepte
## ​Fehlerbehebung
## ​Nächste Schritte









1

Erstellen Sie einen Projektordner

Erstellen Sie ein neues Verzeichnis für diesen Schnellstart:

```
mkdir my-agent && cd my-agent
```

Für Ihre eigenen Projekte können Sie das SDK aus jedem Ordner ausführen; es hat standardmäßig Zugriff auf Dateien in diesem Verzeichnis und seinen Unterverzeichnissen. 2

Installieren Sie das SDK

Installieren Sie das Agent SDK-Paket für Ihre Sprache:

- TypeScript
- Python (uv)
- Python (pip)


```
npm install @anthropic-ai/claude-agent-sdk
```

[uv Python-Paketmanager](https://docs.astral.sh/uv/) ist ein schneller Python-Paketmanager, der virtuelle Umgebungen automatisch verwaltet:

```
uv init && uv add claude-agent-sdk
```

Erstellen Sie zunächst eine virtuelle Umgebung und installieren Sie dann:

```
python3 -m venv .venv && source .venv/bin/activate
pip3 install claude-agent-sdk
```

Das TypeScript SDK bündelt eine native Claude Code-Binärdatei für Ihre Plattform als optionale Abhängigkeit, sodass Sie Claude Code nicht separat installieren müssen. 3

Legen Sie Ihren API-Schlüssel fest

Rufen Sie einen API-Schlüssel von der [Claude-Konsole](https://platform.claude.com/) ab und erstellen Sie dann eine `.env`-Datei in Ihrem Projektverzeichnis:

```
ANTHROPIC_API_KEY=your-api-key
```

Das SDK unterstützt auch Authentifizierung über Drittanbieter-API-Anbieter:

- **Amazon Bedrock**: Setzen Sie die Umgebungsvariable `CLAUDE_CODE_USE_BEDROCK=1` und konfigurieren Sie AWS-Anmeldedaten
- **Google Vertex AI**: Setzen Sie die Umgebungsvariable `CLAUDE_CODE_USE_VERTEX=1` und konfigurieren Sie Google Cloud-Anmeldedaten
- **Microsoft Azure**: Setzen Sie die Umgebungsvariable `CLAUDE_CODE_USE_FOUNDRY=1` und konfigurieren Sie Azure-Anmeldedaten

Weitere Informationen finden Sie in den Einrichtungsleitfäden für [Bedrock](https://code.claude.com/docs/de/amazon-bedrock), [Vertex AI](https://code.claude.com/docs/de/google-vertex-ai) oder [Azure AI Foundry](https://code.claude.com/docs/de/microsoft-foundry). Sofern nicht zuvor genehmigt, erlaubt Anthropic Drittentwicklern nicht, claude.ai-Anmeldungen oder Ratenlimits für ihre Produkte anzubieten, einschließlich Agenten, die auf dem Claude Agent SDK basieren. Verwenden Sie stattdessen die in diesem Dokument beschriebenen API-Schlüssel-Authentifizierungsmethoden.


## [​](https://code.claude.com/docs/de/agent-sdk/quickstart#erstellen-sie-eine-fehlerhafte-datei) Erstellen Sie eine fehlerhafte Datei


Dieser Schnellstart führt Sie durch die Erstellung eines Agenten, der Fehler im Code finden und beheben kann. Zunächst benötigen Sie eine Datei mit einigen absichtlichen Fehlern, die der Agent beheben kann. Erstellen Sie `utils.py` im Verzeichnis `my-agent` und fügen Sie den folgenden Code ein:


```
def calculate_average(numbers):
    total = 0
    for num in numbers:
        total += num
    return total / len(numbers)


def get_user_name(user):
    return user["name"].upper()
```


Dieser Code hat zwei Fehler:


1. `calculate_average([])` stürzt mit Division durch Null ab
2. `get_user_name(None)` stürzt mit einem TypeError ab


## [​](https://code.claude.com/docs/de/agent-sdk/quickstart#erstellen-sie-einen-agenten-der-fehler-findet-und-behebt) Erstellen Sie einen Agenten, der Fehler findet und behebt


Erstellen Sie `agent.py`, wenn Sie das Python SDK verwenden, oder `agent.ts` für TypeScript:
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


Dieser Code hat drei Hauptteile:


1. **`query`**: der Haupteinstiegspunkt, der die agentic loop erstellt. Er gibt einen asynchronen Iterator zurück, daher verwenden Sie `async for`, um Nachrichten zu streamen, während Claude arbeitet. Siehe die vollständige API in der [Python](https://code.claude.com/docs/de/agent-sdk/python#query) oder [TypeScript](https://code.claude.com/docs/de/agent-sdk/typescript#query) SDK-Referenz.
2. **`prompt`**: was Sie Claude tun möchten. Claude ermittelt basierend auf der Aufgabe, welche Tools verwendet werden sollen.
3. **`options`**: Konfiguration für den Agenten. Dieses Beispiel verwendet `allowedTools`, um `Read`, `Edit` und `Glob` vorab zu genehmigen, und `permissionMode: "acceptEdits"`, um Dateiänderungen automatisch zu genehmigen. Weitere Optionen sind `systemPrompt`, `mcpServers` und mehr. Siehe alle Optionen für [Python](https://code.claude.com/docs/de/agent-sdk/python#claude-agent-options) oder [TypeScript](https://code.claude.com/docs/de/agent-sdk/typescript#options).


Die `async for`-Schleife läuft weiter, während Claude denkt, Tools aufruft, Ergebnisse beobachtet und entscheidet, was als nächstes zu tun ist. Jede Iteration ergibt eine Nachricht: Claudes Überlegung, ein Tool-Aufruf, ein Tool-Ergebnis oder das endgültige Ergebnis. Das SDK verwaltet die Orchestrierung (Tool-Ausführung, Kontextverwaltung, Wiederholungen), sodass Sie einfach den Stream verbrauchen. Die Schleife endet, wenn Claude die Aufgabe abschließt oder auf einen Fehler stößt.
Die Nachrichtenbehandlung in der Schleife filtert nach benutzerfreundlicher Ausgabe. Ohne Filterung würden Sie rohe Nachrichtenobjekte sehen, einschließlich Systeminitialisierung und internem Status, was zum Debuggen nützlich ist, aber sonst störend wirkt.
Dieses Beispiel verwendet Streaming, um den Fortschritt in Echtzeit anzuzeigen. Wenn Sie keine Live-Ausgabe benötigen (z. B. für Hintergrundaufträge oder CI-Pipelines), können Sie alle Nachrichten auf einmal sammeln. Weitere Informationen finden Sie unter [Streaming vs. Single-Turn-Modus](https://code.claude.com/docs/de/agent-sdk/streaming-vs-single-mode).


### [​](https://code.claude.com/docs/de/agent-sdk/quickstart#f%C3%BChren-sie-ihren-agenten-aus) Führen Sie Ihren Agenten aus


Ihr Agent ist bereit. Führen Sie ihn mit dem folgenden Befehl aus:


- Python
- TypeScript


```
python3 agent.py
```


```
npx tsx agent.ts
```


Nach der Ausführung überprüfen Sie `utils.py`. Sie sehen defensiven Code, der leere Listen und Null-Benutzer verarbeitet. Ihr Agent hat autonom:


1. **Gelesen** `utils.py`, um den Code zu verstehen
2. **Analysiert** die Logik und identifiziert Grenzfälle, die zum Absturz führen würden
3. **Bearbeitet** die Datei, um ordnungsgemäße Fehlerbehandlung hinzuzufügen


Das macht das Agent SDK anders: Claude führt Tools direkt aus, anstatt Sie zu bitten, sie zu implementieren.
Wenn Sie „API-Schlüssel nicht gefunden” sehen, stellen Sie sicher, dass Sie die Umgebungsvariable `ANTHROPIC_API_KEY` in Ihrer `.env`-Datei oder Shell-Umgebung gesetzt haben. Weitere Hilfe finden Sie im [vollständigen Fehlerbehebungsleitfaden](https://code.claude.com/docs/de/troubleshooting).


### [​](https://code.claude.com/docs/de/agent-sdk/quickstart#versuchen-sie-andere-prompts) Versuchen Sie andere Prompts


Jetzt, da Ihr Agent eingerichtet ist, versuchen Sie einige verschiedene Prompts:


- `"Add docstrings to all functions in utils.py"`
- `"Add type hints to all functions in utils.py"`
- `"Create a README.md documenting the functions in utils.py"`


### [​](https://code.claude.com/docs/de/agent-sdk/quickstart#passen-sie-ihren-agenten-an) Passen Sie Ihren Agenten an


Sie können das Verhalten Ihres Agenten ändern, indem Sie die Optionen ändern. Hier sind einige Beispiele:
**Fügen Sie Web-Suchfunktion hinzu:**
Python TypeScript

```
options = ClaudeAgentOptions(
    allowed_tools=["Read", "Edit", "Glob", "WebSearch"], permission_mode="acceptEdits"
)
```


**Geben Sie Claude einen benutzerdefinierten System-Prompt:**
Python TypeScript

```
options = ClaudeAgentOptions(
    allowed_tools=["Read", "Edit", "Glob"],
    permission_mode="acceptEdits",
    system_prompt="You are a senior Python developer. Always follow PEP 8 style guidelines.",
)
```


**Führen Sie Befehle im Terminal aus:**
Python TypeScript

```
options = ClaudeAgentOptions(
    allowed_tools=["Read", "Edit", "Glob", "Bash"], permission_mode="acceptEdits"
)
```


Mit aktiviertem `Bash` versuchen Sie: `"Write unit tests for utils.py, run them, and fix any failures"`


## [​](https://code.claude.com/docs/de/agent-sdk/quickstart#wichtige-konzepte) Wichtige Konzepte


**Tools** steuern, was Ihr Agent tun kann:


| Tools | Was der Agent tun kann |
| --- | --- |
| `Read`, `Glob`, `Grep` | Schreibgeschützte Analyse |
| `Read`, `Edit`, `Glob` | Code analysieren und ändern |
| `Read`, `Edit`, `Bash`, `Glob`, `Grep` | Vollständige Automatisierung |


**Genehmigungsmodi** steuern, wie viel menschliche Aufsicht Sie möchten:


| Modus | Verhalten | Anwendungsfall |
| --- | --- | --- |
| `acceptEdits` | Genehmigt Dateibearbeitungen und häufige Dateisystembefehle automatisch, fragt nach anderen Aktionen | Vertrauenswürdige Entwicklungs-Workflows |
| `dontAsk` | Lehnt alles ab, das nicht in `allowedTools` enthalten ist | Gesperrte Headless-Agenten |
| `auto` (nur TypeScript) | Ein Modell-Klassifizierer genehmigt oder lehnt jeden Tool-Aufruf ab | Autonome Agenten mit Sicherheitsvorkehrungen |
| `bypassPermissions` | Führt jedes Tool ohne Eingabeaufforderungen aus | Sandboxed CI, vollständig vertrauenswürdige Umgebungen |
| `default` | Erfordert einen `canUseTool`-Callback zur Genehmigungsbehandlung | Benutzerdefinierte Genehmigungsabläufe |


Das obige Beispiel verwendet den `acceptEdits`-Modus, der Dateivorgänge automatisch genehmigt, damit der Agent ohne interaktive Eingabeaufforderungen ausgeführt werden kann. Wenn Sie Benutzer zur Genehmigung auffordern möchten, verwenden Sie den `default`-Modus und stellen Sie einen [`canUseTool`-Callback](https://code.claude.com/docs/de/agent-sdk/user-input) bereit, der Benutzereingaben sammelt. Für mehr Kontrolle siehe [Berechtigungen](https://code.claude.com/docs/de/agent-sdk/permissions).


## [​](https://code.claude.com/docs/de/agent-sdk/quickstart#fehlerbehebung) Fehlerbehebung


### [​](https://code.claude.com/docs/de/agent-sdk/quickstart#api-fehler-thinking-type-enabled-wird-f%C3%BCr-dieses-modell-nicht-unterst%C3%BCtzt) API-Fehler `thinking.type.enabled` wird für dieses Modell nicht unterstützt


Claude Opus 4.7 ersetzt `thinking.type.enabled` durch `thinking.type.adaptive`. Ältere Agent SDK-Versionen schlagen mit dem folgenden API-Fehler fehl, wenn Sie `claude-opus-4-7` auswählen:


```
API Error: 400 {"type":"invalid_request_error","message":"\"thinking.type.enabled\" is not supported for this model. Use \"thinking.type.adaptive\" and \"output_config.effort\" to control thinking behavior."}
```


Aktualisieren Sie auf Agent SDK v0.2.111 oder später, um Opus 4.7 zu verwenden.


## [​](https://code.claude.com/docs/de/agent-sdk/quickstart#n%C3%A4chste-schritte) Nächste Schritte


Jetzt, da Sie Ihren ersten Agenten erstellt haben, erfahren Sie, wie Sie seine Funktionen erweitern und ihn an Ihren Anwendungsfall anpassen:


- **[Berechtigungen](https://code.claude.com/docs/de/agent-sdk/permissions)**: Steuern Sie, was Ihr Agent tun kann und wann er Genehmigung benötigt
- **[Hooks](https://code.claude.com/docs/de/agent-sdk/hooks)**: Führen Sie benutzerdefinierten Code vor oder nach Tool-Aufrufen aus
- **[Sitzungen](https://code.claude.com/docs/de/agent-sdk/sessions)**: Erstellen Sie Multi-Turn-Agenten, die den Kontext beibehalten
- **[MCP-Server](https://code.claude.com/docs/de/agent-sdk/mcp)**: Verbinden Sie sich mit Datenbanken, Browsern, APIs und anderen externen Systemen
- **[Hosting](https://code.claude.com/docs/de/agent-sdk/hosting)**: Stellen Sie Agenten in Docker, Cloud und CI/CD bereit
- **[Beispiel-Agenten](https://github.com/anthropics/claude-agent-sdk-demos)**: Siehe vollständige Beispiele: E-Mail-Assistent, Forschungsagent und mehr[Claude Code Docs home page](https://code.claude.com/docs/de/overview)

[Privacy choices](https://code.claude.com/docs/de/agent-sdk/quickstart#)

