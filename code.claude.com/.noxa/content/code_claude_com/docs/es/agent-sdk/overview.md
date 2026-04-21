# Descripción general del Agent SDK
## ​Comenzar
## ​Capacidades
## ​Compare el Agent SDK con otras herramientas de Claude
## ​Registro de cambios
## ​Reportar errores
## ​Directrices de marca
## ​Licencia y términos
## ​Próximos pasos









Construya agentes de IA en producción con Claude Code como una biblioteca

El Claude Code SDK ha sido renombrado a Claude Agent SDK. Si está migrando desde el SDK anterior, consulte la [Guía de migración](https://code.claude.com/docs/es/agent-sdk/migration-guide).
Construya agentes de IA que lean archivos de forma autónoma, ejecuten comandos, busquen en la web, editen código y mucho más. El Agent SDK le proporciona las mismas herramientas, bucle de agente y gestión de contexto que potencian Claude Code, programable en Python y TypeScript.
Opus 4.7 ( `claude-opus-4-7`) requiere Agent SDK v0.2.111 o posterior. Si ve un error de API `thinking.type.enabled`, consulte [Solución de problemas](https://code.claude.com/docs/es/agent-sdk/quickstart#troubleshooting).
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


El Agent SDK incluye herramientas integradas para leer archivos, ejecutar comandos y editar código, por lo que su agente puede comenzar a trabajar inmediatamente sin que usted implemente la ejecución de herramientas. Sumérjase en el inicio rápido o explore agentes reales construidos con el SDK:


## Inicio rápido

Construya un agente corrector de errores en minutos

## Agentes de ejemplo

Asistente de correo electrónico, agente de investigación y más


## [​](https://code.claude.com/docs/es/agent-sdk/overview#comenzar) Comenzar


1

Instale el SDK


- TypeScript
- Python


```
npm install @anthropic-ai/claude-agent-sdk
```


```
pip install claude-agent-sdk
```

El SDK de TypeScript agrupa un binario nativo de Claude Code para su plataforma como una dependencia opcional, por lo que no necesita instalar Claude Code por separado. 2

Configure su clave de API

Obtenga una clave de API de la [Consola](https://platform.claude.com/), luego configúrela como una variable de entorno:

```
export ANTHROPIC_API_KEY=your-api-key
```

El SDK también admite autenticación a través de proveedores de API de terceros:

- **Amazon Bedrock**: configure la variable de entorno `CLAUDE_CODE_USE_BEDROCK=1` y configure las credenciales de AWS
- **Google Vertex AI**: configure la variable de entorno `CLAUDE_CODE_USE_VERTEX=1` y configure las credenciales de Google Cloud
- **Microsoft Azure**: configure la variable de entorno `CLAUDE_CODE_USE_FOUNDRY=1` y configure las credenciales de Azure

Consulte las guías de configuración para [Bedrock](https://code.claude.com/docs/es/amazon-bedrock), [Vertex AI](https://code.claude.com/docs/es/google-vertex-ai) o [Azure AI Foundry](https://code.claude.com/docs/es/microsoft-foundry) para obtener más detalles. A menos que haya sido aprobado previamente, Anthropic no permite que desarrolladores de terceros ofrezcan inicio de sesión en claude.ai o límites de velocidad para sus productos, incluidos los agentes construidos en el Claude Agent SDK. Por favor, utilice los métodos de autenticación de clave de API descritos en este documento en su lugar. 3

Ejecute su primer agente

Este ejemplo crea un agente que enumera archivos en su directorio actual utilizando herramientas integradas. Python TypeScript

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


**¿Listo para construir?** Siga el [Inicio rápido](https://code.claude.com/docs/es/agent-sdk/quickstart) para crear un agente que encuentre y corrija errores en minutos.


## [​](https://code.claude.com/docs/es/agent-sdk/overview#capacidades) Capacidades


Todo lo que hace que Claude Code sea poderoso está disponible en el SDK:


- Herramientas integradas
- Hooks
- Subagentes
- MCP
- Permisos
- Sesiones

Su agente puede leer archivos, ejecutar comandos y buscar en bases de código de forma inmediata. Las herramientas clave incluyen:

| Herramienta | Qué hace |
| --- | --- |
| **Read** | Leer cualquier archivo en el directorio de trabajo |
| **Write** | Crear nuevos archivos |
| **Edit** | Realizar ediciones precisas en archivos existentes |
| **Bash** | Ejecutar comandos de terminal, scripts, operaciones de git |
| **Monitor** | Observar un script de fondo y reaccionar a cada línea de salida como un evento |
| **Glob** | Encontrar archivos por patrón ( `**/*.ts`, `src/**/*.py`) |
| **Grep** | Buscar contenido de archivos con expresiones regulares |
| **WebSearch** | Buscar en la web información actual |
| **WebFetch** | Obtener y analizar contenido de páginas web |
| **[AskUserQuestion](https://code.claude.com/docs/es/agent-sdk/user-input#handle-clarifying-questions)** | Hacer preguntas aclaratorias al usuario con opciones de opción múltiple |

Este ejemplo crea un agente que busca comentarios TODO en su base de código: Python TypeScript

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

Ejecute código personalizado en puntos clave del ciclo de vida del agente. Los hooks del SDK utilizan funciones de devolución de llamada para validar, registrar, bloquear o transformar el comportamiento del agente. **Hooks disponibles:** `PreToolUse`, `PostToolUse`, `Stop`, `SessionStart`, `SessionEnd`, `UserPromptSubmit` y más. Este ejemplo registra todos los cambios de archivo en un archivo de auditoría: Python TypeScript

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

[Obtenga más información sobre hooks →](https://code.claude.com/docs/es/agent-sdk/hooks) Genere agentes especializados para manejar subtareas enfocadas. Su agente principal delega trabajo y los subagentes informan con resultados. Defina agentes personalizados con instrucciones especializadas. Incluya `Agent` en `allowedTools` ya que los subagentes se invocan a través de la herramienta Agent: Python TypeScript

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

Los mensajes dentro del contexto de un subagente incluyen un campo `parent_tool_use_id`, lo que le permite rastrear qué mensajes pertenecen a qué ejecución de subagente. [Obtenga más información sobre subagentes →](https://code.claude.com/docs/es/agent-sdk/subagents) Conéctese a sistemas externos a través del Protocolo de Contexto del Modelo: bases de datos, navegadores, API y [cientos más](https://github.com/modelcontextprotocol/servers). Este ejemplo conecta el [servidor Playwright MCP](https://github.com/microsoft/playwright-mcp) para dar a su agente capacidades de automatización del navegador: Python TypeScript

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

[Obtenga más información sobre MCP →](https://code.claude.com/docs/es/agent-sdk/mcp) Controle exactamente qué herramientas puede usar su agente. Permita operaciones seguras, bloquee las peligrosas o requiera aprobación para acciones sensibles. Para solicitudes de aprobación interactivas y la herramienta `AskUserQuestion`, consulte [Manejar aprobaciones e entrada del usuario](https://code.claude.com/docs/es/agent-sdk/user-input). Este ejemplo crea un agente de solo lectura que puede analizar pero no modificar código. `allowed_tools` aprueba previamente `Read`, `Glob` y `Grep`. Python TypeScript

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

[Obtenga más información sobre permisos →](https://code.claude.com/docs/es/agent-sdk/permissions) Mantenga el contexto en múltiples intercambios. Claude recuerda archivos leídos, análisis realizados e historial de conversación. Reanude sesiones más tarde o divídalas para explorar diferentes enfoques. Este ejemplo captura el ID de sesión de la primera consulta, luego reanuda para continuar con contexto completo: Python TypeScript

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

[Obtenga más información sobre sesiones →](https://code.claude.com/docs/es/agent-sdk/sessions)


### [​](https://code.claude.com/docs/es/agent-sdk/overview#caracter%C3%ADsticas-de-claude-code) Características de Claude Code


El SDK también admite la configuración basada en el sistema de archivos de Claude Code. Con opciones predeterminadas, el SDK carga estas desde `.claude/` en su directorio de trabajo y `~/.claude/`. Para restringir qué fuentes se cargan, configure `setting_sources` (Python) o `settingSources` (TypeScript) en sus opciones.


| Característica | Descripción | Ubicación |
| --- | --- | --- |
| [Skills](https://code.claude.com/docs/es/agent-sdk/skills) | Capacidades especializadas definidas en Markdown | `.claude/skills/*/SKILL.md` |
| [Slash commands](https://code.claude.com/docs/es/agent-sdk/slash-commands) | Comandos personalizados para tareas comunes | `.claude/commands/*.md` |
| [Memory](https://code.claude.com/docs/es/agent-sdk/modifying-system-prompts) | Contexto e instrucciones del proyecto | `CLAUDE.md` o `.claude/CLAUDE.md` |
| [Plugins](https://code.claude.com/docs/es/agent-sdk/plugins) | Extienda con comandos personalizados, agentes y servidores MCP | Programático a través de la opción `plugins` |


## [​](https://code.claude.com/docs/es/agent-sdk/overview#compare-el-agent-sdk-con-otras-herramientas-de-claude) Compare el Agent SDK con otras herramientas de Claude


La Plataforma Claude ofrece múltiples formas de construir con Claude. Así es como se ajusta el Agent SDK:


- Agent SDK vs Client SDK
- Agent SDK vs Claude Code CLI

El [Anthropic Client SDK](https://platform.claude.com/docs/en/api/client-sdks) le proporciona acceso directo a la API: usted envía solicitudes y implementa la ejecución de herramientas usted mismo. El **Agent SDK** le proporciona Claude con ejecución de herramientas integrada. Con el Client SDK, implementa un bucle de herramientas. Con el Agent SDK, Claude lo maneja: Python TypeScript

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

Mismas capacidades, interfaz diferente:

| Caso de uso | Mejor opción |
| --- | --- |
| Desarrollo interactivo | CLI |
| Canalizaciones CI/CD | SDK |
| Aplicaciones personalizadas | SDK |
| Tareas puntuales | CLI |
| Automatización en producción | SDK |

Muchos equipos usan ambos: CLI para desarrollo diario, SDK para producción. Los flujos de trabajo se traducen directamente entre ellos.


## [​](https://code.claude.com/docs/es/agent-sdk/overview#registro-de-cambios) Registro de cambios


Vea el registro de cambios completo para actualizaciones del SDK, correcciones de errores y nuevas características:


- **TypeScript SDK**: [ver CHANGELOG.md](https://github.com/anthropics/claude-agent-sdk-typescript/blob/main/CHANGELOG.md)
- **Python SDK**: [ver CHANGELOG.md](https://github.com/anthropics/claude-agent-sdk-python/blob/main/CHANGELOG.md)


## [​](https://code.claude.com/docs/es/agent-sdk/overview#reportar-errores) Reportar errores


Si encuentra errores o problemas con el Agent SDK:


- **TypeScript SDK**: [reportar problemas en GitHub](https://github.com/anthropics/claude-agent-sdk-typescript/issues)
- **Python SDK**: [reportar problemas en GitHub](https://github.com/anthropics/claude-agent-sdk-python/issues)


## [​](https://code.claude.com/docs/es/agent-sdk/overview#directrices-de-marca) Directrices de marca


Para socios que integran el Claude Agent SDK, el uso de la marca Claude es opcional. Al hacer referencia a Claude en su producto:
**Permitido:**


- “Claude Agent” (preferido para menús desplegables)
- “Claude” (cuando ya está dentro de un menú etiquetado como “Agents”)
- ” Powered by Claude” (si tiene un nombre de agente existente)


**No permitido:**


- “Claude Code” o “Claude Code Agent”
- Arte ASCII de marca Claude Code o elementos visuales que imiten Claude Code


Su producto debe mantener su propia marca y no parecer ser Claude Code o ningún producto de Anthropic. Para preguntas sobre cumplimiento de marca, póngase en contacto con el [equipo de ventas](https://www.anthropic.com/contact-sales) de Anthropic.


## [​](https://code.claude.com/docs/es/agent-sdk/overview#licencia-y-t%C3%A9rminos) Licencia y términos


El uso del Claude Agent SDK se rige por los [Términos de Servicio Comerciales de Anthropic](https://www.anthropic.com/legal/commercial-terms), incluso cuando lo utiliza para potenciar productos y servicios que pone a disposición de sus propios clientes y usuarios finales, excepto en la medida en que un componente específico o dependencia esté cubierto por una licencia diferente como se indica en el archivo LICENSE de ese componente.


## [​](https://code.claude.com/docs/es/agent-sdk/overview#pr%C3%B3ximos-pasos) Próximos pasos


## Inicio rápido

Construya un agente que encuentre y corrija errores en minutos

## Agentes de ejemplo

Asistente de correo electrónico, agente de investigación y más

## TypeScript SDK

Referencia completa de API de TypeScript y ejemplos

## Python SDK

Referencia completa de API de Python y ejemplos[Claude Code Docs home page](https://code.claude.com/docs/es/overview)

[Privacy choices](https://code.claude.com/docs/es/agent-sdk/overview#)

