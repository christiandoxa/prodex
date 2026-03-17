# prodex

`prodex` adalah wrapper CLI untuk `codex` yang memisahkan banyak profile lewat `CODEX_HOME` terpisah.

Butuh versi singkatnya saja: lihat [QUICKSTART.md](./QUICKSTART.md).

Intinya:

- satu profile `prodex` = satu folder `CODEX_HOME`
- login tetap dijalankan oleh `codex`
- `prodex` mengelola profile, active profile, quota check via `cq`, dan launcher ke `codex`

Model mentalnya mirip profile di browser, tapi untuk `codex`.

## Quick Start

### 1. Build

```bash
cargo build --release
```

Binary akan ada di:

```bash
./target/release/prodex
```

### 2. Import profile `codex` yang sekarang

Kalau kamu sudah punya login aktif di `~/.codex`:

```bash
./target/release/prodex profile import-current main
```

Ini akan:

- copy `~/.codex` ke managed profile baru
- menyimpan profile di `~/.prodex/profiles/main`
- menjadikan `main` sebagai active profile

### 3. Tambah profile baru dan login

Kalau mau profile baru yang kosong:

```bash
./target/release/prodex profile add second
./target/release/prodex login --profile second
```

`prodex login` tidak meng-handle OAuth callback sendiri. Ia hanya menjalankan `codex login` dengan `CODEX_HOME` profile yang dipilih.

### 4. Lihat semua quota

```bash
./target/release/prodex quota --all
```

Contoh kolom `MAIN`:

```text
5h 37/100 used | weekly 12/100 used
```

### 5. Pilih profile aktif dan jalankan `codex`

```bash
./target/release/prodex use main
./target/release/prodex run
```

Atau jalankan langsung dengan profile tertentu:

```bash
./target/release/prodex run --profile second
```

## Requirements

`prodex` mengandalkan binary berikut:

- `codex`
- `cq`

Cek cepat:

```bash
codex --help
cq --help
```

Kalau mau audit environment `prodex`:

```bash
./target/release/prodex doctor
./target/release/prodex doctor --quota
```

## Cara Kerja

`prodex` menyimpan state sendiri di:

```text
~/.prodex
```

Struktur utamanya:

- `state.json`: daftar profile dan active profile
- `profiles/<name>`: managed `CODEX_HOME` per profile

Auth tetap disimpan oleh `codex` di dalam `auth.json` milik masing-masing profile.

## Command yang Paling Sering Dipakai

### Profile management

Tambah profile kosong:

```bash
./target/release/prodex profile add work
```

Import dari `~/.codex`:

```bash
./target/release/prodex profile import-current work
```

Daftar semua profile:

```bash
./target/release/prodex profile list
```

Pilih active profile:

```bash
./target/release/prodex use work
```

Hapus profile:

```bash
./target/release/prodex profile remove work
```

Hapus profile sekaligus managed home-nya:

```bash
./target/release/prodex profile remove work --delete-home
```

### Login/logout

Login ke profile tertentu:

```bash
./target/release/prodex login --profile work
```

Logout profile tertentu:

```bash
./target/release/prodex logout --profile work
```

### Quota

Render quota satu profile:

```bash
./target/release/prodex quota --profile work
```

Render raw JSON quota:

```bash
./target/release/prodex quota --profile work --raw
```

Lihat semua profile sekaligus:

```bash
./target/release/prodex quota --all
```

### Run `codex`

Jalankan `codex` dengan active profile:

```bash
./target/release/prodex run
```

Jalankan `codex` dengan argumen:

```bash
./target/release/prodex run -- --version
./target/release/prodex run exec "review this repo"
```

Jalankan dengan profile spesifik:

```bash
./target/release/prodex run --profile work
```

Lewati quota preflight:

```bash
./target/release/prodex run --profile work --skip-quota-check
```

## Quota Behavior

Sebelum `prodex run` menjalankan `codex`, `prodex` akan coba check quota profile yang dipakai.

Kalau profile itu kelihatan sedang kena limit:

- `prodex` akan block eksekusi
- menampilkan limit yang sedang penuh
- memberi saran profile lain yang terlihat siap, kalau ada

`prodex` tidak melakukan auto-switch profile.

## Catatan Penting

- `quota --all` dan quota preflight bergantung pada `cq`
- quota ChatGPT hanya bisa dibaca kalau profile itu login dengan mode ChatGPT, bukan API key
- kalau auth profile adalah API key, `quota --all` akan tampil `error` untuk profile itu
- `prodex` tidak menggantikan `codex`; dia hanya menjadi launcher dan profile manager

## Environment Variables

Override lokasi state `prodex`:

```bash
PRODEX_HOME=/path/to/prodex-home
```

Override binary `codex`:

```bash
PRODEX_CODEX_BIN=/path/to/codex
```

Override binary `cq`:

```bash
PRODEX_CQ_BIN=/path/to/cq
```

## Development

Run saat development:

```bash
cargo run -- profile list
cargo run -- quota --all
cargo run -- doctor
```

Test:

```bash
cargo test
```
