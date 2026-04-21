# Inicio rápido
## ​Requisitos previos
## ​Configuración
## ​Crear un archivo con errores
## ​Construir un agente que encuentre y corrija errores
## ​Conceptos clave
## ​Solución de problemas
## ​Próximos pasos








Comience con el SDK de Agent de Python o TypeScript para crear agentes de IA que funcionen de forma autónoma

Utilice el SDK de Agent para crear un agente de IA que lea su código, encuentre errores y los corrija, todo sin intervención manual.
**Lo que hará:**


1. Configurar un proyecto con el SDK de Agent
2. Crear un archivo con código con errores
3. Ejecutar un agente que encuentre y corrija los errores automáticamente


## [​](https://code.claude.com/docs/es/agent-sdk/quickstart#requisitos-previos) Requisitos previos


- **Node.js 18+** o **Python 3.10+**
- Una **cuenta de Anthropic** ([regístrese aquí](https://platform.claude.com/))


## [​](https://code.claude.com/docs/es/agent-sdk/quickstart#configuraci%C3%B3n) Configuración


1

Crear una carpeta de proyecto

Cree un nuevo directorio para este inicio rápido:

```
mkdir my-agent && cd my-agent
```

Para sus propios proyectos, puede ejecutar el SDK desde cualquier carpeta; tendrá acceso a los archivos en ese directorio y sus subdirectorios de forma predeterminada. 2

Instalar el SDK

Instale el paquete del SDK de Agent para su idioma:

- TypeScript
- Python (uv)
- Python (pip)


```
npm install @anthropic-ai/claude-agent-sdk
```

[uv Python package manager](https://docs.astral.sh/uv/) es un gestor de paquetes de Python rápido que maneja automáticamente los entornos virtuales:

```
uv init && uv add claude-agent-sdk
```

Primero cree un entorno virtual, luego instale:

```
python3 -m venv .venv && source .venv/bin/activate
pip3 install claude-agent-sdk
```

El SDK de TypeScript incluye un binario nativo de Claude Code para su plataforma como una dependencia opcional, por lo que no necesita instalar Claude Code por separado. 3

Establecer su clave de API

Obtenga una clave de API de la [Consola de Claude](https://platform.claude.com/), luego cree un archivo `.env` en su directorio de proyecto:

```
ANTHROPIC_API_KEY=your-api-key
```

El SDK también admite autenticación a través de proveedores de API de terceros:

- **Amazon Bedrock**: establezca la variable de entorno `CLAUDE_CODE_USE_BEDROCK=1` y configure las credenciales de AWS
- **Google Vertex AI**: establezca la variable de entorno `CLAUDE_CODE_USE_VERTEX=1` y configure las credenciales de Google Cloud
- **Microsoft Azure**: establezca la variable de entorno `CLAUDE_CODE_USE_FOUNDRY=1` y configure las credenciales de Azure

Consulte las guías de configuración para [Bedrock](https://code.claude.com/docs/es/amazon-bedrock), [Vertex AI](https://code.claude.com/docs/es/google-vertex-ai), o [Azure AI Foundry](https://code.claude.com/docs/es/microsoft-foundry) para obtener más detalles. A menos que haya sido aprobado previamente, Anthropic no permite que desarrolladores de terceros ofrezcan inicio de sesión en claude.ai o límites de velocidad para sus productos, incluidos los agentes construidos en el SDK de Agent de Claude. Por favor, utilice los métodos de autenticación de clave de API descritos en este documento en su lugar.


## [​](https://code.claude.com/docs/es/agent-sdk/quickstart#crear-un-archivo-con-errores) Crear un archivo con errores


Este inicio rápido lo guía a través de la construcción de un agente que puede encontrar y corregir errores en el código. Primero, necesita un archivo con algunos errores intencionales para que el agente corrija. Cree `utils.py` en el directorio `my-agent` y pegue el siguiente código:


```
def calculate_average(numbers):
    total = 0
    for num in numbers:
        total += num
    return total / len(numbers)


def get_user_name(user):
    return user["name"].upper()
```


Este código tiene dos errores:


1. `calculate_average([])` se bloquea con una división por cero
2. `get_user_name(None)` se bloquea con un TypeError


## [​](https://code.claude.com/docs/es/agent-sdk/quickstart#construir-un-agente-que-encuentre-y-corrija-errores) Construir un agente que encuentre y corrija errores


Cree `agent.py` si está utilizando el SDK de Python, o `agent.ts` para TypeScript:
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


Este código tiene tres partes principales:


1. **`query`**: el punto de entrada principal que crea el bucle agentic. Devuelve un iterador asincrónico, por lo que utiliza `async for` para transmitir mensajes mientras Claude trabaja. Consulte la API completa en la referencia del SDK de [Python](https://code.claude.com/docs/es/agent-sdk/python#query) o [TypeScript](https://code.claude.com/docs/es/agent-sdk/typescript#query).
2. **`prompt`**: lo que desea que haga Claude. Claude determina qué herramientas usar en función de la tarea.
3. **`options`**: configuración para el agente. Este ejemplo utiliza `allowedTools` para preautorizar `Read`, `Edit` y `Glob`, y `permissionMode: "acceptEdits"` para aprobar automáticamente los cambios de archivo. Otras opciones incluyen `systemPrompt`, `mcpServers` y más. Consulte todas las opciones para [Python](https://code.claude.com/docs/es/agent-sdk/python#claude-agent-options) o [TypeScript](https://code.claude.com/docs/es/agent-sdk/typescript#options).


El bucle `async for` continúa ejecutándose mientras Claude piensa, llama a herramientas, observa resultados y decide qué hacer a continuación. Cada iteración produce un mensaje: el razonamiento de Claude, una llamada a herramienta, un resultado de herramienta o el resultado final. El SDK maneja la orquestación (ejecución de herramientas, gestión de contexto, reintentos) para que solo consuma el flujo. El bucle termina cuando Claude completa la tarea o encuentra un error.
El manejo de mensajes dentro del bucle filtra la salida legible por humanos. Sin filtrado, vería objetos de mensaje sin procesar, incluida la inicialización del sistema y el estado interno, lo que es útil para depuración pero ruidoso de otra manera.
Este ejemplo utiliza transmisión para mostrar el progreso en tiempo real. Si no necesita salida en vivo (por ejemplo, para trabajos en segundo plano o canalizaciones de CI), puede recopilar todos los mensajes a la vez. Consulte [Transmisión frente a modo de un solo turno](https://code.claude.com/docs/es/agent-sdk/streaming-vs-single-mode) para obtener más detalles.


### [​](https://code.claude.com/docs/es/agent-sdk/quickstart#ejecutar-su-agente) Ejecutar su agente


Su agente está listo. Ejecútelo con el siguiente comando:


- Python
- TypeScript


```
python3 agent.py
```


```
npx tsx agent.ts
```


Después de ejecutar, verifique `utils.py`. Verá código defensivo que maneja listas vacías y usuarios nulos. Su agente de forma autónoma:


1. **Leyó** `utils.py` para entender el código
2. **Analizó** la lógica e identificó casos extremos que causarían bloqueos
3. **Editó** el archivo para agregar manejo de errores adecuado


Esto es lo que hace diferente al SDK de Agent: Claude ejecuta herramientas directamente en lugar de pedirle que las implemente.
Si ve “API key not found”, asegúrese de haber establecido la variable de entorno `ANTHROPIC_API_KEY` en su archivo `.env` o entorno de shell. Consulte la [guía completa de solución de problemas](https://code.claude.com/docs/es/troubleshooting) para obtener más ayuda.


### [​](https://code.claude.com/docs/es/agent-sdk/quickstart#probar-otros-prompts) Probar otros prompts


Ahora que su agente está configurado, pruebe algunos prompts diferentes:


- `"Add docstrings to all functions in utils.py"`
- `"Add type hints to all functions in utils.py"`
- `"Create a README.md documenting the functions in utils.py"`


### [​](https://code.claude.com/docs/es/agent-sdk/quickstart#personalizar-su-agente) Personalizar su agente


Puede modificar el comportamiento de su agente cambiando las opciones. Aquí hay algunos ejemplos:
**Agregar capacidad de búsqueda web:**
Python TypeScript

```
options = ClaudeAgentOptions(
    allowed_tools=["Read", "Edit", "Glob", "WebSearch"], permission_mode="acceptEdits"
)
```


**Dar a Claude un prompt de sistema personalizado:**
Python TypeScript

```
options = ClaudeAgentOptions(
    allowed_tools=["Read", "Edit", "Glob"],
    permission_mode="acceptEdits",
    system_prompt="You are a senior Python developer. Always follow PEP 8 style guidelines.",
)
```


**Ejecutar comandos en la terminal:**
Python TypeScript

```
options = ClaudeAgentOptions(
    allowed_tools=["Read", "Edit", "Glob", "Bash"], permission_mode="acceptEdits"
)
```


Con `Bash` habilitado, intente: `"Write unit tests for utils.py, run them, and fix any failures"`


## [​](https://code.claude.com/docs/es/agent-sdk/quickstart#conceptos-clave) Conceptos clave


**Tools** controlan lo que su agente puede hacer:


| Herramientas | Lo que el agente puede hacer |
| --- | --- |
| `Read`, `Glob`, `Grep` | Análisis de solo lectura |
| `Read`, `Edit`, `Glob` | Analizar y modificar código |
| `Read`, `Edit`, `Bash`, `Glob`, `Grep` | Automatización completa |


**Modos de permiso** controlan cuánta supervisión humana desea:


| Modo | Comportamiento | Caso de uso |
| --- | --- | --- |
| `acceptEdits` | Aprueba automáticamente ediciones de archivo y comandos comunes del sistema de archivos, pregunta por otras acciones | Flujos de trabajo de desarrollo confiables |
| `dontAsk` | Deniega cualquier cosa que no esté en `allowedTools` | Agentes sin cabeza bloqueados |
| `auto` (solo TypeScript) | Un clasificador de modelo aprueba o deniega cada llamada de herramienta | Agentes autónomos con protecciones de seguridad |
| `bypassPermissions` | Ejecuta cada herramienta sin indicadores | CI en sandbox, entornos completamente confiables |
| `default` | Requiere una devolución de llamada `canUseTool` para manejar la aprobación | Flujos de aprobación personalizados |


El ejemplo anterior utiliza el modo `acceptEdits`, que aprueba automáticamente las operaciones de archivo para que el agente pueda ejecutarse sin indicadores interactivos. Si desea solicitar a los usuarios la aprobación, utilice el modo `default` y proporcione una devolución de llamada [`canUseTool`](https://code.claude.com/docs/es/agent-sdk/user-input) que recopile la entrada del usuario. Para más control, consulte [Permisos](https://code.claude.com/docs/es/agent-sdk/permissions).


## [​](https://code.claude.com/docs/es/agent-sdk/quickstart#soluci%C3%B3n-de-problemas) Solución de problemas


### [​](https://code.claude.com/docs/es/agent-sdk/quickstart#error-de-api-thinking-type-enabled-no-es-compatible-con-este-modelo) Error de API `thinking.type.enabled` no es compatible con este modelo


Claude Opus 4.7 reemplaza `thinking.type.enabled` con `thinking.type.adaptive`. Las versiones anteriores del SDK de Agent fallan con el siguiente error de API cuando selecciona `claude-opus-4-7`:


```
API Error: 400 {"type":"invalid_request_error","message":"\"thinking.type.enabled\" is not supported for this model. Use \"thinking.type.adaptive\" and \"output_config.effort\" to control thinking behavior."}
```


Actualice a la versión 0.2.111 o posterior del SDK de Agent para usar Opus 4.7.


## [​](https://code.claude.com/docs/es/agent-sdk/quickstart#pr%C3%B3ximos-pasos) Próximos pasos


Ahora que ha creado su primer agente, aprenda cómo extender sus capacidades y adaptarlo a su caso de uso:


- **[Permisos](https://code.claude.com/docs/es/agent-sdk/permissions)**: controle lo que su agente puede hacer y cuándo necesita aprobación
- **[Hooks](https://code.claude.com/docs/es/agent-sdk/hooks)**: ejecute código personalizado antes o después de llamadas de herramientas
- **[Sesiones](https://code.claude.com/docs/es/agent-sdk/sessions)**: construya agentes de múltiples turnos que mantengan contexto
- **[Servidores MCP](https://code.claude.com/docs/es/agent-sdk/mcp)**: conéctese a bases de datos, navegadores, API y otros sistemas externos
- **[Hosting](https://code.claude.com/docs/es/agent-sdk/hosting)**: implemente agentes en Docker, nube e CI/CD
- **[Agentes de ejemplo](https://github.com/anthropics/claude-agent-sdk-demos)**: vea ejemplos completos: asistente de correo electrónico, agente de investigación y más[Claude Code Docs home page](https://code.claude.com/docs/es/overview)

[Privacy choices](https://code.claude.com/docs/es/agent-sdk/quickstart#)

