//! Unified stream message envelope codec.
//!
//! The envelope wraps per-stream-type protobuf payloads with dispatch metadata
//! (stream type, message kind, sequence number). This provides type-safe
//! routing without relying solely on SWSP frame flags.
//!
//! Existing handlers may continue using raw SWSP frames. The envelope is
//! opt-in for new code that benefits from typed dispatch.

use prost::Message;

use crate::proto_generated::stream as wire;
use crate::protocol::swsp::{Frame, FrameFlags};
use crate::stream::{StreamKind, StreamResult};
use crate::utils::error::BlnkError;

/// Typed representation of a stream message envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamEnvelope {
    pub stream_type: StreamTypeTag,
    pub kind: MessageKind,
    pub sequence: u32,
    pub payload: Vec<u8>,
}

/// Stream type tag for dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamTypeTag {
    Http,
    File,
    Tcp,
    WebSocket,
    Shell,
}

impl StreamTypeTag {
    pub fn as_wire(self) -> wire::StreamType {
        match self {
            Self::Http => wire::StreamType::Http,
            Self::File => wire::StreamType::File,
            Self::Tcp => wire::StreamType::Tcp,
            Self::WebSocket => wire::StreamType::Websocket,
            Self::Shell => wire::StreamType::Shell,
        }
    }

    pub fn from_wire(value: i32) -> StreamResult<Self> {
        match wire::StreamType::try_from(value) {
            Ok(wire::StreamType::Http) => Ok(Self::Http),
            Ok(wire::StreamType::File) => Ok(Self::File),
            Ok(wire::StreamType::Tcp) => Ok(Self::Tcp),
            Ok(wire::StreamType::Websocket) => Ok(Self::WebSocket),
            Ok(wire::StreamType::Shell) => Ok(Self::Shell),
            Ok(wire::StreamType::Unspecified) | Err(_) => {
                Err(BlnkError::Protocol("unknown stream type tag".into()))
            }
        }
    }

    pub fn from_kind(kind: StreamKind) -> StreamResult<Self> {
        match kind {
            StreamKind::Http => Ok(Self::Http),
            StreamKind::File => Ok(Self::File),
            StreamKind::Tcp => Ok(Self::Tcp),
            StreamKind::WebSocket => Ok(Self::WebSocket),
            StreamKind::Shell => Ok(Self::Shell),
            StreamKind::Adapter => Err(BlnkError::Protocol(
                "adapter stream has no wire envelope yet".into(),
            )),
        }
    }
}

/// Semantic message kind for dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageKind {
    Open,
    Data,
    Close,
    Error,
    Cancel,
    Resize,
    Metadata,
}

impl MessageKind {
    pub fn as_wire(self) -> wire::StreamMessageKind {
        match self {
            Self::Open => wire::StreamMessageKind::Open,
            Self::Data => wire::StreamMessageKind::Data,
            Self::Close => wire::StreamMessageKind::Close,
            Self::Error => wire::StreamMessageKind::Error,
            Self::Cancel => wire::StreamMessageKind::Cancel,
            Self::Resize => wire::StreamMessageKind::Resize,
            Self::Metadata => wire::StreamMessageKind::Metadata,
        }
    }

    pub fn from_wire(value: i32) -> StreamResult<Self> {
        match wire::StreamMessageKind::try_from(value) {
            Ok(wire::StreamMessageKind::Open) => Ok(Self::Open),
            Ok(wire::StreamMessageKind::Data) => Ok(Self::Data),
            Ok(wire::StreamMessageKind::Close) => Ok(Self::Close),
            Ok(wire::StreamMessageKind::Error) => Ok(Self::Error),
            Ok(wire::StreamMessageKind::Cancel) => Ok(Self::Cancel),
            Ok(wire::StreamMessageKind::Resize) => Ok(Self::Resize),
            Ok(wire::StreamMessageKind::Metadata) => Ok(Self::Metadata),
            Ok(wire::StreamMessageKind::Unspecified) | Err(_) => {
                Err(BlnkError::Protocol("unknown stream message kind".into()))
            }
        }
    }
}

impl StreamEnvelope {
    /// Creates a new envelope.
    pub fn new(
        stream_type: StreamTypeTag,
        kind: MessageKind,
        sequence: u32,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            stream_type,
            kind,
            sequence,
            payload,
        }
    }

    /// Encodes the envelope to protobuf bytes (for use as SWSP frame payload).
    pub fn encode_proto(&self) -> Vec<u8> {
        wire::StreamMessage {
            stream_type: self.stream_type.as_wire() as i32,
            kind: self.kind.as_wire() as i32,
            sequence: self.sequence,
            payload: self.payload.clone(),
        }
        .encode_to_vec()
    }

    /// Decodes an envelope from protobuf bytes.
    pub fn decode_proto(bytes: &[u8]) -> StreamResult<Self> {
        let message = wire::StreamMessage::decode(bytes)
            .map_err(|e| BlnkError::Protocol(format!("invalid stream message protobuf: {e}")))?;
        Ok(Self {
            stream_type: StreamTypeTag::from_wire(message.stream_type)?,
            kind: MessageKind::from_wire(message.kind)?,
            sequence: message.sequence,
            payload: message.payload,
        })
    }

    /// Wraps the envelope into an SWSP frame for a given stream_id.
    pub fn to_frame(&self, stream_id: u32) -> StreamResult<Frame> {
        if stream_id == 0 {
            return Err(BlnkError::Stream(
                "stream message stream_id must be non-zero".into(),
            ));
        }
        let flags = match self.kind {
            MessageKind::Open => FrameFlags::SYN | FrameFlags::DAT,
            MessageKind::Close | MessageKind::Error | MessageKind::Cancel => {
                FrameFlags::DAT | FrameFlags::FIN
            }
            MessageKind::Data | MessageKind::Resize | MessageKind::Metadata => FrameFlags::DAT,
        };
        Ok(Frame::new(stream_id, flags, self.encode_proto()))
    }

    /// Extracts the envelope from an SWSP data frame (stream_id > 0).
    pub fn from_frame(frame: &Frame) -> StreamResult<Self> {
        if frame.stream_id == 0 {
            return Err(BlnkError::Protocol(
                "stream envelope cannot be extracted from control frame".into(),
            ));
        }
        Self::decode_proto(&frame.payload)
    }

    /// Infers the message kind from SWSP frame flags for backward compatibility.
    pub fn infer_kind_from_flags(flags: FrameFlags) -> MessageKind {
        if flags.is_syn() {
            MessageKind::Open
        } else if flags.is_fin() {
            MessageKind::Close
        } else {
            MessageKind::Data
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_round_trips_through_protobuf() {
        let envelope = StreamEnvelope::new(
            StreamTypeTag::Shell,
            MessageKind::Open,
            1,
            b"test-payload".to_vec(),
        );
        let encoded = envelope.encode_proto();
        let decoded = StreamEnvelope::decode_proto(&encoded).expect("decode should succeed");
        assert_eq!(decoded, envelope);
    }

    #[test]
    fn envelope_round_trips_through_swsp_frame() {
        let envelope = StreamEnvelope::new(
            StreamTypeTag::File,
            MessageKind::Data,
            42,
            b"file-data".to_vec(),
        );
        let frame = envelope.to_frame(5).expect("frame should build");
        assert_eq!(frame.stream_id, 5);
        assert!(frame.flags.is_dat());
        assert!(!frame.flags.is_syn());
        assert!(!frame.flags.is_fin());

        let recovered = StreamEnvelope::from_frame(&frame).expect("extract should succeed");
        assert_eq!(recovered, envelope);
    }

    #[test]
    fn open_kind_produces_syn_dat_flags() {
        let envelope = StreamEnvelope::new(StreamTypeTag::Shell, MessageKind::Open, 0, Vec::new());
        let frame = envelope.to_frame(1).expect("frame");
        assert!(frame.flags.is_syn());
        assert!(frame.flags.is_dat());
        assert!(!frame.flags.is_fin());
    }

    #[test]
    fn close_kind_produces_dat_fin_flags() {
        let envelope = StreamEnvelope::new(StreamTypeTag::Shell, MessageKind::Close, 0, Vec::new());
        let frame = envelope.to_frame(1).expect("frame");
        assert!(!frame.flags.is_syn());
        assert!(frame.flags.is_dat());
        assert!(frame.flags.is_fin());
    }

    #[test]
    fn control_stream_id_is_rejected() {
        let envelope = StreamEnvelope::new(StreamTypeTag::Shell, MessageKind::Data, 0, Vec::new());
        assert!(envelope.to_frame(0).is_err());
    }

    #[test]
    fn from_frame_rejects_control_frame() {
        let frame = Frame::new(0, FrameFlags::DAT, Vec::new());
        assert!(StreamEnvelope::from_frame(&frame).is_err());
    }

    #[test]
    fn stream_type_tag_conversions() {
        for kind in [
            StreamKind::Http,
            StreamKind::File,
            StreamKind::Tcp,
            StreamKind::WebSocket,
            StreamKind::Shell,
        ] {
            let tag = StreamTypeTag::from_kind(kind).expect("supported kind");
            let wire_val = tag.as_wire() as i32;
            let round_tripped = StreamTypeTag::from_wire(wire_val).expect("round trip");
            assert_eq!(round_tripped, tag);
        }
    }

    #[test]
    fn message_kind_wire_round_trip() {
        for kind in [
            MessageKind::Open,
            MessageKind::Data,
            MessageKind::Close,
            MessageKind::Error,
            MessageKind::Cancel,
            MessageKind::Resize,
            MessageKind::Metadata,
        ] {
            let wire_val = kind.as_wire() as i32;
            let round_tripped = MessageKind::from_wire(wire_val).expect("round trip");
            assert_eq!(round_tripped, kind);
        }
    }

    #[test]
    fn infer_kind_from_flags_matches_encode() {
        let open = StreamEnvelope::new(StreamTypeTag::Shell, MessageKind::Open, 0, Vec::new());
        let frame = open.to_frame(1).expect("frame");
        assert_eq!(
            StreamEnvelope::infer_kind_from_flags(frame.flags),
            MessageKind::Open
        );

        let data = StreamEnvelope::new(StreamTypeTag::Shell, MessageKind::Data, 1, Vec::new());
        let frame = data.to_frame(1).expect("frame");
        assert_eq!(
            StreamEnvelope::infer_kind_from_flags(frame.flags),
            MessageKind::Data
        );

        let close = StreamEnvelope::new(StreamTypeTag::Shell, MessageKind::Close, 2, Vec::new());
        let frame = close.to_frame(1).expect("frame");
        assert_eq!(
            StreamEnvelope::infer_kind_from_flags(frame.flags),
            MessageKind::Close
        );
    }
}
