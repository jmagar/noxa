# Hosting the Agent SDK
## ​Persyaratan Hosting
## ​Memahami Arsitektur SDK
## ​Opsi Penyedia Sandbox
## ​Pola Penerapan Produksi
## ​FAQ
## ​Langkah Berikutnya







Menerapkan dan menghosting Claude Agent SDK di lingkungan produksi

Claude Agent SDK berbeda dari API LLM stateless tradisional karena mempertahankan status percakapan dan menjalankan perintah di lingkungan yang persisten. Panduan ini mencakup arsitektur, pertimbangan hosting, dan praktik terbaik untuk menerapkan agen berbasis SDK dalam produksi.
Untuk pengerasan keamanan di luar sandboxing dasar (termasuk kontrol jaringan, manajemen kredensial, dan opsi isolasi), lihat [Secure Deployment](https://code.claude.com/docs/id/agent-sdk/secure-deployment).


## [​](https://code.claude.com/docs/id/agent-sdk/hosting#persyaratan-hosting) Persyaratan Hosting


### [​](https://code.claude.com/docs/id/agent-sdk/hosting#container-based-sandboxing) Container-Based Sandboxing


Untuk keamanan dan isolasi, SDK harus berjalan di dalam lingkungan kontainer yang tersandbox. Ini menyediakan isolasi proses, batasan sumber daya, kontrol jaringan, dan sistem file yang bersifat sementara.
SDK juga mendukung [konfigurasi sandbox terprogram](https://code.claude.com/docs/id/agent-sdk/typescript#sandbox-settings) untuk eksekusi perintah.


### [​](https://code.claude.com/docs/id/agent-sdk/hosting#persyaratan-sistem) Persyaratan Sistem


Setiap instans SDK memerlukan:


- **Dependensi runtime**
  - Python 3.10+ untuk Python SDK, atau Node.js 18+ untuk TypeScript SDK
  - Kedua paket SDK menggabungkan biner Claude Code asli untuk platform host, jadi tidak perlu instalasi Claude Code atau Node.js terpisah untuk CLI yang dijalankan
- **Alokasi sumber daya**
  - Direkomendasikan: 1GiB RAM, 5GiB disk, dan 1 CPU (sesuaikan ini berdasarkan tugas Anda sesuai kebutuhan)
- **Akses jaringan**
  - HTTPS keluar ke `api.anthropic.com`
  - Opsional: Akses ke server MCP atau alat eksternal


## [​](https://code.claude.com/docs/id/agent-sdk/hosting#memahami-arsitektur-sdk) Memahami Arsitektur SDK


Tidak seperti panggilan API stateless, Claude Agent SDK beroperasi sebagai **proses yang berjalan lama** yang:


- **Menjalankan perintah** di lingkungan shell yang persisten
- **Mengelola operasi file** dalam direktori kerja
- **Menangani eksekusi alat** dengan konteks dari interaksi sebelumnya


## [​](https://code.claude.com/docs/id/agent-sdk/hosting#opsi-penyedia-sandbox) Opsi Penyedia Sandbox


Beberapa penyedia mengkhususkan diri dalam lingkungan kontainer aman untuk eksekusi kode AI:


- **[Modal Sandbox](https://modal.com/docs/guide/sandbox)** - [demo implementation](https://modal.com/docs/examples/claude-slack-gif-creator)
- **[Cloudflare Sandboxes](https://github.com/cloudflare/sandbox-sdk)**
- **[Daytona](https://www.daytona.io/)**
- **[E2B](https://e2b.dev/)**
- **[Fly Machines](https://fly.io/docs/machines/)**
- **[Vercel Sandbox](https://vercel.com/docs/functions/sandbox)**


Untuk opsi self-hosted (Docker, gVisor, Firecracker) dan konfigurasi isolasi terperinci, lihat [Isolation Technologies](https://code.claude.com/docs/id/agent-sdk/secure-deployment#isolation-technologies).


## [​](https://code.claude.com/docs/id/agent-sdk/hosting#pola-penerapan-produksi) Pola Penerapan Produksi


### [​](https://code.claude.com/docs/id/agent-sdk/hosting#pola-1-sesi-ephemeral) Pola 1: Sesi Ephemeral


Buat kontainer baru untuk setiap tugas pengguna, kemudian hancurkan saat selesai.
Terbaik untuk tugas sekali jalan, pengguna mungkin masih berinteraksi dengan AI saat tugas sedang diselesaikan, tetapi setelah selesai kontainer dihancurkan.
**Contoh:**


- Bug Investigation & Fix: Debug dan selesaikan masalah spesifik dengan konteks yang relevan
- Invoice Processing: Ekstrak dan struktur data dari kwitansi/faktur untuk sistem akuntansi
- Translation Tasks: Terjemahkan dokumen atau batch konten antar bahasa
- Image/Video Processing: Terapkan transformasi, optimasi, atau ekstrak metadata dari file media


### [​](https://code.claude.com/docs/id/agent-sdk/hosting#pola-2-sesi-berjalan-lama) Pola 2: Sesi Berjalan Lama


Pertahankan instans kontainer persisten untuk tugas yang berjalan lama. Sering kali menjalankan **beberapa** proses Claude Agent di dalam kontainer berdasarkan permintaan.
Terbaik untuk agen proaktif yang mengambil tindakan tanpa masukan pengguna, agen yang melayani konten atau agen yang memproses jumlah pesan yang tinggi.
**Contoh:**


- Email Agent: Memantau email masuk dan secara otonom melakukan triase, merespons, atau mengambil tindakan berdasarkan konten
- Site Builder: Menghosting situs web khusus per pengguna dengan kemampuan pengeditan langsung yang disajikan melalui port kontainer
- High-Frequency Chat Bots: Menangani aliran pesan berkelanjutan dari platform seperti Slack di mana waktu respons cepat sangat penting


### [​](https://code.claude.com/docs/id/agent-sdk/hosting#pola-3-sesi-hybrid) Pola 3: Sesi Hybrid


Kontainer ephemeral yang dihidrasi dengan riwayat dan status, mungkin dari database atau dari fitur resumption sesi SDK.
Terbaik untuk kontainer dengan interaksi intermiten dari pengguna yang memulai pekerjaan dan berhenti saat pekerjaan selesai tetapi dapat dilanjutkan.
**Contoh:**


- Personal Project Manager: Membantu mengelola proyek berkelanjutan dengan check-in intermiten, mempertahankan konteks tugas, keputusan, dan kemajuan
- Deep Research: Melakukan tugas penelitian multi-jam, menyimpan temuan dan melanjutkan investigasi saat pengguna kembali
- Customer Support Agent: Menangani tiket dukungan yang mencakup beberapa interaksi, memuat riwayat tiket dan konteks pelanggan


### [​](https://code.claude.com/docs/id/agent-sdk/hosting#pola-4-kontainer-tunggal) Pola 4: Kontainer Tunggal


Jalankan beberapa proses Claude Agent SDK dalam satu kontainer global.
Terbaik untuk agen yang harus berkolaborasi erat satu sama lain. Ini mungkin pola yang paling tidak populer karena Anda harus mencegah agen dari menimpa satu sama lain.
**Contoh:**


- **Simulations**: Agen yang berinteraksi satu sama lain dalam simulasi seperti video game.


## [​](https://code.claude.com/docs/id/agent-sdk/hosting#faq) FAQ


### [​](https://code.claude.com/docs/id/agent-sdk/hosting#bagaimana-cara-saya-berkomunikasi-dengan-sandbox-saya) Bagaimana cara saya berkomunikasi dengan sandbox saya?


Saat menghosting di kontainer, buka port untuk berkomunikasi dengan instans SDK Anda. Aplikasi Anda dapat membuka endpoint HTTP/WebSocket untuk klien eksternal sementara SDK berjalan secara internal dalam kontainer.


### [​](https://code.claude.com/docs/id/agent-sdk/hosting#berapa-biaya-hosting-kontainer) Berapa biaya hosting kontainer?


Biaya dominan melayani agen adalah token; kontainer bervariasi berdasarkan apa yang Anda sediakan, tetapi biaya minimum kira-kira 5 sen per jam berjalan.


### [​](https://code.claude.com/docs/id/agent-sdk/hosting#kapan-saya-harus-mematikan-kontainer-idle-vs-menjaganya-tetap-hangat) Kapan saya harus mematikan kontainer idle vs. menjaganya tetap hangat?


Ini mungkin tergantung penyedia, penyedia sandbox yang berbeda akan membiarkan Anda menetapkan kriteria berbeda untuk timeout idle setelah itu sandbox mungkin berhenti.
Anda akan ingin menyesuaikan timeout ini berdasarkan seberapa sering Anda pikir respons pengguna mungkin terjadi.


### [​](https://code.claude.com/docs/id/agent-sdk/hosting#seberapa-sering-saya-harus-memperbarui-claude-code-cli) Seberapa sering saya harus memperbarui Claude Code CLI?


Claude Code CLI diberi versi dengan semver, jadi perubahan breaking apa pun akan diberi versi.


### [​](https://code.claude.com/docs/id/agent-sdk/hosting#bagaimana-cara-saya-memantau-kesehatan-kontainer-dan-kinerja-agen) Bagaimana cara saya memantau kesehatan kontainer dan kinerja agen?


Karena kontainer hanyalah server, infrastruktur logging yang sama yang Anda gunakan untuk backend akan bekerja untuk kontainer.


### [​](https://code.claude.com/docs/id/agent-sdk/hosting#berapa-lama-sesi-agen-dapat-berjalan-sebelum-timeout) Berapa lama sesi agen dapat berjalan sebelum timeout?


Sesi agen tidak akan timeout, tetapi pertimbangkan untuk menetapkan properti ‘maxTurns’ untuk mencegah Claude terjebak dalam loop.


## [​](https://code.claude.com/docs/id/agent-sdk/hosting#langkah-berikutnya) Langkah Berikutnya


- [Secure Deployment](https://code.claude.com/docs/id/agent-sdk/secure-deployment) - Kontrol jaringan, manajemen kredensial, dan pengerasan isolasi
- [TypeScript SDK - Sandbox Settings](https://code.claude.com/docs/id/agent-sdk/typescript#sandbox-settings) - Konfigurasi sandbox secara terprogram
- [Sessions Guide](https://code.claude.com/docs/id/agent-sdk/sessions) - Pelajari tentang manajemen sesi
- [Permissions](https://code.claude.com/docs/id/agent-sdk/permissions) - Konfigurasi izin alat
- [Cost Tracking](https://code.claude.com/docs/id/agent-sdk/cost-tracking) - Pantau penggunaan API
- [MCP Integration](https://code.claude.com/docs/id/agent-sdk/mcp) - Perluas dengan alat khusus[Claude Code Docs home page](https://code.claude.com/docs/id/overview)

[Privacy choices](https://code.claude.com/docs/id/agent-sdk/hosting#)

