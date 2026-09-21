//! Workspace message headline fetching.
//!
//! Fetches workspace headlines via the protobuf message protocol defined in
//! `proto/workspace.proto`. Headlines are short text banners displayed in
//! the TUI header area.
#![allow(dead_code)]

use std::time::Duration;

use crate::proto::blnk::workspace::GetWorkspaceMessagesResponse;
use crate::proto::blnk::workspace::WorkspaceMessageType;

pub(crate) const WORKSPACE_HEADLINE_REFRESH_INTERVAL: Duration = Duration::from_secs(5 * 60);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum WorkspaceHeadlineFetchResult {
    Available(Option<String>),
    FeatureDisabled,
}

pub(crate) fn workspace_headline_from_response(
    response: GetWorkspaceMessagesResponse,
) -> WorkspaceHeadlineFetchResult {
    if !response.feature_enabled {
        return WorkspaceHeadlineFetchResult::FeatureDisabled;
    }

    WorkspaceHeadlineFetchResult::Available(response.messages.into_iter().find_map(|message| {
        (message.message_type() == WorkspaceMessageType::Headline)
            .then(|| message.message_body.trim().to_string())
            .filter(|headline| !headline.is_empty())
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headline_extracted_when_feature_enabled() {
        let response = GetWorkspaceMessagesResponse {
            feature_enabled: true,
            messages: vec![crate::proto::blnk::workspace::WorkspaceMessage {
                message_id: "1".into(),
                message_type: WorkspaceMessageType::Headline as i32,
                message_body: "  Welcome to workspace  ".into(),
                created_at: 0,
                created_by_device_id: String::new(),
            }],
        };

        assert_eq!(
            workspace_headline_from_response(response),
            WorkspaceHeadlineFetchResult::Available(Some("Welcome to workspace".into()))
        );
    }

    #[test]
    fn empty_headline_filtered() {
        let response = GetWorkspaceMessagesResponse {
            feature_enabled: true,
            messages: vec![crate::proto::blnk::workspace::WorkspaceMessage {
                message_id: "1".into(),
                message_type: WorkspaceMessageType::Headline as i32,
                message_body: "   ".into(),
                created_at: 0,
                created_by_device_id: String::new(),
            }],
        };

        assert_eq!(
            workspace_headline_from_response(response),
            WorkspaceHeadlineFetchResult::Available(None)
        );
    }

    #[test]
    fn feature_disabled_returns_disabled() {
        let response = GetWorkspaceMessagesResponse {
            feature_enabled: false,
            messages: vec![],
        };

        assert_eq!(
            workspace_headline_from_response(response),
            WorkspaceHeadlineFetchResult::FeatureDisabled
        );
    }

    #[test]
    fn non_headline_messages_ignored() {
        let response = GetWorkspaceMessagesResponse {
            feature_enabled: true,
            messages: vec![
                crate::proto::blnk::workspace::WorkspaceMessage {
                    message_id: "1".into(),
                    message_type: WorkspaceMessageType::Announcement as i32,
                    message_body: "This is an announcement".into(),
                    created_at: 0,
                    created_by_device_id: String::new(),
                },
                crate::proto::blnk::workspace::WorkspaceMessage {
                    message_id: "2".into(),
                    message_type: WorkspaceMessageType::Headline as i32,
                    message_body: "Real headline".into(),
                    created_at: 0,
                    created_by_device_id: String::new(),
                },
            ],
        };

        assert_eq!(
            workspace_headline_from_response(response),
            WorkspaceHeadlineFetchResult::Available(Some("Real headline".into()))
        );
    }
}
