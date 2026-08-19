//! Generated protobuf bindings.
//!
//! This namespace is intentionally separate from the handwritten protocol,
//! signaling, session, and stream boundaries. The generated messages provide a
//! schema representation only; wire compatibility still requires fixtures and
//! end-to-end evidence.

pub mod control {
    include!(concat!(env!("OUT_DIR"), "/control.rs"));
}

pub mod identity {
    include!(concat!(env!("OUT_DIR"), "/identity.rs"));
}

pub mod pairing {
    include!(concat!(env!("OUT_DIR"), "/pairing.rs"));
}

pub mod signaling {
    include!(concat!(env!("OUT_DIR"), "/signaling.rs"));
}

pub mod stream {
    include!(concat!(env!("OUT_DIR"), "/stream.rs"));
}

pub mod swsp {
    include!(concat!(env!("OUT_DIR"), "/swsp.rs"));
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::{control, identity, pairing, signaling, stream, swsp};

    #[test]
    fn generated_messages_encode_and_decode_across_all_packages() {
        let register = signaling::RegisterRequest {
            uid: "0123456789012345678901".to_owned(),
            protocol: 3,
            ..Default::default()
        };
        let register_bytes = register.encode_to_vec();
        let decoded_register = signaling::RegisterRequest::decode(register_bytes.as_slice())
            .expect("generated signaling message must decode");
        assert_eq!(decoded_register.uid, register.uid);
        assert_eq!(decoded_register.protocol, 3);

        let identity = identity::Identity {
            uid: decoded_register.uid,
            code: "access-code".to_owned(),
            ..Default::default()
        };
        let identity_bytes = identity.encode_to_vec();
        let decoded_identity = identity::Identity::decode(identity_bytes.as_slice())
            .expect("generated identity message must decode");
        assert_eq!(decoded_identity.code, "access-code");

        let pairing = pairing::PairCommit {
            commit: "commit".to_owned(),
            ..Default::default()
        };
        let pairing_bytes = pairing.encode_to_vec();
        let decoded_pairing = pairing::PairCommit::decode(pairing_bytes.as_slice())
            .expect("generated pairing message must decode");
        assert_eq!(decoded_pairing.commit, "commit");

        let control = control::SessionConfig {
            pin_required: true,
            max_auth_fails: 3,
            pin_fail_delay_ms: 2_000,
        };
        let control_bytes = control.encode_to_vec();
        let decoded_control = control::SessionConfig::decode(control_bytes.as_slice())
            .expect("generated control message must decode");
        assert!(decoded_control.pin_required);

        let stream = stream::HttpRequest {
            method: "GET".to_owned(),
            pathname: "/".to_owned(),
            ..Default::default()
        };
        let stream_bytes = stream.encode_to_vec();
        let decoded_stream = stream::HttpRequest::decode(stream_bytes.as_slice())
            .expect("generated stream message must decode");
        assert_eq!(decoded_stream.pathname, "/");

        let swsp = swsp::Frame {
            stream_id: 1,
            flags: 1,
            length: 3,
            payload: vec![1, 2, 3],
        };
        let swsp_bytes = swsp.encode_to_vec();
        let decoded_swsp =
            swsp::Frame::decode(swsp_bytes.as_slice()).expect("generated SWSP message must decode");
        assert_eq!(decoded_swsp.payload, vec![1, 2, 3]);
    }
}
