# Héberger l'Agent SDK
## ​Exigences d’hébergement
## ​Comprendre l’architecture du SDK
## ​Options de fournisseur de sandbox
## ​Modèles de déploiement en production
## ​FAQ
## ​Étapes suivantes







Déployer et héberger Claude Agent SDK dans des environnements de production

Le Claude Agent SDK diffère des API LLM sans état traditionnelles en ce qu’il maintient l’état conversationnel et exécute des commandes dans un environnement persistant. Ce guide couvre l’architecture, les considérations d’hébergement et les meilleures pratiques pour déployer des agents basés sur le SDK en production.
Pour le renforcement de la sécurité au-delà du sandboxing basique (y compris les contrôles réseau, la gestion des identifiants et les options d’isolation), voir [Déploiement sécurisé](https://code.claude.com/docs/fr/agent-sdk/secure-deployment).


## [​](https://code.claude.com/docs/fr/agent-sdk/hosting#exigences-d%E2%80%99h%C3%A9bergement) Exigences d’hébergement


### [​](https://code.claude.com/docs/fr/agent-sdk/hosting#sandboxing-bas%C3%A9-sur-des-conteneurs) Sandboxing basé sur des conteneurs


Pour la sécurité et l’isolation, le SDK doit s’exécuter dans un environnement de conteneur en sandbox. Cela fournit l’isolation des processus, les limites de ressources, le contrôle réseau et les systèmes de fichiers éphémères.
Le SDK prend également en charge la [configuration de sandbox programmatique](https://code.claude.com/docs/fr/agent-sdk/typescript#sandbox-settings) pour l’exécution de commandes.


### [​](https://code.claude.com/docs/fr/agent-sdk/hosting#configuration-requise-du-syst%C3%A8me) Configuration requise du système


Chaque instance du SDK nécessite :


- **Dépendances d’exécution**
  - Python 3.10+ pour le SDK Python, ou Node.js 18+ pour le SDK TypeScript
  - Les deux packages SDK incluent un binaire Claude Code natif pour la plateforme hôte, donc aucune installation séparée de Claude Code ou Node.js n’est nécessaire pour le CLI généré
- **Allocation de ressources**
  - Recommandé : 1 GiB de RAM, 5 GiB de disque et 1 CPU (ajustez cela en fonction de votre tâche selon les besoins)
- **Accès réseau**
  - HTTPS sortant vers `api.anthropic.com`
  - Optionnel : Accès aux serveurs MCP ou aux outils externes


## [​](https://code.claude.com/docs/fr/agent-sdk/hosting#comprendre-l%E2%80%99architecture-du-sdk) Comprendre l’architecture du SDK


Contrairement aux appels API sans état, le Claude Agent SDK fonctionne comme un **processus de longue durée** qui :


- **Exécute des commandes** dans un environnement shell persistant
- **Gère les opérations de fichiers** dans un répertoire de travail
- **Gère l’exécution des outils** avec le contexte des interactions précédentes


## [​](https://code.claude.com/docs/fr/agent-sdk/hosting#options-de-fournisseur-de-sandbox) Options de fournisseur de sandbox


Plusieurs fournisseurs se spécialisent dans les environnements de conteneurs sécurisés pour l’exécution de code IA :


- **[Modal Sandbox](https://modal.com/docs/guide/sandbox)** - [implémentation de démonstration](https://modal.com/docs/examples/claude-slack-gif-creator)
- **[Cloudflare Sandboxes](https://github.com/cloudflare/sandbox-sdk)**
- **[Daytona](https://www.daytona.io/)**
- **[E2B](https://e2b.dev/)**
- **[Fly Machines](https://fly.io/docs/machines/)**
- **[Vercel Sandbox](https://vercel.com/docs/functions/sandbox)**


Pour les options auto-hébergées (Docker, gVisor, Firecracker) et la configuration détaillée de l’isolation, voir [Technologies d’isolation](https://code.claude.com/docs/fr/agent-sdk/secure-deployment#isolation-technologies).


## [​](https://code.claude.com/docs/fr/agent-sdk/hosting#mod%C3%A8les-de-d%C3%A9ploiement-en-production) Modèles de déploiement en production


### [​](https://code.claude.com/docs/fr/agent-sdk/hosting#mod%C3%A8le-1--sessions-%C3%A9ph%C3%A9m%C3%A8res) Modèle 1 : Sessions éphémères


Créez un nouveau conteneur pour chaque tâche utilisateur, puis détruisez-le une fois terminé.
Idéal pour les tâches ponctuelles, l’utilisateur peut toujours interagir avec l’IA pendant que la tâche se termine, mais une fois terminée, le conteneur est détruit.
**Exemples :**


- Enquête et correction de bogues : Déboguer et résoudre un problème spécifique avec le contexte pertinent
- Traitement des factures : Extraire et structurer les données des reçus/factures pour les systèmes comptables
- Tâches de traduction : Traduire des documents ou des lots de contenu entre les langues
- Traitement d’images/vidéos : Appliquer des transformations, des optimisations ou extraire des métadonnées à partir de fichiers médias


### [​](https://code.claude.com/docs/fr/agent-sdk/hosting#mod%C3%A8le-2--sessions-de-longue-dur%C3%A9e) Modèle 2 : Sessions de longue durée


Maintenir des instances de conteneurs persistantes pour les tâches de longue durée. Souvent, exécuter *plusieurs* processus Claude Agent à l’intérieur du conteneur en fonction de la demande.
Idéal pour les agents proactifs qui agissent sans l’entrée de l’utilisateur, les agents qui servent du contenu ou les agents qui traitent de grandes quantités de messages.
**Exemples :**


- Agent de messagerie : Surveille les e-mails entrants et trie, répond ou prend des mesures de manière autonome en fonction du contenu
- Générateur de sites : Héberge des sites Web personnalisés par utilisateur avec des capacités d’édition en direct servies via les ports du conteneur
- Chatbots haute fréquence : Gère les flux de messages continus à partir de plateformes comme Slack où les temps de réponse rapides sont critiques


### [​](https://code.claude.com/docs/fr/agent-sdk/hosting#mod%C3%A8le-3--sessions-hybrides) Modèle 3 : Sessions hybrides


Conteneurs éphémères qui sont hydratés avec l’historique et l’état, possiblement à partir d’une base de données ou des fonctionnalités de reprise de session du SDK.
Idéal pour les conteneurs avec une interaction intermittente de l’utilisateur qui lance le travail et s’arrête lorsque le travail est terminé mais peut être continué.
**Exemples :**


- Gestionnaire de projets personnels : Aide à gérer les projets en cours avec des vérifications intermittentes, maintient le contexte des tâches, des décisions et de la progression
- Recherche approfondie : Mène des tâches de recherche de plusieurs heures, enregistre les résultats et reprend l’enquête lorsque l’utilisateur revient
- Agent d’assistance client : Gère les tickets d’assistance qui s’étendent sur plusieurs interactions, charge l’historique des tickets et le contexte client


### [​](https://code.claude.com/docs/fr/agent-sdk/hosting#mod%C3%A8le-4--conteneurs-uniques) Modèle 4 : Conteneurs uniques


Exécutez plusieurs processus Claude Agent SDK dans un conteneur global unique.
Idéal pour les agents qui doivent collaborer étroitement ensemble. C’est probablement le modèle le moins populaire car vous devrez empêcher les agents de se réécrire mutuellement.
**Exemples :**


- **Simulations** : Agents qui interagissent les uns avec les autres dans des simulations telles que des jeux vidéo.


## [​](https://code.claude.com/docs/fr/agent-sdk/hosting#faq) FAQ


### [​](https://code.claude.com/docs/fr/agent-sdk/hosting#comment-communiquer-avec-mes-sandboxes-) Comment communiquer avec mes sandboxes ?


Lors de l’hébergement dans des conteneurs, exposez les ports pour communiquer avec vos instances SDK. Votre application peut exposer des points de terminaison HTTP/WebSocket pour les clients externes tandis que le SDK s’exécute en interne dans le conteneur.


### [​](https://code.claude.com/docs/fr/agent-sdk/hosting#quel-est-le-co%C3%BBt-d%E2%80%99h%C3%A9bergement-d%E2%80%99un-conteneur-) Quel est le coût d’hébergement d’un conteneur ?


Le coût dominant de la fourniture d’agents est les jetons ; les conteneurs varient en fonction de ce que vous provisionnez, mais un coût minimum est d’environ 5 cents par heure d’exécution.


### [​](https://code.claude.com/docs/fr/agent-sdk/hosting#quand-dois-je-arr%C3%AAter-les-conteneurs-inactifs-par-rapport-%C3%A0-les-garder-actifs-) Quand dois-je arrêter les conteneurs inactifs par rapport à les garder actifs ?


Cela dépend probablement du fournisseur, différents fournisseurs de sandbox vous permettront de définir différents critères pour les délais d’inactivité après lesquels un sandbox pourrait s’arrêter.
Vous voudrez ajuster ce délai d’expiration en fonction de la fréquence à laquelle vous pensez que la réponse de l’utilisateur pourrait se produire.


### [​](https://code.claude.com/docs/fr/agent-sdk/hosting#%C3%A0-quelle-fr%C3%A9quence-dois-je-mettre-%C3%A0-jour-le-cli-claude-code-) À quelle fréquence dois-je mettre à jour le CLI Claude Code ?


Le CLI Claude Code est versionné avec semver, donc tout changement de rupture sera versionné.


### [​](https://code.claude.com/docs/fr/agent-sdk/hosting#comment-surveiller-la-sant%C3%A9-des-conteneurs-et-les-performances-des-agents-) Comment surveiller la santé des conteneurs et les performances des agents ?


Puisque les conteneurs sont juste des serveurs, la même infrastructure de journalisation que vous utilisez pour le backend fonctionnera pour les conteneurs.


### [​](https://code.claude.com/docs/fr/agent-sdk/hosting#combien-de-temps-une-session-d%E2%80%99agent-peut-elle-s%E2%80%99ex%C3%A9cuter-avant-expiration-) Combien de temps une session d’agent peut-elle s’exécuter avant expiration ?


Une session d’agent n’expirera pas, mais envisagez de définir une propriété « maxTurns » pour empêcher Claude de se retrouver bloqué dans une boucle.


## [​](https://code.claude.com/docs/fr/agent-sdk/hosting#%C3%A9tapes-suivantes) Étapes suivantes


- [Déploiement sécurisé](https://code.claude.com/docs/fr/agent-sdk/secure-deployment) - Contrôles réseau, gestion des identifiants et renforcement de l’isolation
- [SDK TypeScript - Paramètres de sandbox](https://code.claude.com/docs/fr/agent-sdk/typescript#sandbox-settings) - Configurer le sandbox par programmation
- [Guide des sessions](https://code.claude.com/docs/fr/agent-sdk/sessions) - En savoir plus sur la gestion des sessions
- [Permissions](https://code.claude.com/docs/fr/agent-sdk/permissions) - Configurer les permissions des outils
- [Suivi des coûts](https://code.claude.com/docs/fr/agent-sdk/cost-tracking) - Surveiller l’utilisation de l’API
- [Intégration MCP](https://code.claude.com/docs/fr/agent-sdk/mcp) - Étendre avec des outils personnalisés[Claude Code Docs home page](https://code.claude.com/docs/fr/overview)

[Privacy choices](https://code.claude.com/docs/fr/agent-sdk/hosting#)

