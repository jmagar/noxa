# TypeScript SDK V2 interface (preview
## ​Instalasi
## ​Mulai cepat
## ​Referensi API
## ​Ketersediaan fitur
## ​Umpan balik
## ​Lihat juga







Pratinjau SDK Agent TypeScript V2 yang disederhanakan, dengan pola send/stream berbasis sesi untuk percakapan multi-turn.

Antarmuka V2 adalah **pratinjau yang tidak stabil**. API mungkin berubah berdasarkan umpan balik sebelum menjadi stabil. Beberapa fitur seperti session forking hanya tersedia di [SDK V1](https://code.claude.com/docs/id/agent-sdk/typescript).
SDK Agent TypeScript Claude V2 menghilangkan kebutuhan untuk async generators dan koordinasi yield. Ini membuat percakapan multi-turn lebih sederhana, alih-alih mengelola status generator di seluruh turn, setiap turn adalah siklus `send()`/ `stream()` terpisah. Permukaan API berkurang menjadi tiga konsep:


- `createSession()` / `resumeSession()`: Mulai atau lanjutkan percakapan
- `session.send()`: Kirim pesan
- `session.stream()`: Dapatkan respons


## [​](https://code.claude.com/docs/id/agent-sdk/typescript-v2-preview#instalasi) Instalasi


Antarmuka V2 disertakan dalam paket SDK yang ada:


```
npm install @anthropic-ai/claude-agent-sdk
```


SDK menggabungkan binary Claude Code asli untuk platform Anda sebagai dependensi opsional, jadi Anda tidak perlu menginstal Claude Code secara terpisah.


## [​](https://code.claude.com/docs/id/agent-sdk/typescript-v2-preview#mulai-cepat) Mulai cepat


### [​](https://code.claude.com/docs/id/agent-sdk/typescript-v2-preview#prompt-sekali-jalan) Prompt sekali jalan


Untuk kueri single-turn sederhana di mana Anda tidak perlu mempertahankan sesi, gunakan `unstable_v2_prompt()`. Contoh ini mengirim pertanyaan matematika dan mencatat jawabannya:


```
import { unstable_v2_prompt } from "@anthropic-ai/claude-agent-sdk";

const result = await unstable_v2_prompt("What is 2 + 2?", {
  model: "claude-opus-4-7"
});
if (result.subtype === "success") {
  console.log(result.result);
}
```


### [​](https://code.claude.com/docs/id/agent-sdk/typescript-v2-preview#sesi-dasar) Sesi dasar


Untuk interaksi di luar prompt tunggal, buat sesi. V2 memisahkan pengiriman dan streaming menjadi langkah-langkah yang berbeda:


- `send()` mengirimkan pesan Anda
- `stream()` mengalirkan respons kembali


Pemisahan eksplisit ini memudahkan untuk menambahkan logika antar turn (seperti memproses respons sebelum mengirim tindak lanjut).
Contoh di bawah membuat sesi, mengirim “Hello!” ke Claude, dan mencetak respons teks. Ini menggunakan [`await using`](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-2.html#using-declarations-and-explicit-resource-management) (TypeScript 5.2+) untuk secara otomatis menutup sesi ketika blok keluar. Anda juga dapat memanggil `session.close()` secara manual.


```
import { unstable_v2_createSession } from "@anthropic-ai/claude-agent-sdk";

await using session = unstable_v2_createSession({
  model: "claude-opus-4-7"
});

await session.send("Hello!");
for await (const msg of session.stream()) {
  // Filter for assistant messages to get human-readable output
  if (msg.type === "assistant") {
    const text = msg.message.content
      .filter((block) => block.type === "text")
      .map((block) => block.text)
      .join("");
    console.log(text);
  }
}
```


### [​](https://code.claude.com/docs/id/agent-sdk/typescript-v2-preview#percakapan-multi-turn) Percakapan multi-turn


Sesi mempertahankan konteks di seluruh pertukaran berganda. Untuk melanjutkan percakapan, panggil `send()` lagi pada sesi yang sama. Claude mengingat turn sebelumnya.
Contoh ini mengajukan pertanyaan matematika, kemudian mengajukan tindak lanjut yang mereferensikan jawaban sebelumnya:


```
import { unstable_v2_createSession } from "@anthropic-ai/claude-agent-sdk";

await using session = unstable_v2_createSession({
  model: "claude-opus-4-7"
});

// Turn 1
await session.send("What is 5 + 3?");
for await (const msg of session.stream()) {
  // Filter for assistant messages to get human-readable output
  if (msg.type === "assistant") {
    const text = msg.message.content
      .filter((block) => block.type === "text")
      .map((block) => block.text)
      .join("");
    console.log(text);
  }
}

// Turn 2
await session.send("Multiply that by 2");
for await (const msg of session.stream()) {
  if (msg.type === "assistant") {
    const text = msg.message.content
      .filter((block) => block.type === "text")
      .map((block) => block.text)
      .join("");
    console.log(text);
  }
}
```


### [​](https://code.claude.com/docs/id/agent-sdk/typescript-v2-preview#lanjutkan-sesi) Lanjutkan sesi


Jika Anda memiliki ID sesi dari interaksi sebelumnya, Anda dapat melanjutkannya nanti. Ini berguna untuk alur kerja yang berjalan lama atau ketika Anda perlu mempertahankan percakapan di seluruh restart aplikasi.
Contoh ini membuat sesi, menyimpan ID-nya, menutupnya, kemudian melanjutkan percakapan:


```
import {
  unstable_v2_createSession,
  unstable_v2_resumeSession,
  type SDKMessage
} from "@anthropic-ai/claude-agent-sdk";

// Helper to extract text from assistant messages
function getAssistantText(msg: SDKMessage): string | null {
  if (msg.type !== "assistant") return null;
  return msg.message.content
    .filter((block) => block.type === "text")
    .map((block) => block.text)
    .join("");
}

// Create initial session and have a conversation
const session = unstable_v2_createSession({
  model: "claude-opus-4-7"
});

await session.send("Remember this number: 42");

// Get the session ID from any received message
let sessionId: string | undefined;
for await (const msg of session.stream()) {
  sessionId = msg.session_id;
  const text = getAssistantText(msg);
  if (text) console.log("Initial response:", text);
}

console.log("Session ID:", sessionId);
session.close();

// Later: resume the session using the stored ID
await using resumedSession = unstable_v2_resumeSession(sessionId!, {
  model: "claude-opus-4-7"
});

await resumedSession.send("What number did I ask you to remember?");
for await (const msg of resumedSession.stream()) {
  const text = getAssistantText(msg);
  if (text) console.log("Resumed response:", text);
}
```


### [​](https://code.claude.com/docs/id/agent-sdk/typescript-v2-preview#pembersihan) Pembersihan


Sesi dapat ditutup secara manual atau otomatis menggunakan [`await using`](https://www.typescriptlang.org/docs/handbook/release-notes/typescript-5-2.html#using-declarations-and-explicit-resource-management), fitur TypeScript 5.2+ untuk pembersihan sumber daya otomatis. Jika Anda menggunakan versi TypeScript yang lebih lama atau mengalami masalah kompatibilitas, gunakan pembersihan manual sebagai gantinya.
**Pembersihan otomatis (TypeScript 5.2+):**


```
import { unstable_v2_createSession } from "@anthropic-ai/claude-agent-sdk";

await using session = unstable_v2_createSession({
  model: "claude-opus-4-7"
});
// Session closes automatically when the block exits
```


**Pembersihan manual:**


```
import { unstable_v2_createSession } from "@anthropic-ai/claude-agent-sdk";

const session = unstable_v2_createSession({
  model: "claude-opus-4-7"
});
// ... use the session ...
session.close();
```


## [​](https://code.claude.com/docs/id/agent-sdk/typescript-v2-preview#referensi-api) Referensi API


### [​](https://code.claude.com/docs/id/agent-sdk/typescript-v2-preview#unstable_v2_createsession) `unstable_v2_createSession()`


Membuat sesi baru untuk percakapan multi-turn.


```
function unstable_v2_createSession(options: {
  model: string;
  // Additional options supported
}): SDKSession;
```


### [​](https://code.claude.com/docs/id/agent-sdk/typescript-v2-preview#unstable_v2_resumesession) `unstable_v2_resumeSession()`


Melanjutkan sesi yang ada berdasarkan ID.


```
function unstable_v2_resumeSession(
  sessionId: string,
  options: {
    model: string;
    // Additional options supported
  }
): SDKSession;
```


### [​](https://code.claude.com/docs/id/agent-sdk/typescript-v2-preview#unstable_v2_prompt) `unstable_v2_prompt()`


Fungsi kenyamanan sekali jalan untuk kueri single-turn.


```
function unstable_v2_prompt(
  prompt: string,
  options: {
    model: string;
    // Additional options supported
  }
): Promise<SDKResultMessage>;
```


### [​](https://code.claude.com/docs/id/agent-sdk/typescript-v2-preview#antarmuka-sdksession) Antarmuka SDKSession


```
interface SDKSession {
  readonly sessionId: string;
  send(message: string | SDKUserMessage): Promise<void>;
  stream(): AsyncGenerator<SDKMessage, void>;
  close(): void;
}
```


## [​](https://code.claude.com/docs/id/agent-sdk/typescript-v2-preview#ketersediaan-fitur) Ketersediaan fitur


Tidak semua fitur V1 tersedia di V2 belum. Berikut ini memerlukan penggunaan [SDK V1](https://code.claude.com/docs/id/agent-sdk/typescript):


- Session forking (opsi `forkSession`)
- Beberapa pola input streaming lanjutan


## [​](https://code.claude.com/docs/id/agent-sdk/typescript-v2-preview#umpan-balik) Umpan balik


Bagikan umpan balik Anda tentang antarmuka V2 sebelum menjadi stabil. Laporkan masalah dan saran melalui [GitHub Issues](https://github.com/anthropics/claude-code/issues).


## [​](https://code.claude.com/docs/id/agent-sdk/typescript-v2-preview#lihat-juga) Lihat juga


- [Referensi TypeScript SDK (V1)](https://code.claude.com/docs/id/agent-sdk/typescript) - Dokumentasi SDK V1 lengkap
- [Gambaran umum SDK](https://code.claude.com/docs/id/agent-sdk/overview) - Konsep SDK umum
- [Contoh V2 di GitHub](https://github.com/anthropics/claude-agent-sdk-demos/tree/main/hello-world-v2) - Contoh kode yang berfungsi[Claude Code Docs home page](https://code.claude.com/docs/id/overview)

[Privacy choices](https://code.claude.com/docs/id/agent-sdk/typescript-v2-preview#)

