# Hosting des Agent SDK
## ​Hosting-Anforderungen
## ​Verständnis der SDK-Architektur
## ​Sandbox-Anbieter-Optionen
## ​Produktionsbereitstellungsmuster
## ​Häufig gestellte Fragen
## ​Nächste Schritte







Bereitstellung und Hosting des Claude Agent SDK in Produktionsumgebungen

Das Claude Agent SDK unterscheidet sich von traditionellen zustandslosen LLM-APIs dadurch, dass es den Konversationszustand beibehält und Befehle in einer persistenten Umgebung ausführt. Dieser Leitfaden behandelt die Architektur, Hosting-Überlegungen und Best Practices für die Bereitstellung von SDK-basierten Agenten in der Produktion.
Für Sicherheitshärtung über grundlegende Sandboxing hinaus (einschließlich Netzwerkkontrollen, Verwaltung von Anmeldedaten und Isolationsoptionen) siehe [Sichere Bereitstellung](https://code.claude.com/docs/de/agent-sdk/secure-deployment).


## [​](https://code.claude.com/docs/de/agent-sdk/hosting#hosting-anforderungen) Hosting-Anforderungen


### [​](https://code.claude.com/docs/de/agent-sdk/hosting#container-basiertes-sandboxing) Container-basiertes Sandboxing


Aus Sicherheits- und Isolationsgründen sollte das SDK in einer Sandbox-Container-Umgebung ausgeführt werden. Dies bietet Prozessisolation, Ressourcenlimits, Netzwerksteuerung und ephemere Dateisysteme.
Das SDK unterstützt auch [programmgesteuerte Sandbox-Konfiguration](https://code.claude.com/docs/de/agent-sdk/typescript#sandbox-settings) für die Befehlsausführung.


### [​](https://code.claude.com/docs/de/agent-sdk/hosting#systemanforderungen) Systemanforderungen


Jede SDK-Instanz erfordert:


- **Laufzeit-Abhängigkeiten**
  - Python 3.10+ für das Python SDK oder Node.js 18+ für das TypeScript SDK
  - Beide SDK-Pakete enthalten eine native Claude Code-Binärdatei für die Host-Plattform, daher ist keine separate Claude Code- oder Node.js-Installation für die gestartete CLI erforderlich
- **Ressourcenzuteilung**
  - Empfohlen: 1 GiB RAM, 5 GiB Festplatte und 1 CPU (variieren Sie dies je nach Aufgabe nach Bedarf)
- **Netzwerkzugriff**
  - Ausgehend HTTPS zu `api.anthropic.com`
  - Optional: Zugriff auf MCP-Server oder externe Tools


## [​](https://code.claude.com/docs/de/agent-sdk/hosting#verst%C3%A4ndnis-der-sdk-architektur) Verständnis der SDK-Architektur


Im Gegensatz zu zustandslosen API-Aufrufen funktioniert das Claude Agent SDK als ein **lang laufender Prozess**, der:


- **Befehle ausführt** in einer persistenten Shell-Umgebung
- **Dateivorgänge verwaltet** innerhalb eines Arbeitsverzeichnisses
- **Tool-Ausführung handhabt** mit Kontext aus vorherigen Interaktionen


## [​](https://code.claude.com/docs/de/agent-sdk/hosting#sandbox-anbieter-optionen) Sandbox-Anbieter-Optionen


Mehrere Anbieter spezialisieren sich auf sichere Container-Umgebungen für KI-Code-Ausführung:


- **[Modal Sandbox](https://modal.com/docs/guide/sandbox)** - [Demo-Implementierung](https://modal.com/docs/examples/claude-slack-gif-creator)
- **[Cloudflare Sandboxes](https://github.com/cloudflare/sandbox-sdk)**
- **[Daytona](https://www.daytona.io/)**
- **[E2B](https://e2b.dev/)**
- **[Fly Machines](https://fly.io/docs/machines/)**
- **[Vercel Sandbox](https://vercel.com/docs/functions/sandbox)**


Für selbst gehostete Optionen (Docker, gVisor, Firecracker) und detaillierte Isolationskonfiguration siehe [Isolationstechnologien](https://code.claude.com/docs/de/agent-sdk/secure-deployment#isolation-technologies).


## [​](https://code.claude.com/docs/de/agent-sdk/hosting#produktionsbereitstellungsmuster) Produktionsbereitstellungsmuster


### [​](https://code.claude.com/docs/de/agent-sdk/hosting#muster-1-ephemere-sitzungen) Muster 1: Ephemere Sitzungen


Erstellen Sie einen neuen Container für jede Benutzeraufgabe und zerstören Sie ihn nach Abschluss.
Am besten für einmalige Aufgaben, der Benutzer kann weiterhin mit der KI interagieren, während die Aufgabe abgeschlossen wird, aber nach Abschluss wird der Container zerstört.
**Beispiele:**


- Fehleruntersuchung und -behebung: Debuggen und Beheben eines spezifischen Problems mit relevantem Kontext
- Rechnungsverarbeitung: Extrahieren und Strukturieren von Daten aus Quittungen/Rechnungen für Buchhaltungssysteme
- Übersetzungsaufgaben: Übersetzen von Dokumenten oder Inhaltschargen zwischen Sprachen
- Bild-/Videobearbeitung: Anwenden von Transformationen, Optimierungen oder Extrahieren von Metadaten aus Mediendateien


### [​](https://code.claude.com/docs/de/agent-sdk/hosting#muster-2-lang-laufende-sitzungen) Muster 2: Lang laufende Sitzungen


Behalten Sie persistente Container-Instanzen für lang laufende Aufgaben bei. Oft laufen *mehrere* Claude Agent-Prozesse innerhalb des Containers basierend auf Bedarf.
Am besten für proaktive Agenten, die ohne Benutzereingabe handeln, Agenten, die Inhalte bereitstellen, oder Agenten, die große Mengen an Nachrichten verarbeiten.
**Beispiele:**


- E-Mail-Agent: Überwacht eingehende E-Mails und sortiert, antwortet oder ergreift automatisch Maßnahmen basierend auf dem Inhalt
- Website-Builder: Hostet benutzerdefinierte Websites pro Benutzer mit Live-Bearbeitungsfunktionen, die über Container-Ports bereitgestellt werden
- Hochfrequente Chat-Bots: Verarbeitet kontinuierliche Nachrichtenströme von Plattformen wie Slack, wo schnelle Reaktionszeiten entscheidend sind


### [​](https://code.claude.com/docs/de/agent-sdk/hosting#muster-3-hybrid-sitzungen) Muster 3: Hybrid-Sitzungen


Ephemere Container, die mit Verlauf und Zustand hydratisiert werden, möglicherweise aus einer Datenbank oder aus den Sitzungswiederaufnahmefunktionen des SDK.
Am besten für Container mit intermittierender Benutzerinteraktion, die Arbeit startet und herunterfährt, wenn die Arbeit abgeschlossen ist, aber fortgesetzt werden kann.
**Beispiele:**


- Persönlicher Projektmanager: Hilft bei der Verwaltung laufender Projekte mit intermittierenden Check-ins, behält den Kontext von Aufgaben, Entscheidungen und Fortschritt
- Tiefgehende Recherche: Führt mehrstündige Recherchaufgaben durch, speichert Erkenntnisse und setzt die Untersuchung fort, wenn der Benutzer zurückkehrt
- Kundenservice-Agent: Verarbeitet Support-Tickets, die mehrere Interaktionen umfassen, lädt Ticket-Verlauf und Kundenkontext


### [​](https://code.claude.com/docs/de/agent-sdk/hosting#muster-4-einzelne-container) Muster 4: Einzelne Container


Führen Sie mehrere Claude Agent SDK-Prozesse in einem globalen Container aus.
Am besten für Agenten, die eng zusammenarbeiten müssen. Dies ist wahrscheinlich das am wenigsten beliebte Muster, da Sie verhindern müssen, dass Agenten sich gegenseitig überschreiben.
**Beispiele:**


- **Simulationen**: Agenten, die in Simulationen wie Videospielen miteinander interagieren.


## [​](https://code.claude.com/docs/de/agent-sdk/hosting#h%C3%A4ufig-gestellte-fragen) Häufig gestellte Fragen


### [​](https://code.claude.com/docs/de/agent-sdk/hosting#wie-kommuniziere-ich-mit-meinen-sandboxes) Wie kommuniziere ich mit meinen Sandboxes?


Beim Hosting in Containern müssen Sie Ports freigeben, um mit Ihren SDK-Instanzen zu kommunizieren. Ihre Anwendung kann HTTP/WebSocket-Endpunkte für externe Clients freigeben, während das SDK intern im Container ausgeführt wird.


### [​](https://code.claude.com/docs/de/agent-sdk/hosting#was-kostet-das-hosting-eines-containers) Was kostet das Hosting eines Containers?


Die dominanten Kosten für die Bereitstellung von Agenten sind die Token; Container variieren je nachdem, was Sie bereitstellen, aber die Mindestkosten liegen bei etwa 5 Cent pro Stunde Laufzeit.


### [​](https://code.claude.com/docs/de/agent-sdk/hosting#wann-sollte-ich-unt%C3%A4tige-container-herunterfahren-und-wann-sollte-ich-sie-warm-halten) Wann sollte ich untätige Container herunterfahren und wann sollte ich sie warm halten?


Dies ist wahrscheinlich anbieterabhängig, verschiedene Sandbox-Anbieter ermöglichen es Ihnen, unterschiedliche Kriterien für Leerlauf-Timeouts festzulegen, nach denen eine Sandbox möglicherweise heruntergefahren wird.
Sie sollten diesen Timeout basierend darauf abstimmen, wie häufig Sie denken, dass eine Benutzerantwort erfolgen könnte.


### [​](https://code.claude.com/docs/de/agent-sdk/hosting#wie-oft-sollte-ich-die-claude-code-cli-aktualisieren) Wie oft sollte ich die Claude Code CLI aktualisieren?


Die Claude Code CLI wird mit Semver versioniert, daher werden alle Breaking Changes versioniert.


### [​](https://code.claude.com/docs/de/agent-sdk/hosting#wie-%C3%BCberwache-ich-die-container-integrit%C3%A4t-und-die-agent-leistung) Wie überwache ich die Container-Integrität und die Agent-Leistung?


Da Container nur Server sind, funktioniert die gleiche Logging-Infrastruktur, die Sie für das Backend verwenden, auch für Container.


### [​](https://code.claude.com/docs/de/agent-sdk/hosting#wie-lange-kann-eine-agent-sitzung-laufen-bevor-sie-ausf%C3%A4llt) Wie lange kann eine Agent-Sitzung laufen, bevor sie ausfällt?


Eine Agent-Sitzung wird nicht ausfallen, aber erwägen Sie, eine ‘maxTurns’-Eigenschaft festzulegen, um zu verhindern, dass Claude in einer Schleife stecken bleibt.


## [​](https://code.claude.com/docs/de/agent-sdk/hosting#n%C3%A4chste-schritte) Nächste Schritte


- [Sichere Bereitstellung](https://code.claude.com/docs/de/agent-sdk/secure-deployment) - Netzwerkkontrollen, Verwaltung von Anmeldedaten und Isolationshärtung
- [TypeScript SDK - Sandbox-Einstellungen](https://code.claude.com/docs/de/agent-sdk/typescript#sandbox-settings) - Konfigurieren Sie die Sandbox programmgesteuert
- [Sitzungsleitfaden](https://code.claude.com/docs/de/agent-sdk/sessions) - Erfahren Sie mehr über Sitzungsverwaltung
- [Berechtigungen](https://code.claude.com/docs/de/agent-sdk/permissions) - Konfigurieren Sie Tool-Berechtigungen
- [Kostenverfolgung](https://code.claude.com/docs/de/agent-sdk/cost-tracking) - Überwachen Sie die API-Nutzung
- [MCP-Integration](https://code.claude.com/docs/de/agent-sdk/mcp) - Erweitern Sie mit benutzerdefinierten Tools[Claude Code Docs home page](https://code.claude.com/docs/de/overview)

[Privacy choices](https://code.claude.com/docs/de/agent-sdk/hosting#)

