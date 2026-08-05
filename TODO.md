# TODO.md - แผนการพัฒนา blnk-rust

# Milestone 0: CI/CD & Git Workflow

## 0.1 สร้าง .github/workflows/ci.yml
**ทำไป:** ตั้งค่า CI pipeline ให้ run ทุกครั้ง push/PR
**ทำอย่างไร:**
- สร้าง `.github/workflows/ci.yml`
- jobs:
  - `lint` - run `cargo fmt --check` + `cargo clippy`
  - `test` - run `cargo test --all`
  - `audit` - run `cargo audit`
  - `build-linux` - build release binary on Linux (x86_64, aarch64)
  - `build-windows` - build release binary on Windows (x86_64)
  - `build-android` - build for Android (armv7, aarch64) ใช้ `cargo-ndk`
  - ข้าม macOS
- trigger: push to main/develop, PR
- matrix:
  - Linux: ubuntu-latest (x86_64, aarch64)
  - Windows: windows-latest (x86_64)
  - Android: ubuntu-latest ด้วย NDK
- ทุก job ต้อง pass ก่อน merge
**ได้อะไร:** `.github/workflows/ci.yml` ที่ complete
**เสร็จเมื่อไร:** Push ไป repo แล้ว CI run ได้

### ci.yml content:
```yaml
name: CI

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main, develop]

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1

jobs:
  fmt:
    name: Format Check
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt
      - run: cargo fmt --all -- --check

  clippy:
    name: Clippy Lint
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - run: cargo clippy --all --all-targets -- -D warnings

  test:
    name: Tests
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test --all --verbose

  audit:
    name: Security Audit
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: rustsec/audit-check-action@v1
        with:
          token: ${{ secrets.GITHUB_TOKEN }}

  build-linux:
    name: Build Linux
    runs-on: ubuntu-latest
    strategy:
      matrix:
        target: [x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - run: cargo build --release --target ${{ matrix.target }}
      - uses: actions/upload-artifact@v3
        with:
          name: blnk-linux-${{ matrix.target }}
          path: target/${{ matrix.target }}/release/blnk

  build-windows:
    name: Build Windows
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: x86_64-pc-windows-msvc
      - run: cargo build --release --target x86_64-pc-windows-msvc
      - uses: actions/upload-artifact@v3
        with:
          name: blnk-windows-x86_64
          path: target/x86_64-pc-windows-msvc/release/blnk.exe

  build-android:
    name: Build Android
    runs-on: ubuntu-latest
    strategy:
      matrix:
        target: [armv7-linux-androideabi, aarch64-linux-android]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - uses: ndk-build/ndk-build@v1.2.0
        with:
          ndk-version: r25c
      - run: cargo install cargo-ndk
      - run: cargo ndk -t ${{ matrix.target }} build --release
      - uses: actions/upload-artifact@v3
        with:
          name: blnk-android-${{ matrix.target }}
          path: target/${{ matrix.target }}/release/libblnk.so
```

**เสร็จเมื่อไร:** CI run ผ่าน ทุก job

---

## 0.2 สร้าง .github/workflows/release.yml
**ทำไป:** ตั้งค่า release pipeline
**ทำอย่างไร:**
- สร้าง `.github/workflows/release.yml`
- trigger: tag push (v*.*.*)
- build ทุก platform
- create GitHub release
- upload binaries + checksums
- sign binaries (optional)
**ได้อะไร:** `.github/workflows/release.yml` ที่ complete
**เสร็จเมื่อไร:** Tag push แล้ว release ถูกสร้าง

### release.yml content:
```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

env:
  CARGO_TERM_COLOR: always

jobs:
  create-release:
    name: Create Release
    runs-on: ubuntu-latest
    outputs:
      upload_url: ${{ steps.create_release.outputs.upload_url }}
    steps:
      - uses: actions/create-release@v1
        id: create_release
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          tag_name: ${{ github.ref }}
          release_name: Release ${{ github.ref }}
          draft: false
          prerelease: false

  build-and-upload:
    name: Build ${{ matrix.target }}
    needs: create-release
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            artifact: blnk
          - os: ubuntu-latest
            target: aarch64-unknown-linux-gnu
            artifact: blnk
          - os: windows-latest
            target: x86_64-pc-windows-msvc
            artifact: blnk.exe
          - os: ubuntu-latest
            target: armv7-linux-androideabi
            artifact: libblnk.so
          - os: ubuntu-latest
            target: aarch64-linux-android
            artifact: libblnk.so
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - name: Build
        run: cargo build --release --target ${{ matrix.target }}
      - name: Create checksum
        run: |
          cd target/${{ matrix.target }}/release
          sha256sum ${{ matrix.artifact }} > ${{ matrix.artifact }}.sha256
      - name: Upload binary
        uses: actions/upload-release-asset@v1
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          upload_url: ${{ needs.create-release.outputs.upload_url }}
          asset_path: ./target/${{ matrix.target }}/release/${{ matrix.artifact }}
          asset_name: blnk-${{ matrix.target }}-${{ github.ref_name }}
          asset_content_type: application/octet-stream
      - name: Upload checksum
        uses: actions/upload-release-asset@v1
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          upload_url: ${{ needs.create-release.outputs.upload_url }}
          asset_path: ./target/${{ matrix.target }}/release/${{ matrix.artifact }}.sha256
          asset_name: blnk-${{ matrix.target }}-${{ github.ref_name }}.sha256
          asset_content_type: text/plain
```

**เสร็จเมื่อไร:** Tag push แล้ว release artifacts upload ได้

---

## 0.3 สร้าง CONTRIBUTING.md
**ทำไป:** ให้ guide สำหรับ contributors
**ทำอย่างไร:**
- สร้าง `CONTRIBUTING.md` ที่ root
- sections:
  - How to Report Issues
  - How to Submit PRs
  - Commit Message Format
  - Code Style
  - Testing Requirements
  - Branch Strategy

### CONTRIBUTING.md content:
```markdown
# Contributing to blnk

## Commit Message Format

ทุก commit ต้องใช้ format นี้:

\`\`\`
<type>(<scope>): <subject>

<body>

<footer>
\`\`\`

### Type
- **feat**: ฟีเจอร์ใหม่
- **fix**: บัก fix
- **docs**: เอกสาร
- **style**: formatting, missing semicolons, etc
- **refactor**: โค้ด refactor ไม่เปลี่ยน behavior
- **perf**: performance improvement
- **test**: เพิ่ม/แก้ tests
- **chore**: build, deps, CI config
- **ci**: CI/CD changes

### Scope
- `cli` - CLI commands
- `config` - Configuration system
- `signaling` - Signaling client
- `peer` - WebRTC peer connection
- `session` - Session management
- `stream` - Stream handlers
- `protocol` - Protocol definitions
- `identity` - Identity management
- `web` - Web server
- `shell` - Shell handler
- `file` - File handler
- `proxy` - HTTP proxy handler
- `tcp` - TCP forwarding handler
- `websocket` - WebSocket handler

### Subject
- ใช้ imperative mood ("add" ไม่ใช่ "added" หรือ "adds")
- ไม่ใช้ capital letter ที่ต้น
- ไม่มี period (.) ที่ท้าย
- สั้นไม่เกิน 50 characters

### Body
- อธิบาย **what** และ **why** ไม่ใช่ **how**
- wrap ที่ 72 characters
- ใช้ imperative mood

### Footer
- ถ้า commit fix issue: `Fixes #123`
- ถ้า commit related ไปยัง issue: `Related to #456`
- ถ้า breaking change: `BREAKING CHANGE: description`

### Examples

\`\`\`
feat(cli): add connect command with pairing code

Add new 'connect' subcommand that allows users to connect to peer devices
using a pairing code. The command generates a random 6-digit code and waits
for peer confirmation before establishing WebRTC connection.

Fixes #42
\`\`\`

\`\`\`
fix(stream): handle incomplete frames in parser

Frame parser now correctly handles incomplete frames by buffering them
until complete frame is received. Prevents crash when receiving fragmented
data over slow connections.

Related to #89
\`\`\`

\`\`\`
refactor(protocol): simplify frame builder API

Remove unnecessary builder methods and consolidate frame creation logic.
No behavior change, just cleaner API.
\`\`\`

## Issue Labels

ทุก issue ต้องมี label:
- `bug` - บัก
- `feature` - ฟีเจอร์ใหม่
- `enhancement` - ปรับปรุง existing feature
- `documentation` - เอกสาร
- `good first issue` - เหมาะสำหรับ beginner
- `help wanted` - ต้องการความช่วยเหลือ
- `wontfix` - ไม่ fix
- `duplicate` - duplicate issue
- `priority-high` - ต้องทำเลย
- `priority-medium` - ปกติ
- `priority-low` - ทำได้ทีหลัง
- `platform-linux` - Linux specific
- `platform-windows` - Windows specific
- `platform-android` - Android specific

## Branch Strategy

- `main` - production ready code
- `develop` - development branch
- `feature/<name>` - feature branches
- `fix/<name>` - bugfix branches
- `docs/<name>` - documentation branches

## PR Requirements

ทุก PR ต้อง:
- ผ่าน CI (fmt, clippy, test, audit)
- มี description ชัดเจน
- link ไปยัง related issues
- มี label
- ได้ review จาก maintainer อย่างน้อย 1 คน

## Code Style

- ใช้ `cargo fmt` ให้ format ถูก
- ใช้ `cargo clippy` ตรวจ warnings
- ใช้ meaningful variable names
- เขียน comments สำหรับ complex logic
- ใช้ doc comments สำหรับ public APIs

## Testing

- ทุก feature ต้องมี tests
- ทุก bugfix ต้องมี test ที่ fail ก่อน fix
- ทุก test ต้อง pass ก่อน merge
- ต้อง test cross-platform (Linux, Windows, Android)

## Running Tests Locally

\`\`\`bash
# Format check
cargo fmt --all -- --check

# Lint
cargo clippy --all --all-targets -- -D warnings

# Tests
cargo test --all

# Security audit
cargo audit

# Build release
cargo build --release
\`\`\`
```

**เสร็จเมื่อไร:** CONTRIBUTING.md ครบ

---

## 0.4 สร้าง .github/ISSUE_TEMPLATE/
**ทำไป:** ให้ template สำหรับ issue
**ทำอย่างไร:**
- สร้าง `.github/ISSUE_TEMPLATE/bug_report.yml`
- สร้าง `.github/ISSUE_TEMPLATE/feature_request.yml`
- สร้าง `.github/ISSUE_TEMPLATE/config.yml`


### config.yml:
```yaml
blank_issues_enabled: false
contact_links:
  - name: Discussion
    url: https://github.com/blnk-bot/blnk/discussions
    about: ถามคำถาม
```

**เสร็จเมื่อไร:** Issue templates ครบ

---

## 0.5 สร้าง .github/pull_request_template.md
**ทำไป:** ให้ template สำหรับ PR
**ทำอย่างไร:**
- สร้าง `.github/pull_request_template.md`

### pull_request_template.md:
```markdown
## Description
PR นี้ทำอะไร?

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation

## Related Issues
Fixes #(issue number)
Related to #(issue number)

## Testing
- [ ] ผ่าน `cargo fmt --all -- --check`
- [ ] ผ่าน `cargo clippy --all --all-targets -- -D warnings`
- [ ] ผ่าน `cargo test --all`
- [ ] ผ่าน `cargo audit`
- [ ] ทดสอบ cross-platform (Linux, Windows, Android)

## Checklist
- [ ] Code follows style guidelines
- [ ] Comments added for complex logic
- [ ] Documentation updated
- [ ] Tests added/updated
- [ ] No new warnings generated
- [ ] Dependent changes merged and published

## Screenshots (if applicable)
```

**เสร็จเมื่อไร:** PR template ครบ

---

## 0.6 สร้าง .gitignore
**ทำไป:** ไม่ commit ไฟล์ที่ไม่ควร
**ทำอย่างไร:**
- สร้าง `.gitignore`

### .gitignore:
```
# Rust
/target/
Cargo.lock
**/*.rs.bk
*.pdb

# IDE
.vscode/
.idea/
*.swp
*.swo
*~
.DS_Store

# OS
Thumbs.db
.DS_Store

# Local config
config.toml
.env
.env.local

# Build artifacts
*.so
*.dylib
*.dll
*.exe

# Test coverage
*.profraw
*.profdata
/coverage/

# Benchmarks
/benches/target/
```

**เสร็จเมื่อไร:** .gitignore ครบ

---

## 0.7 สร้าง Makefile (optional)

**ทำไป:** ให้ shortcuts สำหรับ common tasks
**ทำอย่างไร:**
- สร้าง `Makefile`

### Makefile:

```makefile
.PHONY: help fmt lint test audit build build-release clean

help:
	@echo "Available commands:"
	@echo "  make fmt          - Format code"
	@echo "  make lint         - Run clippy linter"
	@echo "  make test         - Run tests"
	@echo "  make audit        - Security audit"
	@echo "  make build        - Build debug binary"
	@echo "  make build-release - Build release binary"
	@echo "  make clean        - Clean build artifacts"
	@echo "  make ci           - Run all CI checks"

fmt:
	cargo fmt --all

lint:
	cargo clippy --all --all-targets -- -D warnings

test:
	cargo test --all

audit:
	cargo audit

build:
	cargo build

build-release:
	cargo build --release

clean:
	cargo clean

ci: fmt lint test audit
	@echo "All CI checks passed!"
```

**เสร็จเมื่อไร:** Makefile ครบ

---

# สถานะ: ต้องทำเลยตั้งแต่ต้น

ทุก item ใน Milestone 0 ต้องทำก่อนเริ่ม Milestone 1


## Milestone 1: ตั้งค่าโปรเจกต์และ Build Foundation

### 1.1 สร้าง Cargo.toml
**ทำไป:** ตั้งค่า metadata และ dependencies ของโปรเจกต์
**ทำอย่างไร:**
- ชื่อ package: `blnk`
- edition: `2021`
- version: `0.1.0-alpha`
- dependencies:
  - `tokio` (async runtime) - features: full
  - `clap` (CLI parsing) - features: derive
  - `tracing` + `tracing-subscriber` (logging)
  - `thiserror` (error handling)
  - `anyhow` (context errors)
  - `serde` + `serde_json` (serialization)
  - `config` (config management)
  - `prost` + `prost-build` (protobuf)
  - `webrtc` (WebRTC)
  - `tokio-tungstenite` (WebSocket)
  - `axum` (web server)
  - `tower` (middleware)
  - `nix` (Unix PTY)
  - `winapi` (Windows API)
  - `uuid` (unique IDs)
  - `chrono` (timestamps)
  - `rsa` (RSA crypto)
  - `sha2` (hashing)
**ได้อะไร:** `Cargo.toml` พร้อมใช้
**เสร็จเมื่อไร:** `cargo build` ผ่านโดยไม่มี error

### 1.2 สร้างโครงสร้าง src/ ตามแบบ architecture
**ทำอย่างไร:**
- สร้าง `src/main.rs` (entry point)
- สร้าง `src/lib.rs` (module registry)
- สร้าง `src/config/mod.rs` + `src/config/args.rs`
- สร้าง `src/signaling/mod.rs` + `src/signaling/client.rs` + `src/signaling/messages.rs`
- สร้าง `src/peer/mod.rs` + `src/peer/connection.rs` + `src/peer/ice.rs`
- สร้าง `src/session/mod.rs` + `src/session/session.rs` + `src/session/auth.rs`
- สร้าง `src/stream/mod.rs` + `src/stream/handler.rs` + `src/stream/shell.rs` + `src/stream/file.rs` + `src/stream/proxy.rs` + `src/stream/tcp.rs` + `src/stream/websocket.rs`
- สร้าง `src/protocol/mod.rs` + `src/protocol/swsp.rs` + `src/protocol/pairing.rs`
- สร้าง `src/identity/mod.rs` + `src/identity/key.rs`
- สร้าง `src/fileshare/mod.rs` + `src/fileshare/share.rs`
- สร้าง `src/shell/mod.rs` + `src/shell/pty.rs` + `src/shell/terminal.rs`
- สร้าง `src/proxy/mod.rs` + `src/proxy/http.rs`
- สร้าง `src/tcpforward/mod.rs` + `src/tcpforward/forward.rs`
- สร้าง `src/utils/mod.rs` + `src/utils/qr.rs` + `src/utils/logging.rs` + `src/utils/error.rs`
- สร้าง `src/web/mod.rs`
**ได้อะไร:** โครงสร้าง module ครบตามแบบ architecture
**เสร็จเมื่อไร:** `cargo check` ผ่านโดยไม่มี missing module error

### 1.3 สร้าง build.rs สำหรับ protobuf compilation
**ทำไป:** ตั้งค่า code generation จาก `.proto`
**ทำอย่างไร:**
- สร้าง `build.rs` ที่ root
- ใช้ `prost_build::Config::new()`
- ตั้ง proto path เป็น `proto/`
- compile ทุกไฟล์ `.proto` ที่พบ
**ได้อะไร:** `build.rs` ที่ generate protobuf code ตอน build
**เสร็จเมื่อไร:** `cargo build` สร้าง `.rs` ใน `OUT_DIR` ได้

---

## Milestone 2: Error Handling และ Logging

### 2.1 สร้าง error type ใน src/utils/error.rs
**ทำไป:** นิยาม error variants ทั้งหมดที่ใช้ในโปรเจกต์
**ทำอย่างไร:**
- สร้าง `AppError` enum ด้วย `#[derive(thiserror::Error)]`
- variants:
  - `#[error("IO: {0}")] Io(#[from] std::io::Error)`
  - `#[error("Config: {0}")] Config(String)`
  - `#[error("Parse: {0}")] Parse(String)`
  - `#[error("Protocol: {0}")] Protocol(String)`
  - `#[error("Network: {0}")] Network(String)`
  - `#[error("Auth: {0}")] Auth(String)`
  - `#[error("Signaling: {0}")] Signaling(String)`
  - `#[error("WebRTC: {0}")] WebRTC(String)`
  - `#[error("Stream: {0}")] Stream(String)`
  - `#[error("Crypto: {0}")] Crypto(String)`
  - `#[error("{0}")] Other(String)`
- สร้าง type alias: `pub type AppResult<T> = Result<T, AppError>;`
**ได้อะไร:** Error type ที่ใช้ทั่วโปรเจกต์
**เสร็จเมื่อไร:** ใช้ `AppResult<T>` ในฟังก์ชันได้

### 2.2 ตั้งค่า tracing logging ใน src/utils/logging.rs
**ทำไป:** เตรียมระบบ logging
**ทำอย่างไร:**
- สร้างฟังก์ชัน `pub fn init_logging(level: &str) -> AppResult<()>`
- ใช้ `tracing_subscriber::fmt()`
- ตั้ง default level เป็น `INFO`
- อนุญาต override ด้วย `RUST_LOG` env var
- format output ให้อ่านง่าย
- ใน `src/main.rs` เรียก `logging::init_logging()` ก่อนอื่น
**ได้อะไร:** Logging system ที่ทำงาน
**เสร็จเมื่อไร:** `info!()`, `warn!()`, `error!()` macros ทำงาน

---

## Milestone 3: Configuration System

### 3.1 สร้าง AppConfig struct ใน src/config/mod.rs
**ทำไป:** นิยาม configuration data types
**ทำอย่างไร:**
- สร้าง `AppConfig` struct:
  - `host: String` (default: `127.0.0.1`)
  - `port: u16` (default: `8080`)
  - `log_level: String` (default: `info`)
  - `data_dir: String` (default: `~/.blnk`)
  - `signaling_url: String` (required)
  - `stun_servers: Vec<String>` (default: public STUN)
  - `turn_servers: Vec<String>` (optional)
  - `pin_length: usize` (default: 6)
  - `session_timeout_secs: u64` (default: 1800)
- implement `AppConfig::load() -> AppResult<Self>`
  - โหลดจาก `config.toml` ถ้ามี
  - override ด้วย env vars
  - ใช้ defaults สำหรับค่าที่ขาด
**ได้อะไร:** Config loading system
**เสร็จเมื่อไร:** `AppConfig::load()` ทำงานได้

### 3.2 สร้าง config.toml.example
**ทำไป:** ให้ template config ให้ user
**ทำอย่างไร:**
- สร้าง `config.toml` ที่ examples
- ใส่ทุก field พร้อม comment
- แสดง default values
- แสดง examples
**ได้อะไร:** Template config file
**เสร็จเมื่อไร:** User copy ได้และ customize ได้

---

## Milestone 4: CLI Argument Parsing

### 4.1 สร้าง CLI structure ใน src/config/args.rs
**ทำไป:** นิยาม CLI commands และ flags
**ทำอย่างไร:**
- สร้าง `Cli` struct ด้วย `#[derive(Parser)]`
- subcommands enum:
  - `Serve` - start server
    - `--host <HOST>` (default: `127.0.0.1`)
    - `--port <PORT>` (default: `8080`)
    - `--config <PATH>` (optional)
  - `Connect` - connect to peer
    - `<DEVICE_ID>` (positional)
    - `--code <CODE>` (optional, for pairing)
    - `--timeout <SECS>` (default: `30`)
  - `Cp` - copy files
    - `<SOURCE>` (positional)
    - `<DEST>` (positional)
    - `--device <ID>` (required)
    - `--recursive` (flag)
  - `Devices` - list devices
    - `--format <FORMAT>` (json/table, default: table)
    - `--filter <FILTER>` (optional)
  - `Version` - show version
  - `Help` - show help
**ได้อะไร:** CLI parser ที่ครบ
**เสร็จเมื่อไร:** `cargo run -- --help` แสดง commands ทั้งหมด

### 4.2 ผูก CLI เข้ากับ main.rs
**ทำไป:** เชื่อม CLI parser กับ business logic
**ทำอย่างไร:**
- ใน `src/main.rs`:
  - parse CLI args: `let cli = Cli::parse()`
  - match subcommand
  - เรียก handler ที่เหมาะสม
  - pass arguments ไปให้ handler
**ได้อะไร:** CLI routing ที่ทำงาน
**เสร็จเมื่อไร:** แต่ละ command เรียก handler ได้

---

## Milestone 5: Identity Management

### 5.1 สร้าง Identity system ใน src/identity/key.rs
**ทำไป:** สร้างและเก็บ device identity
**ทำอย่างไร:**
- สร้าง `Identity` struct:
  - `device_id: String` (unique identifier)
  - `public_key: Vec<u8>` (RSA public key)
  - `private_key: Vec<u8>` (RSA private key, encrypted)
- implement methods:
  - `Identity::generate() -> AppResult<Self>` - สร้าง key pair ใหม่
  - `Identity::load(path: &str) -> AppResult<Self>` - โหลดจากดิสก์
  - `Identity::save(&self, path: &str) -> AppResult<()>` - เก็บลงดิสก์
  - `Identity::sign(&self, data: &[u8]) -> AppResult<Vec<u8>>`
  - `Identity::verify(data: &[u8], sig: &[u8], pub_key: &[u8]) -> bool`
  - `Identity::encrypt(&self, data: &[u8]) -> AppResult<Vec<u8>>`
  - `Identity::decrypt(&self, data: &[u8]) -> AppResult<Vec<u8>>`
- เก็บใน `~/.blnk/identity.json`
**ได้อะไร:** Identity generation และ persistence
**เสร็จเมื่อไร:** Device มี persistent unique identity

### 5.2 สร้าง Pairing Code flow ใน src/protocol/pairing.rs
**ทำไป:** implement pairing ระหว่าง devices
**ทำอย่างไร:**
- สร้าง `PairingCode` struct:
  - `code: String` (6-8 digit)
  - `created_at: Timestamp`
  - `expires_at: Timestamp`
- implement methods:
  - `PairingCode::generate() -> Self` - สร้าง random code
  - `PairingCode::verify(code: &str) -> bool` - check code match
  - `PairingCode::is_expired() -> bool`
- implement commit-reveal flow
**ได้อะไร:** Pairing code generation และ verification
**เสร็จเมื่อไร:** สอง devices pair ได้ด้วย code exchange

---

## Milestone 6: Protocol Definitions

### 6.1 นิยาม SWSP frame format ใน src/protocol/swsp.rs
**ทำไป:** ออกแบบ message frame structure
**ทำอย่างไร:**
- นิยาม frame header (8 bytes):
  - Version (1 byte)
  - Frame type (1 byte)
  - Flags (1 byte)
  - Reserved (1 byte)
  - Length (4 bytes, big-endian)
- frame types:
  - `0x01` - Shell
  - `0x02` - File
  - `0x03` - Proxy
  - `0x04` - TCP
  - `0x05` - WebSocket
  - `0x06` - Control
- payload ตามหลัง header
- สร้าง `Frame` struct
- implement `FrameParser::parse(data: &[u8]) -> AppResult<Frame>`
  - validate header
  - extract frame type
  - extract payload
  - handle incomplete frames
- implement `FrameBuilder`:
  - `new(frame_type: u8) -> Self`
  - `set_payload(data: Vec<u8>) -> Self`
  - `set_flags(flags: u8) -> Self`
  - `build() -> Vec<u8>` (complete frame)
**ได้อะไร:** Frame parsing และ building logic
**เสร็จเมื่อไร:** Parse valid frames และ reject invalid ones

### 6.2 นิยาม signaling messages ใน src/signaling/messages.rs
**ทำไป:** ออกแบบ signaling protocol messages
**ทำอย่างไร:**
- สร้าง `SignalingMessage` enum:
  - `Register { device_id: String, public_key: Vec<u8> }`
  - `RegisterAck { status: u32 }`
  - `RequestConnection { peer_id: String }`
  - `Offer { peer_id: String, sdp: String }`
  - `Answer { peer_id: String, sdp: String }`
  - `IceCandidate { peer_id: String, candidate: String }`
  - `PairingRequest { device_id: String }`
  - `PairingCode { code: String }`
  - `Error { code: u32, message: String }`
- implement serialization/deserialization
**ได้อะไร:** Signaling message definitions
**เสร็จเมื่อไร:** Serialize/deserialize messages ได้

---

## Milestone 7: Signaling Client

### 7.1 สร้าง SignalingClient ใน src/signaling/client.rs
**ทำไป:** เชื่อมต่อกับ signaling server
**ทำอย่างไร:**
- สร้าง `SignalingClient` struct
- implement WebSocket connection ไปยัง signaling URL
- implement methods:
  - `SignalingClient::connect(url: &str) -> AppResult<Self>`
  - `send_message(msg: SignalingMessage) -> AppResult<()>`
  - `recv_message() -> AppResult<SignalingMessage>`
  - `register(device_id: &str, pub_key: &[u8]) -> AppResult<()>`
  - `request_connection(peer_id: &str) -> AppResult<()>`
  - `send_offer(peer_id: &str, sdp: &str) -> AppResult<()>`
  - `send_answer(peer_id: &str, sdp: &str) -> AppResult<()>`
  - `send_ice_candidate(peer_id: &str, candidate: &str) -> AppResult<()>`
- handle reconnection logic
- handle message queue
**ได้อะไร:** Signaling client ที่ทำงาน
**เสร็จเมื่อไร:** Connect ได้ และ exchange messages ได้

---

## Milestone 8: WebRTC Peer Connection

### 8.1 สร้าง PeerConnection ใน src/peer/connection.rs
**ทำไป:** สร้างและจัดการ WebRTC peer connection
**ทำอย่างไร:**
- สร้าง `PeerConnection` struct
- ใช้ `webrtc` crate สร้าง connection
- implement methods:
  - `PeerConnection::new() -> AppResult<Self>`
  - `create_offer() -> AppResult<String>` (returns SDP)
  - `create_answer(offer: &str) -> AppResult<String>`
  - `set_remote_description(sdp: &str) -> AppResult<()>`
  - `add_ice_candidate(candidate: &str) -> AppResult<()>`
  - `connection_state() -> ConnectionState`
  - `on_ice_candidate(callback: Box<dyn Fn(String)>)`
  - `on_connection_state_change(callback: Box<dyn Fn(ConnectionState)>)`
- track connection state
**ได้อะไร:** WebRTC peer connection
**เสร็จเมื่อไร:** Exchange SDP และ establish connection

### 8.2 สร้าง DataChannel ใน src/peer/connection.rs
**ทำไป:** ส่ง/รับข้อมูลผ่าน P2P
**ทำอย่างไร:**
- สร้าง `DataChannel` struct
- implement methods:
  - `send(data: Vec<u8>) -> AppResult<()>`
  - `recv() -> AppResult<Vec<u8>>`
  - `on_open(callback: Box<dyn Fn()>)`
  - `on_close(callback: Box<dyn Fn()>)`
  - `on_message(callback: Box<dyn Fn(Vec<u8>)>)`
- buffer incoming data
- handle channel lifecycle
**ได้อะไร:** Data channel สำหรับ P2P communication
**เสร็จเมื่อไร:** Send/receive data ระหว่าง peers

### 8.3 ตั้งค่า STUN/TURN ใน src/peer/ice.rs
**ทำไป:** ตั้งค่า NAT traversal
**ทำอย่างไร:**
- โหลด STUN servers จาก config
- โหลด TURN servers จาก config
- pass ไปยัง WebRTC connection
- handle TURN credentials
**ได้อะไร:** STUN/TURN configuration
**เสร็จเมื่อไร:** Connection ทำงานผ่าน NAT

---

## Milestone 9: Session Management

### 9.1 สร้าง Session ใน src/session/session.rs
**ทำไป:** ดูแล lifecycle ของ session
**ทำอย่างไร:**
- สร้าง `Session` struct:
  - `session_id: String` (unique per connection)
  - `peer_id: String` (connected device)
  - `device_id: String` (local device)
  - `created_at: Timestamp`
  - `last_activity: Timestamp`
  - `state: SessionState` (enum: Active, Idle, Closed)
  - `streams: HashMap<u32, StreamHandler>`
- implement `SessionManager`:
  - `create_session(peer_id: &str) -> AppResult<Session>`
  - `get_session(session_id: &str) -> Option<&Session>`
  - `close_session(session_id: &str) -> AppResult<()>`
  - `update_activity(session_id: &str)`
  - `cleanup_expired_sessions()`
- timeout logic (30 min default)
- session cleanup
**ได้อะไร:** Session tracking system
**เสร็จเมื่อไร:** Create/track/cleanup sessions

### 9.2 สร้าง PIN Authentication ใน src/session/auth.rs
**ทำไป:** implement PIN verification
**ทำอย่างไร:**
- สร้าง `PinAuth` struct
- implement methods:
  - `generate_pin() -> String` (random 6-digit)
  - `verify_pin(entered: &str, stored: &str) -> bool` (constant-time)
  - `hash_pin(pin: &str) -> String`
- constant-time comparison เพื่อป้องกัน timing attack
**ได้อะไร:** PIN authentication system
**เสร็จเมื่อไร:** Verify PIN ได้อย่างปลอดภัย

---

## Milestone 10: Stream Handler Base

### 10.1 สร้าง StreamHandler trait ใน src/stream/handler.rs
**ทำไป:** นิยาม interface สำหรับ stream processors
**ทำอย่างไร:**
- สร้าง trait `StreamHandler`:
  ```rust
  pub trait StreamHandler: Send + Sync {
      async fn handle(&self, frame: Frame) -> AppResult<Frame>;
      fn frame_type(&self) -> u8;
      fn name(&self) -> &str;
  }
  ```
- สร้าง `StreamDispatcher`:
  - `register(handler: Box<dyn StreamHandler>)`
  - `dispatch(frame: Frame) -> AppResult<Frame>`
  - route by frame type
  - handle unknown types
**ได้อะไร:** Handler trait และ dispatcher
**เสร็จเมื่อไร:** Register และ dispatch ไปยัง handlers

---

## Milestone 11: Stream Handlers Implementation

### 11.1 Shell Handler ใน src/stream/shell.rs
**ทำไป:** Execute remote shell commands
**ทำอย่างไร:**
- สร้าง `ShellHandler` implement `StreamHandler`
- frame type: `0x01`
- payload format:
  - Command (string)
  - Working directory (string)
  - Environment variables (map)
- response format:
  - Exit code (u32)
  - Stdout (bytes)
  - Stderr (bytes)
- implement PTY support สำหรับ interactive shells
- support Linux, Windows, Android
- ใช้ `src/shell/pty.rs` สำหรับ PTY operations
**ได้อะไร:** Shell command execution
**เสร็จเมื่อไร:** Run commands และ get output

### 11.2 File Transfer Handler ใน src/stream/file.rs
**ทำไป:** Transfer files ระหว่าง devices
**ทำอย่างไร:**
- สร้าง `FileHandler` implement `StreamHandler`
- frame type: `0x02`
- operations:
  - `List(path: String)` - list directory
  - `Download(path: String)` - send file
  - `Upload(path: String, data: Vec<u8>)` - receive file
  - `Delete(path: String)` - delete file
  - `Stat(path: String)` - get file info
- support range requests สำหรับ large files
- handle permissions และ errors
- ใช้ `src/fileshare/share.rs` สำหรับ file operations
**ได้อะไร:** File transfer operations
**เสร็จเมื่อไร:** List/upload/download/delete files

### 11.3 HTTP Proxy Handler ใน src/stream/proxy.rs
**ทำไป:** Forward HTTP requests
**ทำอย่างไร:**
- สร้าง `ProxyHandler` implement `StreamHandler`
- frame type: `0x03`
- payload format:
  - HTTP method (string)
  - URL (string)
  - Headers (map)
  - Body (bytes)
- response format:
  - Status code (u16)
  - Headers (map)
  - Body (bytes)
- rewrite headers ตามต้องการ
- handle redirects
- ใช้ `src/proxy/http.rs` สำหรับ HTTP operations
**ได้อะไร:** HTTP proxy functionality
**เสร็จเมื่อไร:** Proxy HTTP requests

### 11.4 TCP Forwarding Handler ใน src/stream/tcp.rs
**ทำไป:** Forward raw TCP traffic
**ทำอย่างไร:**
- สร้าง `TcpHandler` implement `StreamHandler`
- frame type: `0x04`
- payload: raw TCP data
- bidirectional forwarding
- handle connection lifecycle
- ใช้ `src/tcpforward/forward.rs` สำหรับ TCP operations
**ได้อะไร:** TCP forwarding
**เสร็จเมื่อไร:** Forward TCP connections

### 11.5 WebSocket Handler ใน src/stream/websocket.rs
**ทำไป:** Bridge WebSocket connections
**ทำอย่างไร:**
- สร้าง `WebSocketHandler` implement `StreamHandler`
- frame type: `0x05`
- payload format:
  - WebSocket URL (string)
  - Message (bytes)
- handle connection upgrade
- forward messages bidirectionally
**ได้อะไร:** WebSocket bridging
**เสร็จเมื่อไร:** Bridge WebSocket connections

---

## Milestone 12: Web Server Integration

### 12.1 สร้าง HTTP Server ใน src/web/mod.rs
**ทำไป:** Start web server
**ทำอย่างไร:**
- ใช้ `axum` framework
- สร้าง router ด้วย routes:
  - `GET /api/devices` - list devices
  - `POST /api/connect` - initiate connection
  - `POST /api/execute` - run command
  - `GET /api/files` - list files
  - `POST /api/upload` - upload file
  - `GET /api/download` - download file
  - `WS /ws` - WebSocket endpoint
- bind ไปยัง configured host:port
- handle CORS
- handle errors gracefully
**ได้อะไร:** HTTP server ด้วย routes
**เสร็จเมื่อไร:** Server start และ routes respond

### 12.2 Static File Serving ใน src/web/mod.rs
**ทำไป:** Serve frontend assets
**ทำอย่างไร:**
- สร้าง route `GET /static/<path>`
- serve files จาก `static/` directory
- หรือ embed ใน binary ถ้าต้องการ
- set appropriate MIME types
**ได้อะไร:** Static file serving
**เสร็จเมื่อไร:** Serve CSS, JS, HTML files

### 12.3 WebSocket Endpoint ใน src/web/mod.rs
**ทำไป:** Real-time communication
**ทำอย่างไร:**
- สร้าง `WS /ws` route
- handle WebSocket upgrade
- forward messages ไปยัง protocol layer
- send responses กลับไปยัง client
- handle connection close
**ได้อะไร:** WebSocket endpoint
**เสร็จเมื่อไร:** Send/receive messages via WebSocket

---

## Milestone 13: CLI Commands Implementation

### 13.1 Implement `serve` command
**ทำไป:** Start server
**ทำอย่างไร:**
- ใน `src/main.rs` หรือ `src/commands/serve.rs`:
  - load config
  - initialize logging
  - load/generate identity
  - start HTTP server
  - start signaling connection
  - print startup message ด้วย URL
  - handle Ctrl+C gracefully
**ได้อะไร:** Running server
**เสร็จเมื่อไร:** Server start และ listen on configured port

### 13.2 Implement `connect` command
**ทำไป:** Connect to peer
**ทำอย่างไร:**
- accept `<DEVICE_ID>` argument
- accept optional `--code <CODE>` flag
- generate pairing code ถ้าไม่มี code
- connect ไปยัง signaling server
- exchange credentials
- establish WebRTC connection
- print connection status
**ได้อะไร:** Active connection to peer
**เสร็จเมื่อไร:** Connection established และ ready

### 13.3 Implement `cp` command
**ทำไป:** Copy files
**ทำอย่างไร:**
- accept `<SOURCE>` และ `<DEST>` arguments
- require `--device <ID>` flag
- support `--recursive` flag
- connect ไปยัง device
- transfer files
- show progress
- handle errors
**ได้อะไร:** Files copied
**เสร็จเมื่อไร:** Files transferred successfully

### 13.4 Implement `devices` command
**ทำไป:** List paired devices
**ทำอย่างไร:**
- load device list จาก config/cache
- accept `--format <FORMAT>` flag (json/table)
- accept `--filter <FILTER>` flag
- display device info:
  - Device ID
  - Name
  - Last seen
  - Status
- format output
**ได้อะไร:** Device list
**เสร็จเมื่อไร:** Show all paired devices

### 13.5 Add version และ help
**ทำไป:** Show program info
**ทำอย่างไร:**
- implement `--version` flag
- show version จาก `Cargo.toml`
- implement `--help` flag
- show usage สำหรับ all commands
- show examples
**ได้อะไร:** Help และ version output
**เสร็จเมื่อไร:** `--help` และ `--version` ทำงาน

---

## Milestone 14: Testing

### 14.1 Write unit tests
**ทำไป:** Test individual components
**ทำอย่างไร:**
- test error types
- test frame parsing/building
- test config loading
- test identity generation
- test crypto functions
- test handler logic
- test signaling messages
**ได้อะไร:** Unit test suite
**เสร็จเมื่อไร:** `cargo test` ผ่าน

### 14.2 Write integration tests
**ทำไป:** Test component interactions
**ทำอย่างไร:**
- test signaling flow
- test peer connection flow
- test session negotiation
- test file/tcp/shell stream behavior
- test CLI command execution
- test server startup
**ได้อะไร:** Integration test suite
**เสร็จเมื่อไร:** End-to-end flows ทำงาน

### 14.3 Test cross-platform build
**ทำไป:** Verify builds on multiple OS
**ทำอย่างไร:**
- build on Linux
- build on Windows
- verify all features ทำงาน
**ได้อะไร:** Cross-platform binaries
**เสร็จเมื่อไร:** Builds succeed on all platforms

---

## Milestone 15: Performance, Security & Release

### 15.1 Performance testing
**ทำไป:** Measure performance
**ทำอย่างไร:**
- benchmark frame parsing
- benchmark file transfer speed
- measure memory usage
- profile hot paths
**ได้อะไร:** Performance metrics
**เสร็จเมื่อไร:** Meets performance targets

### 15.2 Security audit
**ทำไป:** Check for vulnerabilities
**ทำอย่างไร:**
- review crypto implementation
- check for buffer overflows
- verify input validation
- check dependency vulnerabilities
- use `cargo audit`
**ได้อะไร:** Security report
**เสร็จเมื่อไร:** No critical issues found

### 15.3 Write documentation
**ทำไป:** Create user และ developer docs
**ทำอย่างไร:**
- write `README.md`
- write `ARCHITECTURE.md`
- write `PROTOCOL.md`
- write `API.md`
- write `CONTRIBUTING.md`
- write usage examples
**ได้อะไร:** Complete documentation
**เสร็จเมื่อไร:** All docs written และ reviewed

### 15.4 Setup CI/CD
**ทำไป:** Automate testing และ releases
**ทำอย่างไร:**
- create GitHub Actions workflows
- test on every push
- build binaries on release
- upload ไปยัง releases page
**ได้อะไร:** CI/CD pipeline
**เสร็จเมื่อไร:** Automated builds และ tests ทำงาน

### 15.5 Create release binaries
**ทำไป:** Package สำหรับ distribution
**ทำอย่างไร:**
- build static binaries
- create installers ถ้าต้องการ
- sign binaries
- create checksums
- upload ไปยัง release page
**ได้อะไร:** Distributable binaries
**เสร็จเมื่อไร:** Users download และ run ได้

---
