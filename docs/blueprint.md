# blnk Project Blueprint

## 1. Product Voice Card

**Name:** blnk Rust  
**Type:** Remote access multitool (peer-to-peer via WebRTC)  
**Platform:** CLI binary (Linux / Windows / Android)  
**Auth:** Optional PIN + user-visible pairing code (6-digit); persistent `access_code` is separate if required by the original protocol
**Transport:** WebRTC data channel over signaling server  
**Discovery:** mDNS (LAN) + signaling server (WAN)  
**Output:** Single static binary, async-first, memory-safe  

---

## 2. Quick Reference

**Build:**
```bash
cargo build
cargo build --release
```

**Run:**
```bash
cargo run -- serve
cargo run -- connect
cargo run -- cp
```

**Test:**
```bash
cargo fmt --all -- --check
cargo clippy --all --all-targets -- -D warnings
cargo test --all
cargo audit
```

**Dependencies:**
- tokio, webrtc, axum, clap, tracing, thiserror, serde, prost, qrcode, mdns, nix/winapi
