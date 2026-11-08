//! Proto type stubs for blnk-tui.
//!
//! These types mirror the protobuf definitions in `proto/workspace.proto`.
//! In production, import from blnk core crate instead of using these stubs.
//! See `build.rs` in the blnk crate for prost codegen.
#![allow(dead_code)]

pub mod blnk {
    pub mod workspace {
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub enum WorkspaceMessageType {
            Unspecified = 0,
            Headline = 1,
            Announcement = 2,
            System = 3,
        }

        impl WorkspaceMessageType {
            pub fn from_i32(value: i32) -> Self {
                match value {
                    1 => Self::Headline,
                    2 => Self::Announcement,
                    3 => Self::System,
                    _ => Self::Unspecified,
                }
            }
        }

        #[derive(Clone, Debug, Default)]
        pub struct WorkspaceMessage {
            pub message_id: String,
            pub message_type: i32,
            pub message_body: String,
            pub created_at: i64,
            pub created_by_device_id: String,
        }

        impl WorkspaceMessage {
            pub fn message_type(&self) -> WorkspaceMessageType {
                WorkspaceMessageType::from_i32(self.message_type)
            }
        }

        #[derive(Clone, Debug, Default)]
        pub struct GetWorkspaceMessagesRequest {
            pub workspace_id: String,
            pub limit: u32,
        }

        #[derive(Clone, Debug, Default)]
        pub struct GetWorkspaceMessagesResponse {
            pub feature_enabled: bool,
            pub messages: Vec<WorkspaceMessage>,
        }
    }
}
