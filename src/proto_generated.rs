//! Generated protobuf bindings.
//!
//! This namespace is intentionally separate from the handwritten protocol,
//! signaling, session, and stream boundaries. The generated messages provide a
//! schema representation only; wire compatibility still requires fixtures and
//! end-to-end evidence.

pub mod control {
    include!(concat!(env!("OUT_DIR"), "/control.rs"));
}

pub mod adapter {
    include!(concat!(env!("OUT_DIR"), "/blnk.adapter.rs"));
}

pub mod common {
    include!(concat!(env!("OUT_DIR"), "/blnk.common.rs"));
}

pub mod identity {
    include!(concat!(env!("OUT_DIR"), "/identity.rs"));
}

pub mod object {
    include!(concat!(env!("OUT_DIR"), "/blnk.object.rs"));
}

pub mod pairing {
    include!(concat!(env!("OUT_DIR"), "/pairing.rs"));
}

pub mod share {
    include!(concat!(env!("OUT_DIR"), "/blnk.share.rs"));
}

pub mod signaling {
    include!(concat!(env!("OUT_DIR"), "/signaling.rs"));
}

pub mod storage {
    include!(concat!(env!("OUT_DIR"), "/blnk.storage.rs"));
}

pub mod stream {
    include!(concat!(env!("OUT_DIR"), "/stream.rs"));
}

pub mod swsp {
    include!(concat!(env!("OUT_DIR"), "/swsp.rs"));
}

pub mod sync {
    include!(concat!(env!("OUT_DIR"), "/blnk.sync.rs"));
}

pub mod vault {
    include!(concat!(env!("OUT_DIR"), "/blnk.vault.rs"));
}

pub mod workspace {
    include!(concat!(env!("OUT_DIR"), "/blnk.workspace.rs"));
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::{
        adapter, common, control, identity, object, pairing, share, signaling, storage, stream,
        swsp, sync, vault, workspace,
    };

    #[test]
    fn generated_messages_encode_and_decode_across_all_packages() {
        let object = object::ObjectRecord {
            id: "object-1".to_owned(),
            kind: object::ObjectKind::McpServer as i32,
            metadata: Some(object::ObjectMetadata {
                title: "filesystem".to_owned(),
                sensitivity: common::Sensitivity::Sensitive as i32,
                ..Default::default()
            }),
            payload_ref: "vault:object-1-r1".to_owned(),
            ..Default::default()
        };
        let object_bytes = object.encode_to_vec();
        let decoded_object = object::ObjectRecord::decode(object_bytes.as_slice())
            .expect("generated object message must decode");
        assert_eq!(decoded_object.id, "object-1");

        let vault_record = vault::VaultRecord {
            payload_ref: "vault:object-1-r1".to_owned(),
            object_id: decoded_object.id.clone(),
            key_version: 1,
            ciphertext: vec![1, 2, 3],
            ..Default::default()
        };
        let share = share::ShareEnvelope {
            share_id: "share-1".to_owned(),
            objects: vec![decoded_object.clone()],
            encrypted_payload: vault_record.encode_to_vec(),
            ..Default::default()
        };
        let share_bytes = share.encode_to_vec();
        let decoded_share = share::ShareEnvelope::decode(share_bytes.as_slice())
            .expect("generated share message must decode");
        assert_eq!(decoded_share.objects[0].id, "object-1");

        let sync_change = sync::SyncChange {
            object_id: "object-1".to_owned(),
            revision: 1,
            operation: sync::SyncOperation::Upsert as i32,
            ..Default::default()
        };
        assert_eq!(sync_change.object_id, "object-1");

        let adapter_caps = adapter::AdapterCapabilities {
            can_import: true,
            can_apply: true,
            supported_kinds: vec!["mcp.server".to_owned()],
            ..Default::default()
        };
        assert!(adapter_caps.can_apply);

        let workspace = workspace::WorkspacePack {
            id: "pack-1".to_owned(),
            name: "Research".to_owned(),
            object_ids: vec!["object-1".to_owned()],
            ..Default::default()
        };
        assert_eq!(workspace.object_ids.len(), 1);

        let binding = storage::AppBinding {
            app_id: "claude".to_owned(),
            object_id: "object-1".to_owned(),
            enabled: true,
            ..Default::default()
        };
        assert!(binding.enabled);

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
