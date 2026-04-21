# Panduan Cepat
## ​Prasyarat
## ​Penyiapan
## ​Buat file dengan bug
## ​Bangun agen yang menemukan dan memperbaiki bug
## ​Konsep kunci
## ​Pemecahan masalah
## ​Langkah berikutnya








Mulai dengan Agent SDK Python atau TypeScript untuk membangun agen AI yang bekerja secara mandiri

Gunakan Agent SDK untuk membangun agen AI yang membaca kode Anda, menemukan bug, dan memperbaikinya, semuanya tanpa intervensi manual.
**Yang akan Anda lakukan:**


1. Menyiapkan proyek dengan Agent SDK
2. Membuat file dengan beberapa kode yang berisi bug
3. Menjalankan agen yang menemukan dan memperbaiki bug secara otomatis


## [​](https://code.claude.com/docs/id/agent-sdk/quickstart#prasyarat) Prasyarat


- **Node.js 18+** atau **Python 3.10+**
- Akun **Anthropic** ([daftar di sini](https://platform.claude.com/))


## [​](https://code.claude.com/docs/id/agent-sdk/quickstart#penyiapan) Penyiapan


1

Buat folder proyek

Buat direktori baru untuk panduan cepat ini:

```
mkdir my-agent && cd my-agent
```

Untuk proyek Anda sendiri, Anda dapat menjalankan SDK dari folder apa pun; SDK akan memiliki akses ke file di direktori tersebut dan subdirektorinya secara default. 2

Instal SDK

Instal paket Agent SDK untuk bahasa Anda:

- TypeScript
- Python (uv)
- Python (pip)


```
npm install @anthropic-ai/claude-agent-sdk
```

[uv Python package manager](https://docs.astral.sh/uv/) adalah pengelola paket Python yang cepat dan menangani lingkungan virtual secara otomatis:

```
uv init && uv add claude-agent-sdk
```

Buat lingkungan virtual terlebih dahulu, kemudian instal:

```
python3 -m venv .venv && source .venv/bin/activate
pip3 install claude-agent-sdk
```

TypeScript SDK menggabungkan biner Claude Code asli untuk platform Anda sebagai dependensi opsional, jadi Anda tidak perlu menginstal Claude Code secara terpisah. 3

Atur kunci API Anda

Dapatkan kunci API dari [Claude Console](https://platform.claude.com/), kemudian buat file `.env` di direktori proyek Anda:

```
ANTHROPIC_API_KEY=your-api-key
```

SDK juga mendukung autentikasi melalui penyedia API pihak ketiga:

- **Amazon Bedrock**: atur variabel lingkungan `CLAUDE_CODE_USE_BEDROCK=1` dan konfigurasikan kredensial AWS
- **Google Vertex AI**: atur variabel lingkungan `CLAUDE_CODE_USE_VERTEX=1` dan konfigurasikan kredensial Google Cloud
- **Microsoft Azure**: atur variabel lingkungan `CLAUDE_CODE_USE_FOUNDRY=1` dan konfigurasikan kredensial Azure

Lihat panduan penyiapan untuk [Bedrock](https://code.claude.com/docs/id/amazon-bedrock), [Vertex AI](https://code.claude.com/docs/id/google-vertex-ai), atau [Azure AI Foundry](https://code.claude.com/docs/id/microsoft-foundry) untuk detail selengkapnya. Kecuali telah disetujui sebelumnya, Anthropic tidak mengizinkan pengembang pihak ketiga untuk menawarkan login claude.ai atau batas laju untuk produk mereka, termasuk agen yang dibangun di Agent SDK Claude. Silakan gunakan metode autentikasi kunci API yang dijelaskan dalam dokumen ini.


## [​](https://code.claude.com/docs/id/agent-sdk/quickstart#buat-file-dengan-bug) Buat file dengan bug


Panduan cepat ini memandu Anda melalui pembuatan agen yang dapat menemukan dan memperbaiki bug dalam kode. Pertama, Anda memerlukan file dengan beberapa bug yang disengaja untuk diperbaiki oleh agen. Buat `utils.py` di direktori `my-agent` dan tempel kode berikut:


```
def calculate_average(numbers):
    total = 0
    for num in numbers:
        total += num
    return total / len(numbers)


def get_user_name(user):
    return user["name"].upper()
```


Kode ini memiliki dua bug:


1. `calculate_average([])` mogok dengan pembagian oleh nol
2. `get_user_name(None)` mogok dengan TypeError


## [​](https://code.claude.com/docs/id/agent-sdk/quickstart#bangun-agen-yang-menemukan-dan-memperbaiki-bug) Bangun agen yang menemukan dan memperbaiki bug


Buat `agent.py` jika Anda menggunakan Python SDK, atau `agent.ts` untuk TypeScript:
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


Kode ini memiliki tiga bagian utama:


1. **`query`**: titik masuk utama yang membuat loop agentic. Ini mengembalikan iterator async, jadi Anda menggunakan `async for` untuk streaming pesan saat Claude bekerja. Lihat API lengkap di referensi SDK [Python](https://code.claude.com/docs/id/agent-sdk/python#query) atau [TypeScript](https://code.claude.com/docs/id/agent-sdk/typescript#query).
2. **`prompt`**: apa yang ingin Anda lakukan Claude. Claude mengetahui alat mana yang digunakan berdasarkan tugas.
3. **`options`**: konfigurasi untuk agen. Contoh ini menggunakan `allowedTools` untuk pra-persetujuan `Read`, `Edit`, dan `Glob`, dan `permissionMode: "acceptEdits"` untuk auto-persetujuan perubahan file. Opsi lainnya termasuk `systemPrompt`, `mcpServers`, dan lainnya. Lihat semua opsi untuk [Python](https://code.claude.com/docs/id/agent-sdk/python#claude-agent-options) atau [TypeScript](https://code.claude.com/docs/id/agent-sdk/typescript#options).


Loop `async for` terus berjalan saat Claude berpikir, memanggil alat, mengamati hasil, dan memutuskan apa yang harus dilakukan selanjutnya. Setiap iterasi menghasilkan pesan: penalaran Claude, panggilan alat, hasil alat, atau hasil akhir. SDK menangani orkestrasi (eksekusi alat, manajemen konteks, percobaan ulang) sehingga Anda hanya mengonsumsi aliran. Loop berakhir ketika Claude menyelesaikan tugas atau mengalami kesalahan.
Penanganan pesan di dalam loop memfilter output yang dapat dibaca manusia. Tanpa penyaringan, Anda akan melihat objek pesan mentah termasuk inisialisasi sistem dan status internal, yang berguna untuk debugging tetapi berisik sebaliknya.
Contoh ini menggunakan streaming untuk menampilkan kemajuan secara real-time. Jika Anda tidak memerlukan output langsung (misalnya, untuk pekerjaan latar belakang atau pipeline CI), Anda dapat mengumpulkan semua pesan sekaligus. Lihat [Streaming vs. single-turn mode](https://code.claude.com/docs/id/agent-sdk/streaming-vs-single-mode) untuk detail selengkapnya.


### [​](https://code.claude.com/docs/id/agent-sdk/quickstart#jalankan-agen-anda) Jalankan agen Anda


Agen Anda siap. Jalankan dengan perintah berikut:


- Python
- TypeScript


```
python3 agent.py
```


```
npx tsx agent.ts
```


Setelah menjalankan, periksa `utils.py`. Anda akan melihat kode defensif yang menangani daftar kosong dan pengguna null. Agen Anda secara mandiri:


1. **Membaca** `utils.py` untuk memahami kode
2. **Menganalisis** logika dan mengidentifikasi kasus tepi yang akan mogok
3. **Mengedit** file untuk menambahkan penanganan kesalahan yang tepat


Inilah yang membuat Agent SDK berbeda: Claude menjalankan alat secara langsung alih-alih meminta Anda untuk mengimplementasikannya.
Jika Anda melihat “API key not found”, pastikan Anda telah menetapkan variabel lingkungan `ANTHROPIC_API_KEY` di file `.env` atau lingkungan shell Anda. Lihat [panduan pemecahan masalah lengkap](https://code.claude.com/docs/id/troubleshooting) untuk bantuan lebih lanjut.


### [​](https://code.claude.com/docs/id/agent-sdk/quickstart#coba-prompt-lain) Coba prompt lain


Sekarang agen Anda sudah diatur, coba beberapa prompt berbeda:


- `"Add docstrings to all functions in utils.py"`
- `"Add type hints to all functions in utils.py"`
- `"Create a README.md documenting the functions in utils.py"`


### [​](https://code.claude.com/docs/id/agent-sdk/quickstart#sesuaikan-agen-anda) Sesuaikan agen Anda


Anda dapat mengubah perilaku agen dengan mengubah opsi. Berikut adalah beberapa contoh:
**Tambahkan kemampuan pencarian web:**
Python TypeScript

```
options = ClaudeAgentOptions(
    allowed_tools=["Read", "Edit", "Glob", "WebSearch"], permission_mode="acceptEdits"
)
```


**Berikan Claude prompt sistem kustom:**
Python TypeScript

```
options = ClaudeAgentOptions(
    allowed_tools=["Read", "Edit", "Glob"],
    permission_mode="acceptEdits",
    system_prompt="You are a senior Python developer. Always follow PEP 8 style guidelines.",
)
```


**Jalankan perintah di terminal:**
Python TypeScript

```
options = ClaudeAgentOptions(
    allowed_tools=["Read", "Edit", "Glob", "Bash"], permission_mode="acceptEdits"
)
```


Dengan `Bash` diaktifkan, coba: `"Write unit tests for utils.py, run them, and fix any failures"`


## [​](https://code.claude.com/docs/id/agent-sdk/quickstart#konsep-kunci) Konsep kunci


**Tools** mengontrol apa yang dapat dilakukan agen Anda:


| Tools | Apa yang dapat dilakukan agen |
| --- | --- |
| `Read`, `Glob`, `Grep` | Analisis hanya-baca |
| `Read`, `Edit`, `Glob` | Analisis dan modifikasi kode |
| `Read`, `Edit`, `Bash`, `Glob`, `Grep` | Otomasi penuh |


**Permission modes** mengontrol berapa banyak pengawasan manusia yang Anda inginkan:


| Mode | Perilaku | Kasus penggunaan |
| --- | --- | --- |
| `acceptEdits` | Auto-persetujuan pengeditan file dan perintah sistem file umum, meminta tindakan lain | Alur kerja pengembangan terpercaya |
| `dontAsk` | Menolak apa pun yang tidak ada di `allowedTools` | Agen headless terkunci |
| `auto` (TypeScript only) | Pengklasifikasi model menyetujui atau menolak setiap panggilan alat | Agen otonom dengan penjaga keamanan |
| `bypassPermissions` | Menjalankan setiap alat tanpa prompt | CI sandboxed, lingkungan yang sepenuhnya terpercaya |
| `default` | Memerlukan callback `canUseTool` untuk menangani persetujuan | Alur persetujuan kustom |


Contoh di atas menggunakan mode `acceptEdits`, yang auto-persetujuan operasi file sehingga agen dapat berjalan tanpa prompt interaktif. Jika Anda ingin meminta pengguna untuk persetujuan, gunakan mode `default` dan sediakan callback [`canUseTool`](https://code.claude.com/docs/id/agent-sdk/user-input) yang mengumpulkan input pengguna. Untuk kontrol lebih lanjut, lihat [Permissions](https://code.claude.com/docs/id/agent-sdk/permissions).


## [​](https://code.claude.com/docs/id/agent-sdk/quickstart#pemecahan-masalah) Pemecahan masalah


### [​](https://code.claude.com/docs/id/agent-sdk/quickstart#kesalahan-api-thinking-type-enabled-tidak-didukung-untuk-model-ini) Kesalahan API `thinking.type.enabled` tidak didukung untuk model ini


Claude Opus 4.7 menggantikan `thinking.type.enabled` dengan `thinking.type.adaptive`. Versi Agent SDK yang lebih lama gagal dengan kesalahan API berikut ketika Anda memilih `claude-opus-4-7`:


```
API Error: 400 {"type":"invalid_request_error","message":"\"thinking.type.enabled\" is not supported for this model. Use \"thinking.type.adaptive\" and \"output_config.effort\" to control thinking behavior."}
```


Tingkatkan ke Agent SDK v0.2.111 atau lebih baru untuk menggunakan Opus 4.7.


## [​](https://code.claude.com/docs/id/agent-sdk/quickstart#langkah-berikutnya) Langkah berikutnya


Sekarang Anda telah membuat agen pertama Anda, pelajari cara memperluas kemampuannya dan menyesuaikannya dengan kasus penggunaan Anda:


- **[Permissions](https://code.claude.com/docs/id/agent-sdk/permissions)**: kontrol apa yang dapat dilakukan agen Anda dan kapan memerlukan persetujuan
- **[Hooks](https://code.claude.com/docs/id/agent-sdk/hooks)**: jalankan kode kustom sebelum atau sesudah panggilan alat
- **[Sessions](https://code.claude.com/docs/id/agent-sdk/sessions)**: bangun agen multi-turn yang mempertahankan konteks
- **[MCP servers](https://code.claude.com/docs/id/agent-sdk/mcp)**: terhubung ke database, browser, API, dan sistem eksternal lainnya
- **[Hosting](https://code.claude.com/docs/id/agent-sdk/hosting)**: sebarkan agen ke Docker, cloud, dan CI/CD
- **[Example agents](https://github.com/anthropics/claude-agent-sdk-demos)**: lihat contoh lengkap: asisten email, agen penelitian, dan lainnya[Claude Code Docs home page](https://code.claude.com/docs/id/overview)

[Privacy choices](https://code.claude.com/docs/id/agent-sdk/quickstart#)

