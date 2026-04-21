# Gambaran Umum Agent SDK
## ​Memulai
## ​Kemampuan
## ​Bandingkan Agent SDK dengan alat Claude lainnya
## ​Changelog
## ​Melaporkan bug
## ​Pedoman branding
## ​Lisensi dan persyaratan
## ​Langkah berikutnya









Bangun agen AI produksi dengan Claude Code sebagai perpustakaan

Claude Code SDK telah diubah nama menjadi Claude Agent SDK. Jika Anda bermigrasi dari SDK lama, lihat [Panduan Migrasi](https://code.claude.com/docs/id/agent-sdk/migration-guide).
Bangun agen AI yang secara mandiri membaca file, menjalankan perintah, mencari web, mengedit kode, dan banyak lagi. Agent SDK memberi Anda alat yang sama, loop agen, dan manajemen konteks yang mendukung Claude Code, dapat diprogram dalam Python dan TypeScript.
Opus 4.7 ( `claude-opus-4-7`) memerlukan Agent SDK v0.2.111 atau lebih baru. Jika Anda melihat kesalahan API `thinking.type.enabled`, lihat [Troubleshooting](https://code.claude.com/docs/id/agent-sdk/quickstart#troubleshooting).
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


Agent SDK mencakup alat bawaan untuk membaca file, menjalankan perintah, dan mengedit kode, sehingga agen Anda dapat mulai bekerja segera tanpa Anda perlu mengimplementasikan eksekusi alat. Selami panduan cepat atau jelajahi agen nyata yang dibangun dengan SDK:


## Panduan Cepat

Bangun agen perbaikan bug dalam hitungan menit

## Agen contoh

Asisten email, agen penelitian, dan banyak lagi


## [​](https://code.claude.com/docs/id/agent-sdk/overview#memulai) Memulai


1

Instal SDK


- TypeScript
- Python


```
npm install @anthropic-ai/claude-agent-sdk
```


```
pip install claude-agent-sdk
```

TypeScript SDK menggabungkan biner Claude Code asli untuk platform Anda sebagai dependensi opsional, jadi Anda tidak perlu menginstal Claude Code secara terpisah. 2

Atur kunci API Anda

Dapatkan kunci API dari [Konsol](https://platform.claude.com/), kemudian atur sebagai variabel lingkungan:

```
export ANTHROPIC_API_KEY=your-api-key
```

SDK juga mendukung autentikasi melalui penyedia API pihak ketiga:

- **Amazon Bedrock**: atur variabel lingkungan `CLAUDE_CODE_USE_BEDROCK=1` dan konfigurasi kredensial AWS
- **Google Vertex AI**: atur variabel lingkungan `CLAUDE_CODE_USE_VERTEX=1` dan konfigurasi kredensial Google Cloud
- **Microsoft Azure**: atur variabel lingkungan `CLAUDE_CODE_USE_FOUNDRY=1` dan konfigurasi kredensial Azure

Lihat panduan penyiapan untuk [Bedrock](https://code.claude.com/docs/id/amazon-bedrock), [Vertex AI](https://code.claude.com/docs/id/google-vertex-ai), atau [Azure AI Foundry](https://code.claude.com/docs/id/microsoft-foundry) untuk detail. Kecuali sebelumnya disetujui, Anthropic tidak mengizinkan pengembang pihak ketiga untuk menawarkan login claude.ai atau batas laju untuk produk mereka, termasuk agen yang dibangun di Agent SDK Claude. Silakan gunakan metode autentikasi kunci API yang dijelaskan dalam dokumen ini. 3

Jalankan agen pertama Anda

Contoh ini membuat agen yang mencantumkan file di direktori saat ini menggunakan alat bawaan. Python TypeScript

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


**Siap untuk membangun?** Ikuti [Panduan Cepat](https://code.claude.com/docs/id/agent-sdk/quickstart) untuk membuat agen yang menemukan dan memperbaiki bug dalam hitungan menit.


## [​](https://code.claude.com/docs/id/agent-sdk/overview#kemampuan) Kemampuan


Semua yang membuat Claude Code kuat tersedia di SDK:


- Alat bawaan
- Hooks
- Subagents
- MCP
- Izin
- Sesi

Agen Anda dapat membaca file, menjalankan perintah, dan mencari basis kode langsung dari kotak. Alat kunci meliputi:

| Alat | Apa yang dilakukannya |
| --- | --- |
| **Read** | Baca file apa pun di direktori kerja |
| **Write** | Buat file baru |
| **Edit** | Buat pengeditan presisi pada file yang ada |
| **Bash** | Jalankan perintah terminal, skrip, operasi git |
| **Monitor** | Pantau skrip latar belakang dan bereaksi terhadap setiap baris output sebagai acara |
| **Glob** | Temukan file berdasarkan pola ( `**/*.ts`, `src/**/*.py`) |
| **Grep** | Cari konten file dengan regex |
| **WebSearch** | Cari web untuk informasi terkini |
| **WebFetch** | Ambil dan parsing konten halaman web |
| **[AskUserQuestion](https://code.claude.com/docs/id/agent-sdk/user-input#handle-clarifying-questions)** | Tanyakan pertanyaan klarifikasi kepada pengguna dengan opsi pilihan ganda |

Contoh ini membuat agen yang mencari basis kode Anda untuk komentar TODO: Python TypeScript

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

Jalankan kode khusus pada titik-titik kunci dalam siklus hidup agen. SDK hooks menggunakan fungsi callback untuk memvalidasi, mencatat, memblokir, atau mengubah perilaku agen. **Hooks yang tersedia:** `PreToolUse`, `PostToolUse`, `Stop`, `SessionStart`, `SessionEnd`, `UserPromptSubmit`, dan banyak lagi. Contoh ini mencatat semua perubahan file ke file audit: Python TypeScript

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

[Pelajari lebih lanjut tentang hooks →](https://code.claude.com/docs/id/agent-sdk/hooks) Spawn agen khusus untuk menangani subtask yang terfokus. Agen utama Anda mendelegasikan pekerjaan, dan subagen melaporkan kembali dengan hasil. Tentukan agen khusus dengan instruksi khusus. Sertakan `Agent` dalam `allowedTools` karena subagen dipanggil melalui alat Agent: Python TypeScript

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

Pesan dari dalam konteks subagen mencakup bidang `parent_tool_use_id`, memungkinkan Anda melacak pesan mana yang termasuk dalam eksekusi subagen mana. [Pelajari lebih lanjut tentang subagents →](https://code.claude.com/docs/id/agent-sdk/subagents) Terhubung ke sistem eksternal melalui Model Context Protocol: database, browser, API, dan [ratusan lainnya](https://github.com/modelcontextprotocol/servers). Contoh ini menghubungkan [server Playwright MCP](https://github.com/microsoft/playwright-mcp) untuk memberikan agen Anda kemampuan otomasi browser: Python TypeScript

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

[Pelajari lebih lanjut tentang MCP →](https://code.claude.com/docs/id/agent-sdk/mcp) Kontrol dengan tepat alat mana yang dapat digunakan agen Anda. Izinkan operasi yang aman, blokir yang berbahaya, atau minta persetujuan untuk tindakan sensitif. Untuk prompt persetujuan interaktif dan alat `AskUserQuestion`, lihat [Tangani persetujuan dan input pengguna](https://code.claude.com/docs/id/agent-sdk/user-input). Contoh ini membuat agen read-only yang dapat menganalisis tetapi tidak memodifikasi kode. `allowed_tools` pra-menyetujui `Read`, `Glob`, dan `Grep`. Python TypeScript

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

[Pelajari lebih lanjut tentang izin →](https://code.claude.com/docs/id/agent-sdk/permissions) Pertahankan konteks di seluruh pertukaran berganda. Claude mengingat file yang dibaca, analisis yang dilakukan, dan riwayat percakapan. Lanjutkan sesi nanti, atau fork mereka untuk menjelajahi pendekatan berbeda. Contoh ini menangkap ID sesi dari kueri pertama, kemudian melanjutkan untuk terus dengan konteks penuh: Python TypeScript

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

[Pelajari lebih lanjut tentang sesi →](https://code.claude.com/docs/id/agent-sdk/sessions)


### [​](https://code.claude.com/docs/id/agent-sdk/overview#fitur-claude-code) Fitur Claude Code


SDK juga mendukung konfigurasi berbasis filesystem Claude Code. Dengan opsi default, SDK memuat ini dari `.claude/` di direktori kerja Anda dan `~/.claude/`. Untuk membatasi sumber mana yang dimuat, atur `setting_sources` (Python) atau `settingSources` (TypeScript) dalam opsi Anda.


| Fitur | Deskripsi | Lokasi |
| --- | --- | --- |
| [Skills](https://code.claude.com/docs/id/agent-sdk/skills) | Kemampuan khusus yang ditentukan dalam Markdown | `.claude/skills/*/SKILL.md` |
| [Slash commands](https://code.claude.com/docs/id/agent-sdk/slash-commands) | Perintah khusus untuk tugas umum | `.claude/commands/*.md` |
| [Memory](https://code.claude.com/docs/id/agent-sdk/modifying-system-prompts) | Konteks proyek dan instruksi | `CLAUDE.md` atau `.claude/CLAUDE.md` |
| [Plugins](https://code.claude.com/docs/id/agent-sdk/plugins) | Perluas dengan perintah khusus, agen, dan server MCP | Programmatic via `plugins` option |


## [​](https://code.claude.com/docs/id/agent-sdk/overview#bandingkan-agent-sdk-dengan-alat-claude-lainnya) Bandingkan Agent SDK dengan alat Claude lainnya


Platform Claude menawarkan berbagai cara untuk membangun dengan Claude. Berikut cara Agent SDK cocok:


- Agent SDK vs Client SDK
- Agent SDK vs Claude Code CLI

[Anthropic Client SDK](https://platform.claude.com/docs/en/api/client-sdks) memberi Anda akses API langsung: Anda mengirim prompt dan mengimplementasikan eksekusi alat sendiri. **Agent SDK** memberi Anda Claude dengan eksekusi alat bawaan. Dengan Client SDK, Anda mengimplementasikan loop alat. Dengan Agent SDK, Claude menanganinya: Python TypeScript

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

Kemampuan yang sama, antarmuka berbeda:

| Kasus penggunaan | Pilihan terbaik |
| --- | --- |
| Pengembangan interaktif | CLI |
| Pipeline CI/CD | SDK |
| Aplikasi khusus | SDK |
| Tugas sekali jalan | CLI |
| Otomasi produksi | SDK |

Banyak tim menggunakan keduanya: CLI untuk pengembangan harian, SDK untuk produksi. Alur kerja diterjemahkan langsung di antara keduanya.


## [​](https://code.claude.com/docs/id/agent-sdk/overview#changelog) Changelog


Lihat changelog lengkap untuk pembaruan SDK, perbaikan bug, dan fitur baru:


- **TypeScript SDK**: [lihat CHANGELOG.md](https://github.com/anthropics/claude-agent-sdk-typescript/blob/main/CHANGELOG.md)
- **Python SDK**: [lihat CHANGELOG.md](https://github.com/anthropics/claude-agent-sdk-python/blob/main/CHANGELOG.md)


## [​](https://code.claude.com/docs/id/agent-sdk/overview#melaporkan-bug) Melaporkan bug


Jika Anda mengalami bug atau masalah dengan Agent SDK:


- **TypeScript SDK**: [laporkan masalah di GitHub](https://github.com/anthropics/claude-agent-sdk-typescript/issues)
- **Python SDK**: [laporkan masalah di GitHub](https://github.com/anthropics/claude-agent-sdk-python/issues)


## [​](https://code.claude.com/docs/id/agent-sdk/overview#pedoman-branding) Pedoman branding


Untuk mitra yang mengintegrasikan Claude Agent SDK, penggunaan branding Claude bersifat opsional. Saat mereferensikan Claude dalam produk Anda:
**Diizinkan:**


- “Claude Agent” (lebih disukai untuk menu dropdown)
- “Claude” (ketika sudah dalam menu berlabel “Agents”)
- ” Powered by Claude” (jika Anda memiliki nama agen yang ada)


**Tidak diizinkan:**


- “Claude Code” atau “Claude Code Agent”
- Elemen visual atau ASCII art bermerek Claude Code yang meniru Claude Code


Produk Anda harus mempertahankan branding sendiri dan tidak boleh terlihat seperti Claude Code atau produk Anthropic apa pun. Untuk pertanyaan tentang kepatuhan branding, hubungi [tim penjualan](https://www.anthropic.com/contact-sales) Anthropic.


## [​](https://code.claude.com/docs/id/agent-sdk/overview#lisensi-dan-persyaratan) Lisensi dan persyaratan


Penggunaan Claude Agent SDK diatur oleh [Persyaratan Layanan Komersial Anthropic](https://www.anthropic.com/legal/commercial-terms), termasuk ketika Anda menggunakannya untuk memberdayakan produk dan layanan yang Anda buat tersedia untuk pelanggan dan pengguna akhir Anda sendiri, kecuali sejauh komponen atau dependensi tertentu dicakup oleh lisensi berbeda seperti yang ditunjukkan dalam file LICENSE komponen tersebut.


## [​](https://code.claude.com/docs/id/agent-sdk/overview#langkah-berikutnya) Langkah berikutnya


## Panduan Cepat

Bangun agen yang menemukan dan memperbaiki bug dalam hitungan menit

## Agen contoh

Asisten email, agen penelitian, dan banyak lagi

## TypeScript SDK

Referensi API TypeScript lengkap dan contoh

## Python SDK

Referensi API Python lengkap dan contoh[Claude Code Docs home page](https://code.claude.com/docs/id/overview)

[Privacy choices](https://code.claude.com/docs/id/agent-sdk/overview#)

