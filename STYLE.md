# blnk Rust Code Style

## 1. หลักการเขียนโค้ด

โค้ดต้อง:
- อ่านง่าย
- แบ่งหน้าที่ชัดเจน
- testable
- cross-platform friendly
- async-safe
- free จาก panic ใน production เท่าที่เป็นไปได้

---

## 2. Naming Conventions

### 2.1 Functions / Variables / Modules
ใช้ `snake_case`
ตัวอย่าง:
- `connect_to_signaling`
- `load_identity`
- `stream_handler`
- `pin_required`

### 2.2 Types / Traits / Structs / Enums
ใช้ `PascalCase`
ตัวอย่าง:
- `PeerConnection`
- `SignalingClient`
- `SessionState`
- `StreamType`

### 2.3 Constants
ใช้ `SCREAMING_SNAKE_CASE`
ตัวอย่าง:
- `MAX_FRAME_SIZE`
- `DEFAULT_PIN_TIMEOUT_MS`
- `SWSP_VERSION`

---

## 3. Documentation Style

### 3.1 Module Documentation
ใช้ `//!` สำหรับอธิบาย module
ตัวอย่าง:

```rust
//! Signaling client for blnk Rust.
```

### 3.2 Item Documentation
ใช้ `///` สำหรับ function, struct, enum, trait
ตัวอย่าง:

```rust
/// Connects to the signaling server and registers the current device.
pub async fn connect() -> Result<(), BlnkError> {
    Ok(())
}
```

---

## 4. Type Derives

ควรใช้ `#[derive(...)]` เมื่อเหมาะสม
แนะนำ:
- `Debug`
- `Clone`
- `PartialEq`
- `Eq`
- `Serialize`
- `Deserialize`
ตัวอย่าง:

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DeviceId {
    pub uid: String,
}
```

---

## 5. Error Handling Style

### 5.1 ห้ามใช้ใน production
- `unwrap()`
- `expect()` โดยไม่มี context
- `panic!()` เป็น error flow
- silent failure

### 5.2 แนะนำ
- ใช้ `Result<T, E>`
- ใช้ `?`
- ใช้ `thiserror` สำหรับ typed error
- ใช้ `anyhow` สำหรับ application-level context
ตัวอย่าง:

```rust
pub async fn connect() -> Result<SignalingClient, BlnkError> {
    let client = SignalingClient::new().await?;
    Ok(client)
}
```

---

## 6. Async Style

### 6.1 ใช้ async/await เป็นหลัก
- ใช้ `async fn`
- ใช้ `tokio`
- แยก task เมื่อจำเป็น
- ใช้ `tokio::select!` เมื่อรอหลาย future

### 6.2 หลักการ
- อย่า block runtime โดยไม่จำเป็น
- งาน I/O ต้อง async
- งาน CPU-heavy ให้พิจารณา `spawn_blocking`

---

## 7. Logging Style

ใช้ `tracing` แทน `println!`
ตัวอย่าง:
```rust
use tracing::{info, debug, error};
info!("session started");
debug!(peer_id = %peer_id, "peer connected");
error!(error = %err, "connection failed");
```

แนวทาง:
- log ต้องมี context
- หลีกเลี่ยงการ log ข้อมูลลับ
- ใช้ span สำหรับ session/peer/stream

---

## 8. Module Organization

### 8.1 Layout
- `mod.rs` ใช้เป็น public entry ของ module
- แยก file ตาม responsibility
- re-export เฉพาะสิ่งที่ใช้ภายนอกจริง

### 8.2 Example

```rust
pub mod connection;
pub mod ice;
pub use connection::PeerConnection;
```

---

## 9. Code Size and Complexity Rules

### 9.1 Function Size
- ฟังก์ชันไม่ควรยาวเกินประมาณ 50 บรรทัดโดยไม่มีเหตุผล

### 9.2 Nesting
- หลีกเลี่ยง nested `if/else` มากเกิน 3 ชั้น
- ใช้ early return
- แยก helper function

### 9.3 Magic Numbers
- ห้าม hardcode ตัวเลขสำคัญโดยไม่มี constant
ตัวอย่าง:

```rust
const MAX_AUTH_FAILS: usize = 3;
```

---

## 10. Concurrency Style

- ใช้ `Arc<T>` เมื่อ share ownership
- ใช้ `Arc<Mutex<T>>` หรือ `Arc<RwLock<T>>` เฉพาะเมื่อจำเป็น
- ใช้ channel เป็นตัวกลางมากกว่าการแชร์ mutable state โดยตรง
- avoid lock ระยะยาว

---

## 11. Platform-Specific Style

- ใช้ `cfg(target_os = "...")` สำหรับ code ที่ต่างกัน
- แยก Unix/Windows ให้ชัด
- อย่าปน platform code กับ shared protocol code

---

## 12. Testing Style

### 12.1 Unit Test
- ทดสอบ behavior ของ function เดี่ยว
- ทดสอบ encoding/decoding
- ทดสอบ auth logic
- ทดสอบ edge cases

### 12.2 Integration Test
- ทดสอบ flow จริงระหว่าง modules
- ใช้ async test
- ตั้งชื่อ test ให้ชัดเจน
ตัวอย่าง:

```rust
#[tokio::test]
async fn test_signaling_connection_success() {
    // ...
}
```

---

## 13. Dependency Style
- ใช้ dependency เท่าที่จำเป็น
- prefer crates ที่ mature และมี maintenance ดี
- ถ้ามี feature ใน std ใช้ std ก่อน
- ถ้ามีหลาย crate ทำหน้าที่คล้ายกัน ให้เลือกตัวที่เหมาะกับ architecture มากที่สุด

---

## 14. Security Style

- validate input ทุก boundary
- อย่า log secret
- ใช้ constant-time comparison สำหรับ PIN และ secret comparison
- แยก crypto logic ออกจาก business logic
- code path สำหรับ sensitive operation ต้อง review ง่าย

---

## 15. Recommended Rust Practices
- ใช้ `Option<T>` สำหรับค่าที่อาจไม่มี
- ใช้ `Result<T, E>` สำหรับ error path
- ใช้ iterator แทน loop ที่ซับซ้อนเมื่อเหมาะสม
- ใช้ `match` เมื่อ logic เป็นแบบหลายกรณี
- ใช้ trait เพื่อ abstract handler ต่าง ๆ

---

## 16. Example Pattern

```rust
/// Represents a stream handler.
pub trait StreamHandler {
    /// Returns the stream type supported by this handler.
    fn stream_type(&self) -> StreamType;
    /// Handles an incoming frame.
    async fn handle_frame(&self, frame: Frame) -> Result<(), BlnkError>;
}
```

---

## 17. Final Style Rule

ถ้าโค้ดอ่านแล้วไม่ชัดว่า:
- ใครเป็นเจ้าของ state
- error ไหลไปทางไหน
- async boundary อยู่ตรงไหน
- protocol message นี้ใช้ทำอะไร
แปลว่ายังไม่ผ่าน style ของโปรเจคนี้