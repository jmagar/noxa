# Présentation du SDK Agent
## ​Commencer
## ​Capacités
## ​Comparer le SDK Agent à d’autres outils Claude
## ​Journal des modifications
## ​Signaler les bugs
## ​Directives de marque
## ​Licence et conditions
## ​Prochaines étapes









Créez des agents IA de production avec Claude Code en tant que bibliothèque

Le SDK Claude Code a été renommé en SDK Claude Agent. Si vous migrez depuis l’ancien SDK, consultez le [Guide de migration](https://code.claude.com/docs/fr/agent-sdk/migration-guide).
Créez des agents IA qui lisent autonomement les fichiers, exécutent des commandes, recherchent sur le web, modifient le code, et bien plus. Le SDK Agent vous offre les mêmes outils, boucle d’agent et gestion du contexte qui alimentent Claude Code, programmables en Python et TypeScript.
Opus 4.7 ( `claude-opus-4-7`) nécessite le SDK Agent v0.2.111 ou ultérieur. Si vous voyez une erreur API `thinking.type.enabled`, consultez [Dépannage](https://code.claude.com/docs/fr/agent-sdk/quickstart#troubleshooting).
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


Le SDK Agent inclut des outils intégrés pour lire les fichiers, exécuter des commandes et modifier le code, afin que votre agent puisse commencer à travailler immédiatement sans que vous ayez besoin d’implémenter l’exécution des outils. Plongez dans le guide de démarrage rapide ou explorez des agents réels construits avec le SDK :


## Guide de démarrage rapide

Créez un agent de correction de bugs en quelques minutes

## Agents d'exemple

Assistant e-mail, agent de recherche, et bien plus


## [​](https://code.claude.com/docs/fr/agent-sdk/overview#commencer) Commencer


1

Installer le SDK


- TypeScript
- Python


```
npm install @anthropic-ai/claude-agent-sdk
```


```
pip install claude-agent-sdk
```

Le SDK TypeScript regroupe un binaire Claude Code natif pour votre plateforme en tant que dépendance optionnelle, vous n’avez donc pas besoin d’installer Claude Code séparément. 2

Définir votre clé API

Obtenez une clé API à partir de la [Console](https://platform.claude.com/), puis définissez-la comme variable d’environnement :

```
export ANTHROPIC_API_KEY=your-api-key
```

Le SDK prend également en charge l’authentification via des fournisseurs d’API tiers :

- **Amazon Bedrock** : définissez la variable d’environnement `CLAUDE_CODE_USE_BEDROCK=1` et configurez les identifiants AWS
- **Google Vertex AI** : définissez la variable d’environnement `CLAUDE_CODE_USE_VERTEX=1` et configurez les identifiants Google Cloud
- **Microsoft Azure** : définissez la variable d’environnement `CLAUDE_CODE_USE_FOUNDRY=1` et configurez les identifiants Azure

Consultez les guides de configuration pour [Bedrock](https://code.claude.com/docs/fr/amazon-bedrock), [Vertex AI](https://code.claude.com/docs/fr/google-vertex-ai), ou [Azure AI Foundry](https://code.claude.com/docs/fr/microsoft-foundry) pour plus de détails. Sauf approbation préalable, Anthropic n’autorise pas les développeurs tiers à proposer la connexion claude.ai ou les limites de débit pour leurs produits, y compris les agents construits sur le SDK Claude Agent. Veuillez utiliser les méthodes d’authentification par clé API décrites dans ce document à la place. 3

Exécuter votre premier agent

Cet exemple crée un agent qui liste les fichiers de votre répertoire courant en utilisant les outils intégrés. Python TypeScript

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


**Prêt à construire ?** Suivez le [Guide de démarrage rapide](https://code.claude.com/docs/fr/agent-sdk/quickstart) pour créer un agent qui trouve et corrige les bugs en quelques minutes.


## [​](https://code.claude.com/docs/fr/agent-sdk/overview#capacit%C3%A9s) Capacités


Tout ce qui rend Claude Code puissant est disponible dans le SDK :


- Outils intégrés
- Hooks
- Sous-agents
- MCP
- Permissions
- Sessions

Votre agent peut lire des fichiers, exécuter des commandes et rechercher dans les bases de code dès le départ. Les outils clés incluent :

| Outil | Ce qu’il fait |
| --- | --- |
| **Read** | Lire n’importe quel fichier du répertoire de travail |
| **Write** | Créer de nouveaux fichiers |
| **Edit** | Effectuer des modifications précises aux fichiers existants |
| **Bash** | Exécuter des commandes de terminal, des scripts, des opérations git |
| **Monitor** | Surveiller un script en arrière-plan et réagir à chaque ligne de sortie en tant qu’événement |
| **Glob** | Trouver des fichiers par motif ( `**/*.ts`, `src/**/*.py`) |
| **Grep** | Rechercher le contenu des fichiers avec regex |
| **WebSearch** | Rechercher sur le web pour obtenir des informations actuelles |
| **WebFetch** | Récupérer et analyser le contenu des pages web |
| **[AskUserQuestion](https://code.claude.com/docs/fr/agent-sdk/user-input#handle-clarifying-questions)** | Poser à l’utilisateur des questions de clarification avec des options à choix multiples |

Cet exemple crée un agent qui recherche les commentaires TODO dans votre base de code : Python TypeScript

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

Exécutez du code personnalisé à des points clés du cycle de vie de l’agent. Les hooks du SDK utilisent des fonctions de rappel pour valider, enregistrer, bloquer ou transformer le comportement de l’agent. **Hooks disponibles :** `PreToolUse`, `PostToolUse`, `Stop`, `SessionStart`, `SessionEnd`, `UserPromptSubmit`, et bien d’autres. Cet exemple enregistre tous les changements de fichiers dans un fichier d’audit : Python TypeScript

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

[En savoir plus sur les hooks →](https://code.claude.com/docs/fr/agent-sdk/hooks) Générez des agents spécialisés pour gérer des sous-tâches ciblées. Votre agent principal délègue le travail, et les sous-agents rapportent les résultats. Définissez des agents personnalisés avec des instructions spécialisées. Incluez `Agent` dans `allowedTools` puisque les sous-agents sont invoqués via l’outil Agent : Python TypeScript

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

Les messages provenant du contexte d’un sous-agent incluent un champ `parent_tool_use_id`, ce qui vous permet de suivre les messages appartenant à l’exécution de quel sous-agent. [En savoir plus sur les sous-agents →](https://code.claude.com/docs/fr/agent-sdk/subagents) Connectez-vous à des systèmes externes via le Model Context Protocol : bases de données, navigateurs, API, et [des centaines d’autres](https://github.com/modelcontextprotocol/servers). Cet exemple connecte le [serveur Playwright MCP](https://github.com/microsoft/playwright-mcp) pour donner à votre agent des capacités d’automatisation de navigateur : Python TypeScript

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

[En savoir plus sur MCP →](https://code.claude.com/docs/fr/agent-sdk/mcp) Contrôlez exactement quels outils votre agent peut utiliser. Autorisez les opérations sûres, bloquez les opérations dangereuses, ou exigez une approbation pour les actions sensibles. Pour les invites d’approbation interactives et l’outil `AskUserQuestion`, consultez [Gérer les approbations et l’entrée utilisateur](https://code.claude.com/docs/fr/agent-sdk/user-input). Cet exemple crée un agent en lecture seule qui peut analyser mais pas modifier le code. `allowed_tools` pré-approuve `Read`, `Glob`, et `Grep`. Python TypeScript

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

[En savoir plus sur les permissions →](https://code.claude.com/docs/fr/agent-sdk/permissions) Maintenez le contexte sur plusieurs échanges. Claude se souvient des fichiers lus, de l’analyse effectuée et de l’historique de la conversation. Reprenez les sessions plus tard, ou divisez-les pour explorer différentes approches. Cet exemple capture l’ID de session de la première requête, puis reprend pour continuer avec le contexte complet : Python TypeScript

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

[En savoir plus sur les sessions →](https://code.claude.com/docs/fr/agent-sdk/sessions)


### [​](https://code.claude.com/docs/fr/agent-sdk/overview#fonctionnalit%C3%A9s-de-claude-code) Fonctionnalités de Claude Code


Le SDK prend également en charge la configuration basée sur le système de fichiers de Claude Code. Avec les options par défaut, le SDK les charge à partir de `.claude/` dans votre répertoire de travail et `~/.claude/`. Pour restreindre les sources qui se chargent, définissez `setting_sources` (Python) ou `settingSources` (TypeScript) dans vos options.


| Fonctionnalité | Description | Emplacement |
| --- | --- | --- |
| [Skills](https://code.claude.com/docs/fr/agent-sdk/skills) | Capacités spécialisées définies en Markdown | `.claude/skills/*/SKILL.md` |
| [Slash commands](https://code.claude.com/docs/fr/agent-sdk/slash-commands) | Commandes personnalisées pour les tâches courantes | `.claude/commands/*.md` |
| [Memory](https://code.claude.com/docs/fr/agent-sdk/modifying-system-prompts) | Contexte du projet et instructions | `CLAUDE.md` ou `.claude/CLAUDE.md` |
| [Plugins](https://code.claude.com/docs/fr/agent-sdk/plugins) | Étendre avec des commandes personnalisées, des agents et des serveurs MCP | Programmatique via l’option `plugins` |


## [​](https://code.claude.com/docs/fr/agent-sdk/overview#comparer-le-sdk-agent-%C3%A0-d%E2%80%99autres-outils-claude) Comparer le SDK Agent à d’autres outils Claude


La plateforme Claude offre plusieurs façons de construire avec Claude. Voici comment le SDK Agent s’intègre :


- SDK Agent vs SDK Client
- SDK Agent vs CLI Claude Code

Le [SDK Client Anthropic](https://platform.claude.com/docs/en/api/client-sdks) vous donne un accès direct à l’API : vous envoyez des invites et implémentez vous-même l’exécution des outils. Le **SDK Agent** vous donne Claude avec l’exécution des outils intégrée. Avec le SDK Client, vous implémentez une boucle d’outils. Avec le SDK Agent, Claude la gère : Python TypeScript

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

Mêmes capacités, interface différente :

| Cas d’usage | Meilleur choix |
| --- | --- |
| Développement interactif | CLI |
| Pipelines CI/CD | SDK |
| Applications personnalisées | SDK |
| Tâches ponctuelles | CLI |
| Automatisation de production | SDK |

De nombreuses équipes utilisent les deux : CLI pour le développement quotidien, SDK pour la production. Les flux de travail se traduisent directement entre eux.


## [​](https://code.claude.com/docs/fr/agent-sdk/overview#journal-des-modifications) Journal des modifications


Consultez le journal des modifications complet pour les mises à jour du SDK, les corrections de bugs et les nouvelles fonctionnalités :


- **SDK TypeScript** : [voir CHANGELOG.md](https://github.com/anthropics/claude-agent-sdk-typescript/blob/main/CHANGELOG.md)
- **SDK Python** : [voir CHANGELOG.md](https://github.com/anthropics/claude-agent-sdk-python/blob/main/CHANGELOG.md)


## [​](https://code.claude.com/docs/fr/agent-sdk/overview#signaler-les-bugs) Signaler les bugs


Si vous rencontrez des bugs ou des problèmes avec le SDK Agent :


- **SDK TypeScript** : [signaler les problèmes sur GitHub](https://github.com/anthropics/claude-agent-sdk-typescript/issues)
- **SDK Python** : [signaler les problèmes sur GitHub](https://github.com/anthropics/claude-agent-sdk-python/issues)


## [​](https://code.claude.com/docs/fr/agent-sdk/overview#directives-de-marque) Directives de marque


Pour les partenaires intégrant le SDK Claude Agent, l’utilisation de la marque Claude est facultative. Lorsque vous référencez Claude dans votre produit :
**Autorisé :**


- ’ Claude Agent ’ (préféré pour les menus déroulants)
- ’ Claude ’ (lorsque vous êtes déjà dans un menu étiqueté ’ Agents ’)
- ’ Powered by Claude ’ (si vous avez un nom d’agent existant)


**Non autorisé :**


- ’ Claude Code ’ ou ’ Claude Code Agent ’
- Art ASCII ou éléments visuels de marque Claude Code qui imitent Claude Code


Votre produit doit conserver sa propre marque et ne pas sembler être Claude Code ou un produit Anthropic. Pour des questions sur la conformité de la marque, contactez l’équipe [ventes](https://www.anthropic.com/contact-sales) d’Anthropic.


## [​](https://code.claude.com/docs/fr/agent-sdk/overview#licence-et-conditions) Licence et conditions


L’utilisation du SDK Claude Agent est régie par les [Conditions commerciales d’Anthropic](https://www.anthropic.com/legal/commercial-terms), y compris lorsque vous l’utilisez pour alimenter des produits et services que vous mettez à disposition de vos propres clients et utilisateurs finaux, sauf dans la mesure où un composant ou une dépendance spécifique est couvert par une licence différente comme indiqué dans le fichier LICENSE de ce composant.


## [​](https://code.claude.com/docs/fr/agent-sdk/overview#prochaines-%C3%A9tapes) Prochaines étapes


## Guide de démarrage rapide

Créez un agent qui trouve et corrige les bugs en quelques minutes

## Agents d'exemple

Assistant e-mail, agent de recherche, et bien plus

## SDK TypeScript

Référence API TypeScript complète et exemples

## SDK Python

Référence API Python complète et exemples[Claude Code Docs home page](https://code.claude.com/docs/fr/overview)

[Privacy choices](https://code.claude.com/docs/fr/agent-sdk/overview#)

