# blnk Core Foundation Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Implement the minimal runnable foundation for blnk: library root, typed errors, config loading, identity stubs, and CLI wiring, with tests.

**Architecture:** Follow `docs/architecture.md` module layout. Keep the module layout stable; documentation-only status and decision records may be added without changing runtime design. Implement real code, not stubs.

**Tech Stack:** Rust 2024, tokio, clap derive, config, serde, thiserror, anyhow, tracing.

**Platform constraint:** Linux / Windows / Android only. macOS is out of scope.

**Conflict resolution order:** `docs/architecture.md` > `specs/spec.md` > `docs/api.md` > `STYLE.md` > `README.md` > `TODO.md`.

---

### Task 1: Add lib root and module placeholders

**Objective:** Turn the crate into a proper library-backed binary so modules can compile incrementally.

**Files:**
- Modify: `Cargo.toml`
- Create: `src/lib.rs`
- Modify: `src/main.rs`

**Step 1: Update Cargo.toml**

Add `[lib]` and `[[bin]]` entries:

```toml
[lib]
name = "blnk"
path = "src/lib.rs"

[[bin]]
name = "blnk"
path = "src/main.rs"
```

**Step 2: Create `src/lib.rs`**

```rust
pub mod config;
pub mod identity;
pub mod signaling;
pub mod peer;
pub mod session;
pub mod stream;
pub mod protocol;
pub mod utils;

pub use utils::error::BlnkError;
```

**Step 3: Update `src/main.rs`**

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Serve,
    Connect,
    Cp,
    Devices,
    Version,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Serve => println!("serve"),
        Commands::Connect => println!("connect"),
        Commands::Cp => println!("cp"),
        Commands::Devices => println!("devices"),
        Commands::Version => println!("blnk 0.1.0"),
    }
}
```

**Step 4: Verify compile**

Run: `cargo build`
Expected: compile succeeds or fails only on missing `serve/connect/cp/version` modules.

---

### Task 2: Implement typed error module

**Objective:** Central error type used by all modules.

**Files:**
- Create: `src/utils/error.rs`
- Modify: `src/utils/mod.rs`

**Step 1: Create `src/utils/mod.rs`**

```rust
pub mod error;
```

**Step 2: Create `src/utils/error.rs`**

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum BlnkError {
    #[error("config error: {0}")]
    Config(String),

    #[error("identity error: {0}")]
    Identity(String),

    #[error("signaling error: {0}")]
    Signaling(String),

    #[error("peer error: {0}")]
    Peer(String),

    #[error("session error: {0}")]
    Session(String),

    #[error("protocol error: {0}")]
    Protocol(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
```

**Step 3: Verify compile**

Run: `cargo build`
Expected: succeeds.

---

### Task 3: Implement config module

**Objective:** Load configuration from file/env/CLI into a typed struct.

**Files:**
- Create: `src/config/mod.rs`
- Create: `src/config/args.rs`

**Step 1: Create `src/config/mod.rs`**

```rust
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub signaling_url: String,
    pub identity_path: String,
    pub pin: Option<String>,
}

impl Config {
    pub fn load() -> anyhow::Result<Self> {
        let settings = config::Config::builder()
            .add_source(config::File::with_name("blnk.toml").required(false))
            .add_source(config::Environment::with_prefix("BLNK"))
            .build()?;

        let cfg: Self = settings.try_deserialize()?;
        Ok(cfg)
    }
}
```

**Step 2: Create `src/config/args.rs`**

```rust
use clap::Parser;

#[derive(Parser, Debug, Clone)]
pub struct ServeArgs {
    #[arg(long)]
    pub signaling_url: Option<String>,
}

#[derive(Parser, Debug, Clone)]
pub struct ConnectArgs {
    #[arg(long)]
    pub target: String,
}

#[derive(Parser, Debug, Clone)]
pub struct CpArgs {
    pub source: String,
    pub destination: String,
}
```

**Step 3: Verify compile**

Run: `cargo build`
Expected: succeeds.

---

### Task 4: Implement identity stub

**Objective:** Minimal identity type and persistence stub.

**Files:**
- Create: `src/identity/mod.rs`
- Create: `src/identity/key.rs`

**Step 1: Create `src/identity/mod.rs`**

```rust
pub mod key;
pub use key::Identity;
```

**Step 2: Create `src/identity/key.rs`**

```rust
use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Debug, Clone)]
pub struct Identity {
    pub uid: String,
    pub pairing_code: String,
}

impl Identity {
    pub fn generate() -> Self {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let value = COUNTER.fetch_add(1, Ordering::Relaxed);
        Self {
            uid: uuid::Uuid::new_v4().to_string(),
            pairing_code: {
                let mut buf = [0u8; 4];
                getrandom::fill(&mut buf).expect("secure random failed");
                format!("{:06}", u32::from_le_bytes(buf) % 1_000_000)
            },
        }
    }
}
```

**Step 3: Verify compile**

Run: `cargo build`
Expected: succeeds without adding `rand`.

---

### Task 5: Add unit tests for config and identity

**Objective:** Prove the foundation behaves as expected.

**Files:**
- Modify: `src/config/mod.rs`
- Modify: `src/identity/key.rs`

**Step 1: Add tests to `src/config/mod.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_has_defaults_when_file_missing() {
        std::env::set_var("BLNK_SIGNALING_URL", "wss://example.com");
        std::env::set_var("BLNK_IDENTITY_PATH", "./identity.json");
        let cfg = Config::load().unwrap();
        assert_eq!(cfg.signaling_url, "wss://example.com");
    }
}
```

**Step 2: Add tests to `src/identity/key.rs`**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_generate_has_uid_and_code() {
        let id = Identity::generate();
        assert!(!id.uid.is_empty());
        assert_eq!(id.pairing_code.len(), 6);
    }
}
```

**Step 3: Run tests**

Run: `cargo test`
Expected: both tests pass.

---

### Task 6: Run full verification

**Objective:** Ensure foundation compiles and tests are green.

**Commands:**

```bash
cargo fmt --all -- --check
cargo clippy --all --all-targets -- -D warnings
cargo test --all
```

**Expected:** all pass.
