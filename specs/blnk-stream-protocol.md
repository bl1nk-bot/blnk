# blnk Stream Message Protocol Specification

**สถานะ:** Implementation contract  
**Version:** v1  
**ขอบเขต:** Unified stream message envelope, SWSP flag mapping, dispatch table  
**Source files:** `proto/stream.proto`, `src/protocol/stream.rs`, `src/protocol/swsp.rs`

เอกสารนี้อธิบาย unified stream message envelope ที่ครอบคลุมทุก stream type (shell, file, http, tcp, websocket) ให้มี dispatch metadata เดียวกัน โดยยังคง backward compatibility กับ raw SWSP frame ที่มีอยู่

---

## 1. ภาพรวม

### 1.1 ปัญหา

ก่อนหน้านี้ แต่ละ stream type มี encode/decode functions เฉพาะตัวที่สร้าง SWSP frame โดยตรง:
- Shell: `encode_open_frame`, `encode_input_frame`, `encode_output_frame`
- File: `encode_request_frame`, `encode_data_frame`, `encode_response_frames`

ไม่มี unified way ที่จะ send/receive ข้าม stream type ได้ และ dispatch logic กระจายอยู่ใน `runtime.rs`

### 1.2 แนวทางแก้ไข

เพิ่ม `StreamMessage` envelope ที่ครอบ SWSP frame payload ด้วย dispatch metadata:
- `stream_type`: ระบุ stream type (Shell, File, Http, Tcp, WebSocket)
- `kind`: ระบุ semantic action (Open, Data, Close, Error, Cancel, Resize, Metadata)
- `sequence`: monotonic counter ภายใน stream
- `payload`: type-specific protobuf bytes

Envelope เป็น **opt-in** — existing handlers ยังใช้ raw SWSP frames ได้ตามเดิม

---

## 2. Protocol Contract

### 2.1 StreamMessage Envelope

```protobuf
message StreamMessage {
  StreamType stream_type = 1;
  StreamMessageKind kind = 2;
  uint32 sequence = 3;
  bytes payload = 10;
}
```

- `stream_type`: enum จาก `proto/stream.proto` — ระบุ stream type ที่ message นี้ belongs to
- `kind`: enum `StreamMessageKind` — ระบุ semantic action
- `sequence`: monotonic counter ภายใน stream สำหรับ ordering/dedup
- `payload`: type-specific protobuf message ที่ decode ได้จาก (stream_type, kind) pair

### 2.2 StreamMessageKind Enum

| Kind | Wire Value | ความหมาย |
|---|---|---|
| `UNSPECIFIED` | 0 | ค่าเริ่มต้น ห้ามใช้ |
| `OPEN` | 1 | Stream open / handshake (SYN) |
| `DATA` | 2 | Bulk data transfer |
| `CLOSE` | 3 | Graceful close (FIN) |
| `ERROR` | 4 | Error / policy rejection |
| `CANCEL` | 5 | Cancellation request |
| `RESIZE` | 6 | Terminal resize (shell only) |
| `METADATA` | 7 | Response metadata (file info, list) |

### 2.3 SWSP Flag Mapping

Envelope แปลง `kind` เป็น SWSP frame flags ดังนี้:

| Kind | SWSP Flags | หมายเหตุ |
|---|---|---|
| `OPEN` | `SYN \| DAT` | Start of stream + metadata payload |
| `DATA` | `DAT` | Intermediate data |
| `CLOSE` | `DAT \| FIN` | Terminal frame |
| `ERROR` | `DAT \| FIN` | Terminal frame |
| `CANCEL` | `DAT \| FIN` | Terminal frame |
| `RESIZE` | `DAT` | Non-terminal metadata |
| `METADATA` | `DAT` | Non-terminal metadata |

### 2.4 Dispatch Table

ตารางนี้อธิบายว่า `payload` bytes decode เป็น message อะไร เมื่อ `(stream_type, kind)` คู่กัน:

| StreamType | Kind | Payload Schema |
|---|---|---|
| Shell | OPEN | `ShellOpen` |
| Shell | DATA | `ShellInput` \| `ShellOutput` (direction depends on role) |
| Shell | RESIZE | `TerminalResize` |
| Shell | CLOSE | `ShellExit` |
| Shell | ERROR | `ShellError` |
| Shell | CANCEL | `ShellCancel` |
| File | OPEN | `FileOp` |
| File | DATA | raw bytes |
| File | METADATA | `FileInfo` \| `FileList` |
| File | ERROR | `ShellError` (reused) |
| Http | OPEN | `HTTPRequest` |
| Http | DATA | `HTTPData` |
| Http | METADATA | `HTTPResponse` |
| Tcp | OPEN | `TCPOpen` |
| Tcp | DATA | `TCPData` |
| WebSocket | OPEN | `WebSocketOpen` |
| WebSocket | DATA | `WebSocketMessage` |

---

## 3. Rust Codec Contract

### 3.1 StreamEnvelope

`src/protocol/stream.rs` ให้ `StreamEnvelope` struct:

```rust
pub struct StreamEnvelope {
    pub stream_type: StreamTypeTag,
    pub kind: MessageKind,
    pub sequence: u32,
    pub payload: Vec<u8>,
}
```

### 3.2 API Surface

| Method | Behavior |
|---|---|
| `new(stream_type, kind, sequence, payload)` | สร้าง envelope |
| `encode_proto()` | Encode เป็น protobuf bytes สำหรับ SWSP payload |
| `decode_proto(bytes)` | Decode จาก protobuf bytes |
| `to_frame(stream_id)` | Wrap เป็น SWSP frame (infer flags จาก kind) |
| `from_frame(frame)` | Extract envelope จาก SWSP data frame |
| `infer_kind_from_flags(flags)` | Backward compat: infer kind จาก SWSP flags |

### 3.3 Constraints

- `stream_id` ต้องไม่เป็น 0 (control stream ห้ามใช้ envelope)
- `from_frame` ปฏิเสธ frame ที่มี `stream_id == 0`
- `kind` ต้องไม่เป็น `UNSPECIFIED`

---

## 4. Backward Compatibility

### 4.1 Opt-in Model

Existing handlers ยังใช้ raw SWSP frames ได้ตามเดิม:
- `shell.rs`: `encode_open_frame`, `encode_input_frame`, etc.
- `file.rs`: `encode_request_frame`, `encode_data_frame`, etc.

Envelope เป็น additive layer ที่ new code สามารถใช้ได้เมื่อต้องการ typed dispatch

### 4.2 Flag Inference

`infer_kind_from_flags()` ให้ backward compatibility โดยแปลง SWSP flags เป็น kind:
- `SYN` → `Open`
- `FIN` → `Close`
- อื่นๆ → `Data`

### 4.3 Migration Path

1. New stream types ควรใช้ envelope ตั้งแต่แรก
2. Existing handlers ค่อยๆ migrate ได้ตาม convenience
3. ห้ามลบ raw frame encode/decode functions ที่มีอยู่

---

## 5. Build System Contract

### 5.1 Protoc Resolution

`build.rs` ต้อง resolve protoc binary ตามลำดับ:
1. `protoc-bin-vendored` (default สำหรับ deterministic builds)
2. `PROTOC` env var (ถ้าตั้งไว้)
3. `which protoc` (system binary)
4. Panic พร้อม helpful error message

### 5.2 Android Compatibility

`protoc-bin-vendored` ไม่รองรับ `aarch64-android` — build.rs ต้อง fall back ไปใช้ system protoc ที่ติดตั้งผ่าน package manager (เช่น `pkg install protobuf` บน Termux)

### 5.3 Warning Format

เมื่อ fall back ไปใช้ system protoc ต้อง emit cargo warning:
```
cargo:warning=protoc-bin-vendored unavailable (<error>); using system protoc at <path>
```

---

## 6. Conformance Tests

### 6.1 Required Unit Tests

| Test | ตรวจสอบ |
|---|---|
| `envelope_round_trips_through_protobuf` | encode → decode ได้ค่าเดิม |
| `envelope_round_trips_through_swsp_frame` | to_frame → from_frame ได้ค่าเดิม |
| `open_kind_produces_syn_dat_flags` | OPEN → SYN\|DAT |
| `close_kind_produces_dat_fin_flags` | CLOSE → DAT\|FIN |
| `control_stream_id_is_rejected` | stream_id=0 ถูกปฏิเสธ |
| `from_frame_rejects_control_frame` | from_frame ปฏิเสธ control frame |
| `stream_type_tag_conversions` | Wire ↔ Tag round-trip ทุก kind |
| `message_kind_wire_round_trip` | Wire ↔ Kind round-trip ทุก kind |
| `infer_kind_from_flags_matches_encode` | Flag inference ตรงกับ kind ที่ encode ไว้ |

### 6.2 Required Integration Tests

| Test | ตรวจสอบ |
|---|---|
| `compatibility_baseline` | SWSP raw bytes, protobuf control, signaling register, session lifecycle ยังผ่าน |
| `authenticated_session_reaches_ready` | Session handshake ยังทำงานได้ |

---

## 7. Source of Truth

- Protobuf wire contract: `proto/stream.proto`
- Rust generated bindings: `src/proto_generated/stream.rs`
- Rust codec: `src/protocol/stream.rs`
- SWSP frame codec: `src/protocol/swsp.rs`
- Main specification: `specs/spec.md`
- Schema contract: `specs/blnk-protobuf-schema.md`
