# Visão geral do Agent SDK
## ​Comece agora
## ​Capacidades
## ​Compare o Agent SDK com outras ferramentas Claude
## ​Changelog
## ​Relatando bugs
## ​Diretrizes de marca
## ​Licença e termos
## ​Próximos passos









Construa agentes de IA em produção com Claude Code como uma biblioteca

O Claude Code SDK foi renomeado para Claude Agent SDK. Se você está migrando do SDK antigo, consulte o [Guia de Migração](https://code.claude.com/docs/pt/agent-sdk/migration-guide).
Construa agentes de IA que leem arquivos autonomamente, executam comandos, pesquisam na web, editam código e muito mais. O Agent SDK oferece as mesmas ferramentas, loop de agente e gerenciamento de contexto que alimentam Claude Code, programável em Python e TypeScript.
Opus 4.7 ( `claude-opus-4-7`) requer Agent SDK v0.2.111 ou posterior. Se você vir um erro de API `thinking.type.enabled`, consulte [Troubleshooting](https://code.claude.com/docs/pt/agent-sdk/quickstart#troubleshooting).
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


O Agent SDK inclui ferramentas integradas para ler arquivos, executar comandos e editar código, para que seu agente possa começar a trabalhar imediatamente sem você implementar a execução de ferramentas. Mergulhe no guia de início rápido ou explore agentes reais construídos com o SDK:


## Guia de Início Rápido

Construa um agente de correção de bugs em minutos

## Agentes de exemplo

Assistente de email, agente de pesquisa e muito mais


## [​](https://code.claude.com/docs/pt/agent-sdk/overview#comece-agora) Comece agora


1

Instale o SDK


- TypeScript
- Python


```
npm install @anthropic-ai/claude-agent-sdk
```


```
pip install claude-agent-sdk
```

O SDK TypeScript agrupa um binário nativo do Claude Code para sua plataforma como uma dependência opcional, portanto você não precisa instalar Claude Code separadamente. 2

Defina sua chave de API

Obtenha uma chave de API do [Console](https://platform.claude.com/), depois defina-a como uma variável de ambiente:

```
export ANTHROPIC_API_KEY=your-api-key
```

O SDK também suporta autenticação via provedores de API de terceiros:

- **Amazon Bedrock**: defina a variável de ambiente `CLAUDE_CODE_USE_BEDROCK=1` e configure as credenciais da AWS
- **Google Vertex AI**: defina a variável de ambiente `CLAUDE_CODE_USE_VERTEX=1` e configure as credenciais do Google Cloud
- **Microsoft Azure**: defina a variável de ambiente `CLAUDE_CODE_USE_FOUNDRY=1` e configure as credenciais do Azure

Consulte os guias de configuração para [Bedrock](https://code.claude.com/docs/pt/amazon-bedrock), [Vertex AI](https://code.claude.com/docs/pt/google-vertex-ai) ou [Azure AI Foundry](https://code.claude.com/docs/pt/microsoft-foundry) para obter detalhes. A menos que previamente aprovado, a Anthropic não permite que desenvolvedores terceirizados ofereçam login claude.ai ou limites de taxa para seus produtos, incluindo agentes construídos no Claude Agent SDK. Use os métodos de autenticação de chave de API descritos neste documento. 3

Execute seu primeiro agente

Este exemplo cria um agente que lista arquivos em seu diretório atual usando ferramentas integradas. Python TypeScript

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


**Pronto para construir?** Siga o [Guia de Início Rápido](https://code.claude.com/docs/pt/agent-sdk/quickstart) para criar um agente que encontra e corrige bugs em minutos.


## [​](https://code.claude.com/docs/pt/agent-sdk/overview#capacidades) Capacidades


Tudo o que torna Claude Code poderoso está disponível no SDK:


- Ferramentas integradas
- hooks
- Subagentes
- MCP
- Permissões
- Sessões

Seu agente pode ler arquivos, executar comandos e pesquisar bases de código imediatamente. As ferramentas principais incluem:

| Ferramenta | O que faz |
| --- | --- |
| **Read** | Ler qualquer arquivo no diretório de trabalho |
| **Write** | Criar novos arquivos |
| **Edit** | Fazer edições precisas em arquivos existentes |
| **Bash** | Executar comandos de terminal, scripts, operações git |
| **Monitor** | Observar um script em segundo plano e reagir a cada linha de saída como um evento |
| **Glob** | Encontrar arquivos por padrão ( `**/*.ts`, `src/**/*.py`) |
| **Grep** | Pesquisar conteúdo de arquivos com regex |
| **WebSearch** | Pesquisar na web por informações atuais |
| **WebFetch** | Buscar e analisar conteúdo de páginas da web |
| **[AskUserQuestion](https://code.claude.com/docs/pt/agent-sdk/user-input#handle-clarifying-questions)** | Fazer perguntas de esclarecimento ao usuário com opções de múltipla escolha |

Este exemplo cria um agente que pesquisa sua base de código por comentários TODO: Python TypeScript

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

Execute código personalizado em pontos-chave do ciclo de vida do agente. Os hooks do SDK usam funções de retorno de chamada para validar, registrar, bloquear ou transformar o comportamento do agente. **Hooks disponíveis:** `PreToolUse`, `PostToolUse`, `Stop`, `SessionStart`, `SessionEnd`, `UserPromptSubmit` e muito mais. Este exemplo registra todas as alterações de arquivo em um arquivo de auditoria: Python TypeScript

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

[Saiba mais sobre hooks →](https://code.claude.com/docs/pt/agent-sdk/hooks) Crie agentes especializados para lidar com subtarefas focadas. Seu agente principal delega trabalho e os subagentes relatam resultados. Defina agentes personalizados com instruções especializadas. Inclua `Agent` em `allowedTools` já que os subagentes são invocados via a ferramenta Agent: Python TypeScript

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

As mensagens dentro do contexto de um subagente incluem um campo `parent_tool_use_id`, permitindo que você rastreie quais mensagens pertencem a qual execução de subagente. [Saiba mais sobre subagentes →](https://code.claude.com/docs/pt/agent-sdk/subagents) Conecte-se a sistemas externos via Model Context Protocol: bancos de dados, navegadores, APIs e [centenas mais](https://github.com/modelcontextprotocol/servers). Este exemplo conecta o [servidor Playwright MCP](https://github.com/microsoft/playwright-mcp) para dar ao seu agente capacidades de automação de navegador: Python TypeScript

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

[Saiba mais sobre MCP →](https://code.claude.com/docs/pt/agent-sdk/mcp) Controle exatamente quais ferramentas seu agente pode usar. Permita operações seguras, bloqueie operações perigosas ou exija aprovação para ações sensíveis. Para prompts de aprovação interativa e a ferramenta `AskUserQuestion`, consulte [Lidar com aprovações e entrada do usuário](https://code.claude.com/docs/pt/agent-sdk/user-input). Este exemplo cria um agente somente leitura que pode analisar mas não modificar código. `allowed_tools` pré-aprova `Read`, `Glob` e `Grep`. Python TypeScript

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

[Saiba mais sobre permissões →](https://code.claude.com/docs/pt/agent-sdk/permissions) Mantenha contexto em múltiplas trocas. Claude se lembra de arquivos lidos, análises feitas e histórico de conversa. Retome sessões depois ou divida-as para explorar diferentes abordagens. Este exemplo captura o ID da sessão da primeira consulta, depois retoma para continuar com contexto completo: Python TypeScript

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

[Saiba mais sobre sessões →](https://code.claude.com/docs/pt/agent-sdk/sessions)


### [​](https://code.claude.com/docs/pt/agent-sdk/overview#recursos-do-claude-code) Recursos do Claude Code


O SDK também suporta a configuração baseada em sistema de arquivos do Claude Code. Com opções padrão, o SDK carrega estas do `.claude/` em seu diretório de trabalho e `~/.claude/`. Para restringir quais fontes carregam, defina `setting_sources` (Python) ou `settingSources` (TypeScript) em suas opções.


| Recurso | Descrição | Localização |
| --- | --- | --- |
| [Skills](https://code.claude.com/docs/pt/agent-sdk/skills) | Capacidades especializadas definidas em Markdown | `.claude/skills/*/SKILL.md` |
| [Slash commands](https://code.claude.com/docs/pt/agent-sdk/slash-commands) | Comandos personalizados para tarefas comuns | `.claude/commands/*.md` |
| [Memory](https://code.claude.com/docs/pt/agent-sdk/modifying-system-prompts) | Contexto do projeto e instruções | `CLAUDE.md` ou `.claude/CLAUDE.md` |
| [Plugins](https://code.claude.com/docs/pt/agent-sdk/plugins) | Estenda com comandos personalizados, agentes e servidores MCP | Programático via opção `plugins` |


## [​](https://code.claude.com/docs/pt/agent-sdk/overview#compare-o-agent-sdk-com-outras-ferramentas-claude) Compare o Agent SDK com outras ferramentas Claude


A Plataforma Claude oferece múltiplas maneiras de construir com Claude. Aqui está como o Agent SDK se encaixa:


- Agent SDK vs Client SDK
- Agent SDK vs Claude Code CLI

O [Anthropic Client SDK](https://platform.claude.com/docs/en/api/client-sdks) oferece acesso direto à API: você envia prompts e implementa a execução de ferramentas você mesmo. O **Agent SDK** oferece Claude com execução de ferramentas integrada. Com o Client SDK, você implementa um loop de ferramentas. Com o Agent SDK, Claude o manipula: Python TypeScript

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

Mesmas capacidades, interface diferente:

| Caso de uso | Melhor escolha |
| --- | --- |
| Desenvolvimento interativo | CLI |
| Pipelines CI/CD | SDK |
| Aplicações personalizadas | SDK |
| Tarefas únicas | CLI |
| Automação em produção | SDK |

Muitas equipes usam ambas: CLI para desenvolvimento diário, SDK para produção. Os fluxos de trabalho se traduzem diretamente entre eles.


## [​](https://code.claude.com/docs/pt/agent-sdk/overview#changelog) Changelog


Veja o changelog completo para atualizações do SDK, correções de bugs e novos recursos:


- **TypeScript SDK**: [ver CHANGELOG.md](https://github.com/anthropics/claude-agent-sdk-typescript/blob/main/CHANGELOG.md)
- **Python SDK**: [ver CHANGELOG.md](https://github.com/anthropics/claude-agent-sdk-python/blob/main/CHANGELOG.md)


## [​](https://code.claude.com/docs/pt/agent-sdk/overview#relatando-bugs) Relatando bugs


Se você encontrar bugs ou problemas com o Agent SDK:


- **TypeScript SDK**: [relatar problemas no GitHub](https://github.com/anthropics/claude-agent-sdk-typescript/issues)
- **Python SDK**: [relatar problemas no GitHub](https://github.com/anthropics/claude-agent-sdk-python/issues)


## [​](https://code.claude.com/docs/pt/agent-sdk/overview#diretrizes-de-marca) Diretrizes de marca


Para parceiros integrando o Claude Agent SDK, o uso de marca Claude é opcional. Ao fazer referência a Claude em seu produto:
**Permitido:**


- “Claude Agent” (preferido para menus suspensos)
- “Claude” (quando dentro de um menu já rotulado “Agents”)
- ” Powered by Claude” (se você tiver um nome de agente existente)


**Não permitido:**


- “Claude Code” ou “Claude Code Agent”
- Arte ASCII com marca Claude Code ou elementos visuais que imitam Claude Code


Seu produto deve manter sua própria marca e não parecer ser Claude Code ou qualquer produto Anthropic. Para perguntas sobre conformidade de marca, entre em contato com a [equipe de vendas](https://www.anthropic.com/contact-sales) da Anthropic.


## [​](https://code.claude.com/docs/pt/agent-sdk/overview#licen%C3%A7a-e-termos) Licença e termos


O uso do Claude Agent SDK é regido pelos [Termos de Serviço Comercial da Anthropic](https://www.anthropic.com/legal/commercial-terms), incluindo quando você o usa para alimentar produtos e serviços que você disponibiliza para seus próprios clientes e usuários finais, exceto na medida em que um componente específico ou dependência seja coberto por uma licença diferente conforme indicado no arquivo LICENSE desse componente.


## [​](https://code.claude.com/docs/pt/agent-sdk/overview#pr%C3%B3ximos-passos) Próximos passos


## Guia de Início Rápido

Construa um agente que encontra e corrige bugs em minutos

## Agentes de exemplo

Assistente de email, agente de pesquisa e muito mais

## TypeScript SDK

Referência completa da API TypeScript e exemplos

## Python SDK

Referência completa da API Python e exemplos[Claude Code Docs home page](https://code.claude.com/docs/pt/overview)

[Privacy choices](https://code.claude.com/docs/pt/agent-sdk/overview#)

