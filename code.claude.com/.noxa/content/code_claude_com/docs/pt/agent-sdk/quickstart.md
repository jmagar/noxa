# Início Rápido
## ​Pré-requisitos
## ​Configuração
## ​Criar um arquivo com bugs
## ​Construir um agente que encontra e corrige bugs
## ​Conceitos-chave
## ​Solução de problemas
## ​Próximos passos








Comece com o Agent SDK Python ou TypeScript para construir agentes de IA que funcionam autonomamente

Use o Agent SDK para construir um agente de IA que leia seu código, encontre bugs e os corrija, tudo sem intervenção manual.
**O que você fará:**


1. Configurar um projeto com o Agent SDK
2. Criar um arquivo com código com bugs
3. Executar um agente que encontra e corrige os bugs automaticamente


## [​](https://code.claude.com/docs/pt/agent-sdk/quickstart#pr%C3%A9-requisitos) Pré-requisitos


- **Node.js 18+** ou **Python 3.10+**
- Uma **conta Anthropic** ([inscreva-se aqui](https://platform.claude.com/))


## [​](https://code.claude.com/docs/pt/agent-sdk/quickstart#configura%C3%A7%C3%A3o) Configuração


1

Criar uma pasta de projeto

Crie um novo diretório para este início rápido:

```
mkdir my-agent && cd my-agent
```

Para seus próprios projetos, você pode executar o SDK de qualquer pasta; ele terá acesso aos arquivos nesse diretório e seus subdiretórios por padrão. 2

Instalar o SDK

Instale o pacote Agent SDK para sua linguagem:

- TypeScript
- Python (uv)
- Python (pip)


```
npm install @anthropic-ai/claude-agent-sdk
```

[uv Python package manager](https://docs.astral.sh/uv/) é um gerenciador de pacotes Python rápido que lida com ambientes virtuais automaticamente:

```
uv init && uv add claude-agent-sdk
```

Crie um ambiente virtual primeiro, depois instale:

```
python3 -m venv .venv && source .venv/bin/activate
pip3 install claude-agent-sdk
```

O SDK TypeScript agrupa um binário nativo Claude Code para sua plataforma como uma dependência opcional, portanto você não precisa instalar Claude Code separadamente. 3

Defina sua chave de API

Obtenha uma chave de API no [Claude Console](https://platform.claude.com/), depois crie um arquivo `.env` no diretório do seu projeto:

```
ANTHROPIC_API_KEY=your-api-key
```

O SDK também suporta autenticação através de provedores de API de terceiros:

- **Amazon Bedrock**: defina a variável de ambiente `CLAUDE_CODE_USE_BEDROCK=1` e configure as credenciais AWS
- **Google Vertex AI**: defina a variável de ambiente `CLAUDE_CODE_USE_VERTEX=1` e configure as credenciais Google Cloud
- **Microsoft Azure**: defina a variável de ambiente `CLAUDE_CODE_USE_FOUNDRY=1` e configure as credenciais Azure

Consulte os guias de configuração para [Bedrock](https://code.claude.com/docs/pt/amazon-bedrock), [Vertex AI](https://code.claude.com/docs/pt/google-vertex-ai), ou [Azure AI Foundry](https://code.claude.com/docs/pt/microsoft-foundry) para detalhes. A menos que previamente aprovado, a Anthropic não permite que desenvolvedores terceirizados ofereçam login claude.ai ou limites de taxa para seus produtos, incluindo agentes construídos no Agent SDK Claude. Use os métodos de autenticação de chave de API descritos neste documento.


## [​](https://code.claude.com/docs/pt/agent-sdk/quickstart#criar-um-arquivo-com-bugs) Criar um arquivo com bugs


Este início rápido o orienta na construção de um agente que pode encontrar e corrigir bugs no código. Primeiro, você precisa de um arquivo com alguns bugs intencionais para o agente corrigir. Crie `utils.py` no diretório `my-agent` e cole o seguinte código:


```
def calculate_average(numbers):
    total = 0
    for num in numbers:
        total += num
    return total / len(numbers)


def get_user_name(user):
    return user["name"].upper()
```


Este código tem dois bugs:


1. `calculate_average([])` falha com divisão por zero
2. `get_user_name(None)` falha com um TypeError


## [​](https://code.claude.com/docs/pt/agent-sdk/quickstart#construir-um-agente-que-encontra-e-corrige-bugs) Construir um agente que encontra e corrige bugs


Crie `agent.py` se estiver usando o SDK Python, ou `agent.ts` para TypeScript:
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


Este código tem três partes principais:


1. **`query`**: o ponto de entrada principal que cria o loop agentic. Ele retorna um iterador assíncrono, então você usa `async for` para transmitir mensagens enquanto Claude trabalha. Veja a API completa na referência do SDK [Python](https://code.claude.com/docs/pt/agent-sdk/python#query) ou [TypeScript](https://code.claude.com/docs/pt/agent-sdk/typescript#query).
2. **`prompt`**: o que você quer que Claude faça. Claude descobre quais ferramentas usar com base na tarefa.
3. **`options`**: configuração para o agente. Este exemplo usa `allowedTools` para pré-aprovar `Read`, `Edit` e `Glob`, e `permissionMode: "acceptEdits"` para auto-aprovar alterações de arquivo. Outras opções incluem `systemPrompt`, `mcpServers` e muito mais. Veja todas as opções para [Python](https://code.claude.com/docs/pt/agent-sdk/python#claude-agent-options) ou [TypeScript](https://code.claude.com/docs/pt/agent-sdk/typescript#options).


O loop `async for` continua executando enquanto Claude pensa, chama ferramentas, observa resultados e decide o que fazer a seguir. Cada iteração produz uma mensagem: o raciocínio de Claude, uma chamada de ferramenta, um resultado de ferramenta ou o resultado final. O SDK lida com a orquestração (execução de ferramentas, gerenciamento de contexto, tentativas) para que você apenas consuma o fluxo. O loop termina quando Claude conclui a tarefa ou encontra um erro.
O tratamento de mensagens dentro do loop filtra a saída legível por humanos. Sem filtragem, você veria objetos de mensagem brutos, incluindo inicialização do sistema e estado interno, o que é útil para depuração, mas barulhento caso contrário.
Este exemplo usa streaming para mostrar o progresso em tempo real. Se você não precisar de saída ao vivo (por exemplo, para trabalhos em segundo plano ou pipelines de CI), você pode coletar todas as mensagens de uma vez. Veja [Streaming vs. modo de turno único](https://code.claude.com/docs/pt/agent-sdk/streaming-vs-single-mode) para detalhes.


### [​](https://code.claude.com/docs/pt/agent-sdk/quickstart#execute-seu-agente) Execute seu agente


Seu agente está pronto. Execute-o com o seguinte comando:


- Python
- TypeScript


```
python3 agent.py
```


```
npx tsx agent.ts
```


Após executar, verifique `utils.py`. Você verá código defensivo tratando listas vazias e usuários nulos. Seu agente autonomamente:


1. **Leu** `utils.py` para entender o código
2. **Analisou** a lógica e identificou casos extremos que causariam falhas
3. **Editou** o arquivo para adicionar tratamento de erros apropriado


Isto é o que torna o Agent SDK diferente: Claude executa ferramentas diretamente em vez de pedir que você as implemente.
Se você vir “API key not found”, certifique-se de que definiu a variável de ambiente `ANTHROPIC_API_KEY` no seu arquivo `.env` ou ambiente shell. Veja o [guia completo de solução de problemas](https://code.claude.com/docs/pt/troubleshooting) para mais ajuda.


### [​](https://code.claude.com/docs/pt/agent-sdk/quickstart#tente-outros-prompts) Tente outros prompts


Agora que seu agente está configurado, tente alguns prompts diferentes:


- `"Add docstrings to all functions in utils.py"`
- `"Add type hints to all functions in utils.py"`
- `"Create a README.md documenting the functions in utils.py"`


### [​](https://code.claude.com/docs/pt/agent-sdk/quickstart#personalize-seu-agente) Personalize seu agente


Você pode modificar o comportamento do seu agente alterando as opções. Aqui estão alguns exemplos:
**Adicionar capacidade de busca na web:**
Python TypeScript

```
options = ClaudeAgentOptions(
    allowed_tools=["Read", "Edit", "Glob", "WebSearch"], permission_mode="acceptEdits"
)
```


**Dê a Claude um prompt de sistema personalizado:**
Python TypeScript

```
options = ClaudeAgentOptions(
    allowed_tools=["Read", "Edit", "Glob"],
    permission_mode="acceptEdits",
    system_prompt="You are a senior Python developer. Always follow PEP 8 style guidelines.",
)
```


**Execute comandos no terminal:**
Python TypeScript

```
options = ClaudeAgentOptions(
    allowed_tools=["Read", "Edit", "Glob", "Bash"], permission_mode="acceptEdits"
)
```


Com `Bash` ativado, tente: `"Write unit tests for utils.py, run them, and fix any failures"`


## [​](https://code.claude.com/docs/pt/agent-sdk/quickstart#conceitos-chave) Conceitos-chave


**Ferramentas** controlam o que seu agente pode fazer:


| Ferramentas | O que o agente pode fazer |
| --- | --- |
| `Read`, `Glob`, `Grep` | Análise somente leitura |
| `Read`, `Edit`, `Glob` | Analisar e modificar código |
| `Read`, `Edit`, `Bash`, `Glob`, `Grep` | Automação completa |


**Modos de permissão** controlam quanto de supervisão humana você deseja:


| Modo | Comportamento | Caso de uso |
| --- | --- | --- |
| `acceptEdits` | Auto-aprova edições de arquivo e comandos comuns do sistema de arquivos, pede outras ações | Fluxos de trabalho de desenvolvimento confiáveis |
| `dontAsk` | Nega qualquer coisa não em `allowedTools` | Agentes headless bloqueados |
| `auto` (apenas TypeScript) | Um classificador de modelo aprova ou nega cada chamada de ferramenta | Agentes autônomos com proteções de segurança |
| `bypassPermissions` | Executa cada ferramenta sem prompts | CI em sandbox, ambientes totalmente confiáveis |
| `default` | Requer um callback `canUseTool` para lidar com aprovação | Fluxos de aprovação personalizados |


O exemplo acima usa o modo `acceptEdits`, que auto-aprova operações de arquivo para que o agente possa executar sem prompts interativos. Se você quiser solicitar aprovação dos usuários, use o modo `default` e forneça um callback [`canUseTool`](https://code.claude.com/docs/pt/agent-sdk/user-input) que coleta entrada do usuário. Para mais controle, veja [Permissões](https://code.claude.com/docs/pt/agent-sdk/permissions).


## [​](https://code.claude.com/docs/pt/agent-sdk/quickstart#solu%C3%A7%C3%A3o-de-problemas) Solução de problemas


### [​](https://code.claude.com/docs/pt/agent-sdk/quickstart#erro-de-api-thinking-type-enabled-n%C3%A3o-%C3%A9-suportado-para-este-modelo) Erro de API `thinking.type.enabled` não é suportado para este modelo


Claude Opus 4.7 substitui `thinking.type.enabled` por `thinking.type.adaptive`. Versões mais antigas do Agent SDK falham com o seguinte erro de API quando você seleciona `claude-opus-4-7`:


```
API Error: 400 {"type":"invalid_request_error","message":"\"thinking.type.enabled\" is not supported for this model. Use \"thinking.type.adaptive\" and \"output_config.effort\" to control thinking behavior."}
```


Atualize para Agent SDK v0.2.111 ou posterior para usar Opus 4.7.


## [​](https://code.claude.com/docs/pt/agent-sdk/quickstart#pr%C3%B3ximos-passos) Próximos passos


Agora que você criou seu primeiro agente, aprenda como estender suas capacidades e adaptá-lo ao seu caso de uso:


- **[Permissões](https://code.claude.com/docs/pt/agent-sdk/permissions)**: controle o que seu agente pode fazer e quando precisa de aprovação
- **[Hooks](https://code.claude.com/docs/pt/agent-sdk/hooks)**: execute código personalizado antes ou depois de chamadas de ferramenta
- **[Sessões](https://code.claude.com/docs/pt/agent-sdk/sessions)**: construa agentes multi-turno que mantêm contexto
- **[Servidores MCP](https://code.claude.com/docs/pt/agent-sdk/mcp)**: conecte-se a bancos de dados, navegadores, APIs e outros sistemas externos
- **[Hospedagem](https://code.claude.com/docs/pt/agent-sdk/hosting)**: implante agentes no Docker, nuvem e CI/CD
- **[Agentes de exemplo](https://github.com/anthropics/claude-agent-sdk-demos)**: veja exemplos completos: assistente de email, agente de pesquisa e muito mais[Claude Code Docs home page](https://code.claude.com/docs/pt/overview)

[Privacy choices](https://code.claude.com/docs/pt/agent-sdk/quickstart#)

