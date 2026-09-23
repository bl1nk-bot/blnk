# blnk Wire Protocol Specification

blnk uses **SWSP (Stream Wire Session Protocol)** over WebRTC Data Channels and WebSocket Signaling.

## 1. SWSP Frame Header

Each SWSP frame begins with an 8-byte little-endian header:

| Field | Size | Description |
| --- | --- | --- |
| `stream_id` | 4 bytes (u32 LE) | Stream identifier (0 is reserved for control) |
| `flags` | 2 bytes (u16 LE) | Bitmask flags: SYN, ACK, FIN, RST, DAT, PSH |
| `length` | 2 bytes (u16 LE) | Payload size in bytes (max 65,535 bytes) |

Followed by `length` bytes of raw payload.

## 2. Stream Multiplexing

- `stream_id = 0`: Session and pairing control messages (Protobuf / JSON).
- `stream_id > 0`: Application streams:
  - **Shell Stream**: Interactive PTY / command execution.
  - **File Stream**: Chunked file upload and download transfers.
  - **Proxy Stream**: TCP, HTTP, and WebSocket port-forwarding.

## 3. Cryptographic Identity & Pairing

- RSA-2048 identity key pairs.
- Commit-reveal pairing authentication with 6-digit numeric SAS (Short Authentication String).
- Constant-time PIN verification preventing timing attacks.
