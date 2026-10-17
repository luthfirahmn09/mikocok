# mikocok 🍜

**Pengganti Numbers & Pages, buat yang cuma mau ngedit file kantor doang.**

Pernah nggak, lagi buru-buru, klik dua kali `laporan.xlsx`, terus yang kebuka… **Numbers**? Atau `kontrak.docx` kebuka di **Pages**, terus layout-nya geser seribu piksel dan tabelnya pindah alam?

Nah. Itu dia alasan *mikocok* ada.

Dua aplikasi Mac mungil, sama-sama ±5 MB (iya, MB, bukan GB, cek lagi aja):

| App | Gantiin siapa | Urusannya |
|---|---|---|
| **mikocokWord** | Pages (yang nyasar buka `.docx`) | Buka & edit `.docx` |
| **mikocokExcel** | Numbers (yang nyasar buka `.xlsx`) | Buka & edit `.xlsx` |

Nggak perlu Office. Nggak perlu langganan. Nggak perlu login. Nggak ada pop-up "Coba gratis 30 hari". Nggak ada AI yang nawarin "bantu rangkum dokumen ini?" padahal kamu cuma mau benerin typo.

Ditulis pakai Rust + [egui](https://github.com/emilk/egui). Dua-duanya sebenarnya **satu binary yang sama** pakai dua baju beda. Hemat, kan? Ala anak kos.

---

## mikocokWord: tampil kayak Word, bukan kayak "kata-kata mutiara"

Dokumen tampil **per halaman**, beneran kertas putih dengan ukuran dan margin sesuai file aslinya, bukan satu scroll panjang tanpa akhir kayak thread Twitter.

- **Halaman asli**: pindah halaman otomatis, page break manual dihormati, ada penghitung halaman, bisa zoom.
- **Format kebaca semua**: font, ukuran, tebal, miring, garis bawah, coret, warna, highlight, heading, rata kiri-kanan, spasi, indentasi.
- **Daftar, tabel, gambar**: bullet, penomoran, tabel dengan border dan warna sel, gambar PNG/JPEG.
- **Ngetik langsung di tempat**: klik, ketik, selesai. Nggak ada mode "edit" yang harus dibuka dulu.
  - `Enter` bikin paragraf baru (formatnya ikut diwarisin, kayak keturunan).
  - `Backspace` di awal paragraf = gabung ke paragraf atas.
  - Panah atas/bawah pindah antar paragraf.
  - Teks yang kamu ubah **tetap tebal/miring/berwarna** sesuai aslinya.

## mikocokExcel: grid, rumus, tanpa drama

Tabel sel `.xlsx` dengan tab sheet, bar rumus, dan edit langsung di sel. Ketik `=...` jadi rumus, ketik angka jadi angka, ketik curhatan jadi teks. Sesederhana itu.

---

## File kamu aman (ini serius, bagian paling penting)

mikocok **nggak** "buka terus export ulang semuanya" lalu hasilnya jadi dokumen yang mirip tapi bukan. Dia ngedit XML di dalam file dan **cuma nulis ulang bagian yang kamu ubah**. Style, gambar, chart, header/footer, komentar, dan apa pun yang dia nggak ngerti, dibiarin utuh. Prinsipnya: *"nggak ngerti? jangan disentuh."* Filosofi hidup yang bagus juga.

Pas nyimpan, dia nulis ke file sementara dulu. Kalau gagal di tengah jalan, file aslimu nggak ikut kepotong.

---

## Pasang

Butuh macOS dan [Rust](https://rustup.rs).

```sh
git clone https://github.com/luthfirahmn09/mikocok.git
cd mikocok
./build_app.sh            # build & pasang ke ~/Applications
./build_app.sh --default  # sekalian jadiin default buat .docx / .xlsx
```

Habis itu: klik kanan file → **Buka Dengan → mikocokWord / mikocokExcel**, atau klik dua kali kalau udah dijadiin default, atau seret file ke jendelanya.

Kalau macOS rewel di peluncuran pertama (aplikasinya cuma di-sign ad-hoc, bukan sama Apple), klik kanan → Open sekali aja. Habis itu damai.

Pintasan: `⌘O` buka, `⌘S` simpan, `⇧⌘S` simpan sebagai.

---

## Jujur-jujuran: yang belum ada

Ini editor **minimalis**, bukan pembunuh Microsoft Office. Jangan berharap dia bisa bikin pivot table sambil nyeduhin kopi.

- **Word**: belum ada toolbar tebal/miring (format yang udah ada tampil dan terjaga, tapi kamu belum bisa nambah format baru). Belum ada header/footer, kolom, text box, atau track changes. `Shift+Enter` belum jalan. Nomor daftar setelah item baru baru benar setelah file dibuka ulang.
- **Pembagian halaman** dihitung sama layout mikocok sendiri, jadi bisa beda tipis dari Word: paragraf nggak dipotong antar halaman, dan font yang nggak ada di Mac-mu (misal Calibri tanpa Office) diganti Arial.
- **Excel**: belum ada chart, format angka/tanggal, tampilan sel gabungan, atau penghitungan rumus. Sel rumus nampilin nilai terakhir yang tersimpan, dan Excel bakal ngitung ulang pas file dibuka di sana.
- **Gambar**: baru PNG dan JPEG.

Kalau butuh yang lebih dari ini, ya… ada Office. Dia nggak ke mana-mana.

---

## Buat yang mau ngoprek

```sh
python3 tools/make_samples.py   # bikin file contoh .docx/.xlsx buat test
cargo test
cargo run --release -- path/ke/file.docx
```

| File | Isinya |
|---|---|
| `src/docx.rs` | Model Word: style, numbering, tabel, parser, dan penulis ulang XML (edit / pecah / gabung paragraf) |
| `src/docview.rs` | Render halaman, pagination, edit langsung pakai egui |
| `src/xlsx.rs` | Pembaca Excel dan penulis ulang XML per sel |
| `src/ooxml.rs` | Urusan zip dan XML |
| `src/fonts.rs` | Ngambil font sistem biar dokumen pakai font yang Word pakai |
| `build_app.sh` | Bikin dua `.app` lengkap dengan "Buka Dengan" |

Ada unit test buat parsing, round-trip edit, sama layout halaman tanpa layar. Kemiripan visual dengan Word asli itu *best effort*. Nemu dokumen yang tampilannya aneh? Buka issue dan lampirin filenya, aku seneng diajak debat sama dokumen jelek.

---

*mikocok: karena kadang kamu cuma mau benerin satu kata di dokumen, bukan ikut kelas onboarding.*
