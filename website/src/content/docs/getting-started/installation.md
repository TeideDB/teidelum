---
title: Installation
description: Build Teidelum from source
---

## Prerequisites

- **Rust toolchain** (1.75+) — install via [rustup](https://rustup.rs/):
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```
- **C compiler** — required by teide (the columnar engine). On macOS, Xcode CLT (`xcode-select --install`). On Linux, `build-essential`.
- **Git** — to clone the repository.

## Build from Source

```bash
git clone https://github.com/TeideDB/teidelum.git
cd teidelum
cargo build --release
```

The binary is at `./target/release/teidelum`.

## Verify Installation

```bash
./target/release/teidelum --help
```

Or run the test suite:

```bash
cargo test
```

## Development Build

For faster iteration during development:

```bash
cargo build    # debug build (faster compile, slower runtime)
cargo run      # build and run
cargo check    # type-check only (fastest feedback)
```
