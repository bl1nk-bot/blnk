//! SWSP raw frame codec.
//!
//! The wire header is eight bytes in little-endian order:
//! `stream_id` (u32), `flags` (u16), and `payload_len` (u16), followed by
//! the payload. Protobuf messages in `proto/swsp.proto` remain schema/control
//! representations and are not used as the raw encoder.

use std::fmt;

use thiserror::Error;

/// The fixed size of an SWSP wire header.
pub const HEADER_LEN: usize = 8;

/// A conservative payload limit for a single data-channel frame.
///
/// The two-byte wire length permits up to 65,535 bytes, but the codec uses a
/// smaller default to leave room for SCTP/data-channel behavior. Callers can
/// select a lower limit with [`Frame::encode_with_limit`] and
/// [`Frame::decode_with_limit`].
pub const DEFAULT_MAX_PAYLOAD_LEN: usize = 16 * 1024;

const FLAG_SYN: u16 = 0x0001;
const FLAG_MORE: u16 = 0x0002;
const FLAG_FIN: u16 = 0x0004;
const FLAG_DAT: u16 = 0x0008;
const KNOWN_FLAGS: u16 = FLAG_SYN | FLAG_MORE | FLAG_FIN | FLAG_DAT;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FrameError {
    #[error("frame is incomplete: expected {expected} bytes, received {actual}")]
    Incomplete { expected: usize, actual: usize },

    #[error("frame contains unknown flag bits: 0x{bits:04x}")]
    UnknownFlags { bits: u16 },

    #[error("payload length {length} exceeds maximum {maximum}")]
    PayloadTooLarge { length: usize, maximum: usize },

    #[error("payload length {length} cannot be represented by the wire format")]
    LengthOverflow { length: usize },

    #[error("maximum payload length must be at most 65535 bytes")]
    InvalidMaximum,
}

/// Validated SWSP frame flags.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameFlags(u16);

impl FrameFlags {
    pub const NONE: Self = Self(0);
    pub const SYN: Self = Self(FLAG_SYN);
    pub const MORE: Self = Self(FLAG_MORE);
    pub const FIN: Self = Self(FLAG_FIN);
    pub const DAT: Self = Self(FLAG_DAT);

    pub fn from_bits(bits: u16) -> Result<Self, FrameError> {
        let unknown = bits & !KNOWN_FLAGS;
        if unknown != 0 {
            return Err(FrameError::UnknownFlags { bits: unknown });
        }
        Ok(Self(bits))
    }

    pub const fn bits(self) -> u16 {
        self.0
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub const fn is_syn(self) -> bool {
        self.contains(Self::SYN)
    }

    pub const fn is_more(self) -> bool {
        self.contains(Self::MORE)
    }

    pub const fn is_fin(self) -> bool {
        self.contains(Self::FIN)
    }

    pub const fn is_dat(self) -> bool {
        self.contains(Self::DAT)
    }
}

impl fmt::Display for FrameFlags {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "0x{:04x}", self.bits())
    }
}

/// A decoded SWSP frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub stream_id: u32,
    pub flags: FrameFlags,
    pub payload: Vec<u8>,
}

impl Frame {
    pub fn new(stream_id: u32, flags: FrameFlags, payload: Vec<u8>) -> Self {
        Self {
            stream_id,
            flags,
            payload,
        }
    }

    /// Encodes one frame using [`DEFAULT_MAX_PAYLOAD_LEN`].
    pub fn encode(&self) -> Result<Vec<u8>, FrameError> {
        self.encode_with_limit(DEFAULT_MAX_PAYLOAD_LEN)
    }

    /// Encodes one frame with a caller-selected payload limit.
    pub fn encode_with_limit(&self, maximum: usize) -> Result<Vec<u8>, FrameError> {
        validate_maximum(maximum)?;
        validate_payload_len(self.payload.len(), maximum)?;

        let payload_len =
            u16::try_from(self.payload.len()).map_err(|_| FrameError::LengthOverflow {
                length: self.payload.len(),
            })?;
        let mut encoded = Vec::with_capacity(HEADER_LEN + self.payload.len());
        encoded.extend_from_slice(&self.stream_id.to_le_bytes());
        encoded.extend_from_slice(&self.flags.bits().to_le_bytes());
        encoded.extend_from_slice(&payload_len.to_le_bytes());
        encoded.extend_from_slice(&self.payload);
        Ok(encoded)
    }

    /// Decodes the first frame from `input`, returning the frame and bytes consumed.
    ///
    /// Additional bytes are left for the caller to parse as a subsequent frame.
    /// An incomplete prefix is reported as [`FrameError::Incomplete`] so a
    /// stream reader can buffer it without treating it as malformed data.
    pub fn decode(input: &[u8]) -> Result<(Self, usize), FrameError> {
        Self::decode_with_limit(input, DEFAULT_MAX_PAYLOAD_LEN)
    }

    pub fn decode_with_limit(input: &[u8], maximum: usize) -> Result<(Self, usize), FrameError> {
        validate_maximum(maximum)?;
        if input.len() < HEADER_LEN {
            return Err(FrameError::Incomplete {
                expected: HEADER_LEN,
                actual: input.len(),
            });
        }

        let stream_id = u32::from_le_bytes([input[0], input[1], input[2], input[3]]);
        let flags = FrameFlags::from_bits(u16::from_le_bytes([input[4], input[5]]))?;
        let payload_len = usize::from(u16::from_le_bytes([input[6], input[7]]));
        validate_payload_len(payload_len, maximum)?;

        let frame_len = HEADER_LEN + payload_len;
        if input.len() < frame_len {
            return Err(FrameError::Incomplete {
                expected: frame_len,
                actual: input.len(),
            });
        }

        Ok((
            Self {
                stream_id,
                flags,
                payload: input[HEADER_LEN..frame_len].to_vec(),
            },
            frame_len,
        ))
    }
}

fn validate_maximum(maximum: usize) -> Result<(), FrameError> {
    if maximum > usize::from(u16::MAX) {
        return Err(FrameError::InvalidMaximum);
    }
    Ok(())
}

fn validate_payload_len(length: usize, maximum: usize) -> Result<(), FrameError> {
    if length > maximum {
        return Err(FrameError::PayloadTooLarge { length, maximum });
    }
    if length > usize::from(u16::MAX) {
        return Err(FrameError::LengthOverflow { length });
    }
    Ok(())
}

impl std::ops::BitOr for FrameFlags {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self(self.0 | rhs.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_preserves_header_and_payload() {
        let frame = Frame::new(
            7,
            FrameFlags::SYN | FrameFlags::DAT,
            b"{\"kind\":\"shell\"}".to_vec(),
        );

        let encoded = frame.encode().expect("frame should encode");
        assert_eq!(&encoded[..HEADER_LEN], &[7, 0, 0, 0, 9, 0, 16, 0]);
        let (decoded, consumed) = Frame::decode(&encoded).expect("frame should decode");
        assert_eq!(decoded, frame);
        assert_eq!(consumed, encoded.len());
    }

    #[test]
    fn decode_supports_multiple_frames_in_one_buffer() {
        let first = Frame::new(1, FrameFlags::DAT, b"one".to_vec())
            .encode()
            .expect("first frame should encode");
        let second = Frame::new(2, FrameFlags::FIN, b"two".to_vec())
            .encode()
            .expect("second frame should encode");
        let mut buffer = first.clone();
        buffer.extend_from_slice(&second);

        let (decoded, consumed) = Frame::decode(&buffer).expect("first frame should decode");
        assert_eq!(decoded.stream_id, 1);
        assert_eq!(consumed, first.len());
        let (decoded, consumed_second) =
            Frame::decode(&buffer[consumed..]).expect("second frame should decode");
        assert_eq!(decoded.stream_id, 2);
        assert_eq!(consumed_second, second.len());
    }

    #[test]
    fn incomplete_header_and_payload_are_distinguished() {
        assert_eq!(
            Frame::decode(&[1, 2, 3]).expect_err("header should be incomplete"),
            FrameError::Incomplete {
                expected: HEADER_LEN,
                actual: 3
            }
        );

        let mut encoded = Frame::new(1, FrameFlags::DAT, b"payload".to_vec())
            .encode()
            .expect("frame should encode");
        encoded.pop();
        assert_eq!(
            Frame::decode(&encoded).expect_err("payload should be incomplete"),
            FrameError::Incomplete {
                expected: HEADER_LEN + 7,
                actual: HEADER_LEN + 6
            }
        );
    }

    #[test]
    fn unknown_flags_are_rejected() {
        let mut encoded = Frame::new(1, FrameFlags::NONE, Vec::new())
            .encode()
            .expect("frame should encode");
        encoded[4] = 0x80;
        assert_eq!(
            Frame::decode(&encoded).expect_err("unknown flag should fail"),
            FrameError::UnknownFlags { bits: 0x0080 }
        );
    }

    #[test]
    fn maximum_payload_is_enforced_on_encode_and_decode() {
        let frame = Frame::new(1, FrameFlags::DAT, vec![0; 12]);
        assert_eq!(
            frame
                .encode_with_limit(8)
                .expect_err("payload should exceed custom limit"),
            FrameError::PayloadTooLarge {
                length: 12,
                maximum: 8
            }
        );

        let mut encoded = Frame::new(1, FrameFlags::DAT, vec![0; 12])
            .encode_with_limit(16)
            .expect("frame should encode with a larger limit");
        assert_eq!(
            Frame::decode_with_limit(&encoded, 8).expect_err("payload should exceed decode limit"),
            FrameError::PayloadTooLarge {
                length: 12,
                maximum: 8
            }
        );
        encoded[6..8].copy_from_slice(&u16::MAX.to_le_bytes());
        assert_eq!(
            Frame::decode(&encoded).expect_err("wire length should exceed default max"),
            FrameError::PayloadTooLarge {
                length: usize::from(u16::MAX),
                maximum: DEFAULT_MAX_PAYLOAD_LEN
            }
        );
    }

    #[test]
    fn all_canonical_flags_are_exposed() {
        let flags = FrameFlags::SYN | FrameFlags::MORE | FrameFlags::FIN | FrameFlags::DAT;
        assert!(flags.is_syn());
        assert!(flags.is_more());
        assert!(flags.is_fin());
        assert!(flags.is_dat());
        assert_eq!(
            FrameFlags::from_bits(flags.bits()).expect("flags are valid"),
            flags
        );
    }

    #[test]
    fn invalid_maximum_is_rejected() {
        let frame = Frame::new(0, FrameFlags::NONE, Vec::new());
        assert_eq!(
            frame
                .encode_with_limit(usize::from(u16::MAX) + 1)
                .expect_err("maximum should be rejected"),
            FrameError::InvalidMaximum
        );
    }
}
