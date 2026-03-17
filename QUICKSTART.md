# Quick Start

## Build

```bash
cargo build --release
```

Pakai binary:

```bash
./target/release/prodex
```

## Import login `codex` yang sekarang

Kalau `~/.codex` kamu sudah berisi login aktif:

```bash
./target/release/prodex profile import-current main
```

## Atau bikin profile baru lalu login

```bash
./target/release/prodex profile add second
./target/release/prodex login --profile second
```

## Cek semua profile dan quota

```bash
./target/release/prodex profile list
./target/release/prodex quota --all
```

## Pilih profile aktif

```bash
./target/release/prodex use main
./target/release/prodex current
```

## Jalankan `codex` lewat `prodex`

```bash
./target/release/prodex run
```

Contoh:

```bash
./target/release/prodex run -- --version
./target/release/prodex run exec "review this repo"
./target/release/prodex run --profile second
```

## Debug environment

```bash
./target/release/prodex doctor
./target/release/prodex doctor --quota
```

## Catatan

- `prodex` hanya wrapper; login tetap dikerjakan oleh `codex`
- quota via `cq` hanya jalan untuk profile dengan auth ChatGPT
- `prodex run` akan melakukan quota preflight kecuali kamu pakai `--skip-quota-check`
