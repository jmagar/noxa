# Démarrage rapide
## ​Prérequis
## ​Configuration
## ​Créer un fichier avec des bugs
## ​Créer un agent qui trouve et corrige les bugs
## ​Concepts clés
## ​Dépannage
## ​Étapes suivantes








Commencez avec le SDK Agent Python ou TypeScript pour créer des agents IA qui fonctionnent de manière autonome

Utilisez le SDK Agent pour créer un agent IA qui lit votre code, trouve les bugs et les corrige, tout sans intervention manuelle.
**Ce que vous allez faire :**


1. Configurer un projet avec le SDK Agent
2. Créer un fichier avec du code contenant des bugs
3. Exécuter un agent qui trouve et corrige les bugs automatiquement


## [​](https://code.claude.com/docs/fr/agent-sdk/quickstart#pr%C3%A9requis) Prérequis


- **Node.js 18+** ou **Python 3.10+**
- Un **compte Anthropic** ([s’inscrire ici](https://platform.claude.com/))


## [​](https://code.claude.com/docs/fr/agent-sdk/quickstart#configuration) Configuration


1

Créer un dossier de projet

Créez un nouveau répertoire pour ce démarrage rapide :

```
mkdir my-agent && cd my-agent
```

Pour vos propres projets, vous pouvez exécuter le SDK à partir de n’importe quel dossier ; il aura accès aux fichiers de ce répertoire et de ses sous-répertoires par défaut. 2

Installer le SDK

Installez le package du SDK Agent pour votre langage :

- TypeScript
- Python (uv)
- Python (pip)


```
npm install @anthropic-ai/claude-agent-sdk
```

[uv Python package manager](https://docs.astral.sh/uv/) est un gestionnaire de paquets Python rapide qui gère automatiquement les environnements virtuels :

```
uv init && uv add claude-agent-sdk
```

Créez d’abord un environnement virtuel, puis installez :

```
python3 -m venv .venv && source .venv/bin/activate
pip3 install claude-agent-sdk
```

Le SDK TypeScript regroupe un binaire Claude Code natif pour votre plateforme en tant que dépendance optionnelle, vous n’avez donc pas besoin d’installer Claude Code séparément. 3

Définir votre clé API

Obtenez une clé API à partir de la [Console Claude](https://platform.claude.com/), puis créez un fichier `.env` dans votre répertoire de projet :

```
ANTHROPIC_API_KEY=your-api-key
```

Le SDK prend également en charge l’authentification via des fournisseurs d’API tiers :

- **Amazon Bedrock** : définissez la variable d’environnement `CLAUDE_CODE_USE_BEDROCK=1` et configurez les identifiants AWS
- **Google Vertex AI** : définissez la variable d’environnement `CLAUDE_CODE_USE_VERTEX=1` et configurez les identifiants Google Cloud
- **Microsoft Azure** : définissez la variable d’environnement `CLAUDE_CODE_USE_FOUNDRY=1` et configurez les identifiants Azure

Consultez les guides de configuration pour [Bedrock](https://code.claude.com/docs/fr/amazon-bedrock), [Vertex AI](https://code.claude.com/docs/fr/google-vertex-ai) ou [Azure AI Foundry](https://code.claude.com/docs/fr/microsoft-foundry) pour plus de détails. Sauf approbation préalable, Anthropic n’autorise pas les développeurs tiers à proposer la connexion claude.ai ou les limites de débit pour leurs produits, y compris les agents construits sur le SDK Agent Claude. Veuillez utiliser les méthodes d’authentification par clé API décrites dans ce document à la place.


## [​](https://code.claude.com/docs/fr/agent-sdk/quickstart#cr%C3%A9er-un-fichier-avec-des-bugs) Créer un fichier avec des bugs


Ce démarrage rapide vous guide dans la création d’un agent capable de trouver et corriger les bugs dans le code. D’abord, vous avez besoin d’un fichier avec quelques bugs intentionnels pour que l’agent les corrige. Créez `utils.py` dans le répertoire `my-agent` et collez le code suivant :


```
def calculate_average(numbers):
    total = 0
    for num in numbers:
        total += num
    return total / len(numbers)


def get_user_name(user):
    return user["name"].upper()
```


Ce code a deux bugs :


1. `calculate_average([])` plante avec une division par zéro
2. `get_user_name(None)` plante avec une TypeError


## [​](https://code.claude.com/docs/fr/agent-sdk/quickstart#cr%C3%A9er-un-agent-qui-trouve-et-corrige-les-bugs) Créer un agent qui trouve et corrige les bugs


Créez `agent.py` si vous utilisez le SDK Python, ou `agent.ts` pour TypeScript :
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


Ce code a trois parties principales :


1. **`query`** : le point d’entrée principal qui crée la boucle agentique. Il retourne un itérateur asynchrone, vous utilisez donc `async for` pour diffuser les messages au fur et à mesure que Claude travaille. Consultez l’API complète dans la référence du SDK [Python](https://code.claude.com/docs/fr/agent-sdk/python#query) ou [TypeScript](https://code.claude.com/docs/fr/agent-sdk/typescript#query).
2. **`prompt`** : ce que vous voulez que Claude fasse. Claude détermine les outils à utiliser en fonction de la tâche.
3. **`options`** : configuration de l’agent. Cet exemple utilise `allowedTools` pour pré-approuver `Read`, `Edit` et `Glob`, et `permissionMode: "acceptEdits"` pour approuver automatiquement les modifications de fichiers. Les autres options incluent `systemPrompt`, `mcpServers` et bien d’autres. Consultez toutes les options pour [Python](https://code.claude.com/docs/fr/agent-sdk/python#claude-agent-options) ou [TypeScript](https://code.claude.com/docs/fr/agent-sdk/typescript#options).


La boucle `async for` continue de s’exécuter tandis que Claude réfléchit, appelle des outils, observe les résultats et décide de la prochaine étape. Chaque itération produit un message : le raisonnement de Claude, un appel d’outil, un résultat d’outil ou le résultat final. Le SDK gère l’orchestration (exécution des outils, gestion du contexte, tentatives) afin que vous consommiez simplement le flux. La boucle se termine lorsque Claude termine la tâche ou rencontre une erreur.
La gestion des messages à l’intérieur de la boucle filtre la sortie lisible par l’homme. Sans filtrage, vous verriez des objets de message bruts incluant l’initialisation du système et l’état interne, ce qui est utile pour le débogage mais bruyant autrement.
Cet exemple utilise la diffusion en continu pour afficher la progression en temps réel. Si vous n’avez pas besoin de sortie en direct (par exemple, pour les tâches en arrière-plan ou les pipelines CI), vous pouvez collecter tous les messages à la fois. Consultez [Mode diffusion en continu vs. mode à tour unique](https://code.claude.com/docs/fr/agent-sdk/streaming-vs-single-mode) pour plus de détails.


### [​](https://code.claude.com/docs/fr/agent-sdk/quickstart#ex%C3%A9cuter-votre-agent) Exécuter votre agent


Votre agent est prêt. Exécutez-le avec la commande suivante :


- Python
- TypeScript


```
python3 agent.py
```


```
npx tsx agent.ts
```


Après l’exécution, vérifiez `utils.py`. Vous verrez du code défensif gérant les listes vides et les utilisateurs nuls. Votre agent a autonomement :


1. **Lu** `utils.py` pour comprendre le code
2. **Analysé** la logique et identifié les cas limites qui causeraient un plantage
3. **Modifié** le fichier pour ajouter une gestion d’erreur appropriée


C’est ce qui rend le SDK Agent différent : Claude exécute les outils directement au lieu de vous demander de les implémenter.
Si vous voyez « API key not found », assurez-vous d’avoir défini la variable d’environnement `ANTHROPIC_API_KEY` dans votre fichier `.env` ou votre environnement shell. Consultez le [guide de dépannage complet](https://code.claude.com/docs/fr/troubleshooting) pour plus d’aide.


### [​](https://code.claude.com/docs/fr/agent-sdk/quickstart#essayer-d%E2%80%99autres-invites) Essayer d’autres invites


Maintenant que votre agent est configuré, essayez quelques invites différentes :


- `"Add docstrings to all functions in utils.py"`
- `"Add type hints to all functions in utils.py"`
- `"Create a README.md documenting the functions in utils.py"`


### [​](https://code.claude.com/docs/fr/agent-sdk/quickstart#personnaliser-votre-agent) Personnaliser votre agent


Vous pouvez modifier le comportement de votre agent en changeant les options. Voici quelques exemples :
**Ajouter la capacité de recherche web :**
Python TypeScript

```
options = ClaudeAgentOptions(
    allowed_tools=["Read", "Edit", "Glob", "WebSearch"], permission_mode="acceptEdits"
)
```


**Donner à Claude une invite système personnalisée :**
Python TypeScript

```
options = ClaudeAgentOptions(
    allowed_tools=["Read", "Edit", "Glob"],
    permission_mode="acceptEdits",
    system_prompt="You are a senior Python developer. Always follow PEP 8 style guidelines.",
)
```


**Exécuter des commandes dans le terminal :**
Python TypeScript

```
options = ClaudeAgentOptions(
    allowed_tools=["Read", "Edit", "Glob", "Bash"], permission_mode="acceptEdits"
)
```


Avec `Bash` activé, essayez : `"Write unit tests for utils.py, run them, and fix any failures"`


## [​](https://code.claude.com/docs/fr/agent-sdk/quickstart#concepts-cl%C3%A9s) Concepts clés


**Les outils** contrôlent ce que votre agent peut faire :


| Outils | Ce que l’agent peut faire |
| --- | --- |
| `Read`, `Glob`, `Grep` | Analyse en lecture seule |
| `Read`, `Edit`, `Glob` | Analyser et modifier le code |
| `Read`, `Edit`, `Bash`, `Glob`, `Grep` | Automatisation complète |


**Les modes de permission** contrôlent le niveau de surveillance humaine que vous souhaitez :


| Mode | Comportement | Cas d’usage |
| --- | --- | --- |
| `acceptEdits` | Approuve automatiquement les modifications de fichiers et les commandes courantes du système de fichiers, demande les autres actions | Flux de travail de développement de confiance |
| `dontAsk` | Refuse tout ce qui n’est pas dans `allowedTools` | Agents sans tête verrouillés |
| `auto` (TypeScript uniquement) | Un classificateur de modèle approuve ou refuse chaque appel d’outil | Agents autonomes avec garde-fous de sécurité |
| `bypassPermissions` | Exécute chaque outil sans invites | CI en bac à sable, environnements entièrement de confiance |
| `default` | Nécessite un rappel `canUseTool` pour gérer l’approbation | Flux d’approbation personnalisés |


L’exemple ci-dessus utilise le mode `acceptEdits`, qui approuve automatiquement les opérations de fichiers afin que l’agent puisse s’exécuter sans invites interactives. Si vous souhaitez inviter les utilisateurs à approuver, utilisez le mode `default` et fournissez un rappel [`canUseTool`](https://code.claude.com/docs/fr/agent-sdk/user-input) qui collecte l’entrée de l’utilisateur. Pour plus de contrôle, consultez [Permissions](https://code.claude.com/docs/fr/agent-sdk/permissions).


## [​](https://code.claude.com/docs/fr/agent-sdk/quickstart#d%C3%A9pannage) Dépannage


### [​](https://code.claude.com/docs/fr/agent-sdk/quickstart#erreur-api-thinking-type-enabled-n%E2%80%99est-pas-pris-en-charge-pour-ce-mod%C3%A8le) Erreur API `thinking.type.enabled` n’est pas pris en charge pour ce modèle


Claude Opus 4.7 remplace `thinking.type.enabled` par `thinking.type.adaptive`. Les versions plus anciennes du SDK Agent échouent avec l’erreur API suivante lorsque vous sélectionnez `claude-opus-4-7` :


```
API Error: 400 {"type":"invalid_request_error","message":"\"thinking.type.enabled\" is not supported for this model. Use \"thinking.type.adaptive\" and \"output_config.effort\" to control thinking behavior."}
```


Mettez à niveau vers le SDK Agent v0.2.111 ou version ultérieure pour utiliser Opus 4.7.


## [​](https://code.claude.com/docs/fr/agent-sdk/quickstart#%C3%A9tapes-suivantes) Étapes suivantes


Maintenant que vous avez créé votre premier agent, apprenez à étendre ses capacités et à l’adapter à votre cas d’usage :


- **[Permissions](https://code.claude.com/docs/fr/agent-sdk/permissions)** : contrôlez ce que votre agent peut faire et quand il a besoin d’approbation
- **[Hooks](https://code.claude.com/docs/fr/agent-sdk/hooks)** : exécutez du code personnalisé avant ou après les appels d’outils
- **[Sessions](https://code.claude.com/docs/fr/agent-sdk/sessions)** : créez des agents multi-tours qui maintiennent le contexte
- **[Serveurs MCP](https://code.claude.com/docs/fr/agent-sdk/mcp)** : connectez-vous à des bases de données, des navigateurs, des API et d’autres systèmes externes
- **[Hébergement](https://code.claude.com/docs/fr/agent-sdk/hosting)** : déployez des agents sur Docker, le cloud et CI/CD
- **[Agents d’exemple](https://github.com/anthropics/claude-agent-sdk-demos)** : consultez des exemples complets : assistant e-mail, agent de recherche et bien d’autres[Claude Code Docs home page](https://code.claude.com/docs/fr/overview)

[Privacy choices](https://code.claude.com/docs/fr/agent-sdk/quickstart#)

