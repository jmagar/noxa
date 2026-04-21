# Hospedagem do Agent SDK
## ​Requisitos de Hospedagem
## ​Compreendendo a Arquitetura do SDK
## ​Opções de Provedor de Sandbox
## ​Padrões de Implantação em Produção
## ​Perguntas Frequentes
## ​Próximas Etapas







Implante e hospede o Claude Agent SDK em ambientes de produção

O Claude Agent SDK difere das APIs LLM tradicionais sem estado, pois mantém o estado conversacional e executa comandos em um ambiente persistente. Este guia aborda a arquitetura, considerações de hospedagem e melhores práticas para implantar agentes baseados em SDK em produção.
Para endurecimento de segurança além da sandboxing básica (incluindo controles de rede, gerenciamento de credenciais e opções de isolamento), consulte [Implantação Segura](https://code.claude.com/docs/pt/agent-sdk/secure-deployment).


## [​](https://code.claude.com/docs/pt/agent-sdk/hosting#requisitos-de-hospedagem) Requisitos de Hospedagem


### [​](https://code.claude.com/docs/pt/agent-sdk/hosting#sandboxing-baseado-em-container) Sandboxing Baseado em Container


Para segurança e isolamento, o SDK deve ser executado dentro de um ambiente de container sandboxed. Isso fornece isolamento de processo, limites de recursos, controle de rede e sistemas de arquivos efêmeros.
O SDK também suporta [configuração de sandbox programática](https://code.claude.com/docs/pt/agent-sdk/typescript#sandbox-settings) para execução de comandos.


### [​](https://code.claude.com/docs/pt/agent-sdk/hosting#requisitos-do-sistema) Requisitos do Sistema


Cada instância do SDK requer:


- **Dependências de tempo de execução**
  - Python 3.10+ para o SDK Python, ou Node.js 18+ para o SDK TypeScript
  - Ambos os pacotes SDK incluem um binário nativo do Claude Code para a plataforma do host, portanto, nenhuma instalação separada do Claude Code ou Node.js é necessária para o CLI gerado
- **Alocação de recursos**
  - Recomendado: 1GiB de RAM, 5GiB de disco e 1 CPU (varie isso com base em sua tarefa conforme necessário)
- **Acesso à rede**
  - HTTPS de saída para `api.anthropic.com`
  - Opcional: Acesso a servidores MCP ou ferramentas externas


## [​](https://code.claude.com/docs/pt/agent-sdk/hosting#compreendendo-a-arquitetura-do-sdk) Compreendendo a Arquitetura do SDK


Diferentemente das chamadas de API sem estado, o Claude Agent SDK opera como um **processo de longa duração** que:


- **Executa comandos** em um ambiente de shell persistente
- **Gerencia operações de arquivo** dentro de um diretório de trabalho
- **Manipula execução de ferramentas** com contexto de interações anteriores


## [​](https://code.claude.com/docs/pt/agent-sdk/hosting#op%C3%A7%C3%B5es-de-provedor-de-sandbox) Opções de Provedor de Sandbox


Vários provedores se especializam em ambientes de container seguro para execução de código de IA:


- **[Modal Sandbox](https://modal.com/docs/guide/sandbox)** - [implementação de demonstração](https://modal.com/docs/examples/claude-slack-gif-creator)
- **[Cloudflare Sandboxes](https://github.com/cloudflare/sandbox-sdk)**
- **[Daytona](https://www.daytona.io/)**
- **[E2B](https://e2b.dev/)**
- **[Fly Machines](https://fly.io/docs/machines/)**
- **[Vercel Sandbox](https://vercel.com/docs/functions/sandbox)**


Para opções auto-hospedadas (Docker, gVisor, Firecracker) e configuração de isolamento detalhada, consulte [Tecnologias de Isolamento](https://code.claude.com/docs/pt/agent-sdk/secure-deployment#isolation-technologies).


## [​](https://code.claude.com/docs/pt/agent-sdk/hosting#padr%C3%B5es-de-implanta%C3%A7%C3%A3o-em-produ%C3%A7%C3%A3o) Padrões de Implantação em Produção


### [​](https://code.claude.com/docs/pt/agent-sdk/hosting#padr%C3%A3o-1-sess%C3%B5es-ef%C3%AAmeras) Padrão 1: Sessões Efêmeras


Crie um novo container para cada tarefa do usuário e destrua-o quando concluído.
Melhor para tarefas únicas, o usuário ainda pode interagir com a IA enquanto a tarefa está sendo concluída, mas uma vez concluída, o container é destruído.
**Exemplos:**


- Investigação e Correção de Bugs: Depure e resolva um problema específico com contexto relevante
- Processamento de Faturas: Extraia e estruture dados de recibos/faturas para sistemas contábeis
- Tarefas de Tradução: Traduza documentos ou lotes de conteúdo entre idiomas
- Processamento de Imagem/Vídeo: Aplique transformações, otimizações ou extraia metadados de arquivos de mídia


### [​](https://code.claude.com/docs/pt/agent-sdk/hosting#padr%C3%A3o-2-sess%C3%B5es-de-longa-dura%C3%A7%C3%A3o) Padrão 2: Sessões de Longa Duração


Mantenha instâncias de container persistentes para tarefas de longa duração. Frequentemente, execute *múltiplos* processos do Claude Agent dentro do container com base na demanda.
Melhor para agentes proativos que tomam ações sem entrada do usuário, agentes que servem conteúdo ou agentes que processam grandes quantidades de mensagens.
**Exemplos:**


- Agente de Email: Monitora emails recebidos e triagem autônoma, responde ou toma ações com base no conteúdo
- Construtor de Sites: Hospeda sites personalizados por usuário com recursos de edição ao vivo servidos através de portas de container
- Chatbots de Alta Frequência: Manipula fluxos contínuos de mensagens de plataformas como Slack onde tempos de resposta rápidos são críticos


### [​](https://code.claude.com/docs/pt/agent-sdk/hosting#padr%C3%A3o-3-sess%C3%B5es-h%C3%ADbridas) Padrão 3: Sessões Híbridas


Containers efêmeros que são hidratados com histórico e estado, possivelmente de um banco de dados ou dos recursos de retomada de sessão do SDK.
Melhor para containers com interação intermitente do usuário que inicia trabalho e desliga quando o trabalho é concluído, mas pode ser continuado.
**Exemplos:**


- Gerenciador de Projetos Pessoais: Ajuda a gerenciar projetos em andamento com check-ins intermitentes, mantém contexto de tarefas, decisões e progresso
- Pesquisa Profunda: Conduz tarefas de pesquisa de várias horas, salva descobertas e retoma a investigação quando o usuário retorna
- Agente de Suporte ao Cliente: Manipula tickets de suporte que abrangem múltiplas interações, carrega histórico de tickets e contexto do cliente


### [​](https://code.claude.com/docs/pt/agent-sdk/hosting#padr%C3%A3o-4-containers-%C3%BAnicos) Padrão 4: Containers Únicos


Execute múltiplos processos do Claude Agent SDK em um container global.
Melhor para agentes que devem colaborar estreitamente. Este é provavelmente o padrão menos popular porque você terá que impedir que os agentes se sobrescrevam.
**Exemplos:**


- **Simulações**: Agentes que interagem entre si em simulações, como videogames.


## [​](https://code.claude.com/docs/pt/agent-sdk/hosting#perguntas-frequentes) Perguntas Frequentes


### [​](https://code.claude.com/docs/pt/agent-sdk/hosting#como-me-comunico-com-meus-sandboxes) Como me comunico com meus sandboxes?


Ao hospedar em containers, exponha portas para se comunicar com suas instâncias do SDK. Sua aplicação pode expor endpoints HTTP/WebSocket para clientes externos enquanto o SDK é executado internamente dentro do container.


### [​](https://code.claude.com/docs/pt/agent-sdk/hosting#qual-%C3%A9-o-custo-de-hospedar-um-container) Qual é o custo de hospedar um container?


O custo dominante de servir agentes são os tokens; containers variam com base no que você provisiona, mas um custo mínimo é aproximadamente 5 centavos por hora de execução.


### [​](https://code.claude.com/docs/pt/agent-sdk/hosting#quando-devo-desligar-containers-ociosos-versus-mant%C3%AA-los-aquecidos) Quando devo desligar containers ociosos versus mantê-los aquecidos?


Isso provavelmente depende do provedor, diferentes provedores de sandbox permitirão que você defina critérios diferentes para tempos limite de ociosidade após os quais um sandbox pode desligar.
Você desejará ajustar esse tempo limite com base na frequência com que acha que a resposta do usuário pode ocorrer.


### [​](https://code.claude.com/docs/pt/agent-sdk/hosting#com-que-frequ%C3%AAncia-devo-atualizar-o-claude-code-cli) Com que frequência devo atualizar o Claude Code CLI?


O Claude Code CLI é versionado com semver, portanto, quaisquer alterações significativas serão versionadas.


### [​](https://code.claude.com/docs/pt/agent-sdk/hosting#como-monitoro-a-sa%C3%BAde-do-container-e-o-desempenho-do-agente) Como monitoro a saúde do container e o desempenho do agente?


Como containers são apenas servidores, a mesma infraestrutura de logging que você usa para o backend funcionará para containers.


### [​](https://code.claude.com/docs/pt/agent-sdk/hosting#quanto-tempo-uma-sess%C3%A3o-de-agente-pode-ser-executada-antes-de-atingir-o-tempo-limite) Quanto tempo uma sessão de agente pode ser executada antes de atingir o tempo limite?


Uma sessão de agente não atingirá o tempo limite, mas considere definir uma propriedade ‘maxTurns’ para impedir que Claude fique preso em um loop.


## [​](https://code.claude.com/docs/pt/agent-sdk/hosting#pr%C3%B3ximas-etapas) Próximas Etapas


- [Implantação Segura](https://code.claude.com/docs/pt/agent-sdk/secure-deployment) - Controles de rede, gerenciamento de credenciais e endurecimento de isolamento
- [SDK TypeScript - Configurações de Sandbox](https://code.claude.com/docs/pt/agent-sdk/typescript#sandbox-settings) - Configure sandbox programaticamente
- [Guia de Sessões](https://code.claude.com/docs/pt/agent-sdk/sessions) - Saiba mais sobre gerenciamento de sessões
- [Permissões](https://code.claude.com/docs/pt/agent-sdk/permissions) - Configure permissões de ferramentas
- [Rastreamento de Custos](https://code.claude.com/docs/pt/agent-sdk/cost-tracking) - Monitore o uso da API
- [Integração MCP](https://code.claude.com/docs/pt/agent-sdk/mcp) - Estenda com ferramentas personalizadas[Claude Code Docs home page](https://code.claude.com/docs/pt/overview)

[Privacy choices](https://code.claude.com/docs/pt/agent-sdk/hosting#)

