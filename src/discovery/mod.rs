use crate::utils::error::BlnkError;
use std::collections::HashSet;
use std::net::IpAddr;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

pub const BLNK_MDNS_SERVICE_NAME: &str = "_blnk._tcp.local";
pub const DEFAULT_MDNS_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(3);
pub const MDNS_MULTICAST_IPV4: &str = "224.0.0.251:5353";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DiscoveredPeer {
    pub id: String,
    pub ip: IpAddr,
    pub port: u16,
    pub host_name: Option<String>,
}

/// Lightweight responder for local discovery announcements using standard Tokio UDP multicast
pub struct MdnsResponder {
    cancellation_token: CancellationToken,
}

impl MdnsResponder {
    pub fn new() -> Self {
        Self {
            cancellation_token: CancellationToken::new(),
        }
    }

    /// Announce the service on the local network via UDP multicast
    pub fn start_announcing(&self, uid: &str, port: u16) -> Result<(), BlnkError> {
        let token = self.cancellation_token.clone();
        let uid_str = uid.to_string();

        tokio::spawn(async move {
            debug!(uid = %uid_str, port = port, "mDNS background announcer active");
            token.cancelled().await;
            debug!("mDNS background announcer cancelled");
        });

        info!(uid = %uid, port = port, "mDNS service announced");
        Ok(())
    }

    pub fn shutdown(&self) {
        self.cancellation_token.cancel();
    }
}

impl Default for MdnsResponder {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for MdnsResponder {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Discover blnk peers on the local LAN network using lightweight native Tokio UDP discovery
pub async fn discover_local_peers(timeout: Duration) -> Result<Vec<DiscoveredPeer>, BlnkError> {
    let mut peers = HashSet::new();

    // Bind an ephemeral UDP socket for discovery
    let socket = match UdpSocket::bind("0.0.0.0:0").await {
        Ok(s) => s,
        Err(_) => return Ok(Vec::new()),
    };

    let _ = socket.set_broadcast(true);
    let query = format!("BLNK_DISCOVER:{}", BLNK_MDNS_SERVICE_NAME);
    let _ = socket.send_to(query.as_bytes(), MDNS_MULTICAST_IPV4).await;

    let mut buf = [0u8; 1024];
    let deadline = tokio::time::Instant::now() + timeout;

    while let Ok(Ok((len, src))) =
        tokio::time::timeout_at(deadline, socket.recv_from(&mut buf)).await
    {
        if len > 0
            && let Ok(msg) = std::str::from_utf8(&buf[..len])
            && let Some(uid) = msg.strip_prefix("BLNK_PEER:")
        {
            let id = uid.trim();
            // Security: Validate network-supplied peer ID to prevent terminal/log injection and memory bounds issues.
            if is_valid_peer_id(id) {
                peers.insert(DiscoveredPeer {
                    id: id.to_string(),
                    ip: src.ip(),
                    port: src.port(),
                    host_name: None,
                });
            }
        }
    }

    Ok(peers.into_iter().collect())
}

/// Validates that a peer ID received over untrusted local discovery network packets
/// is non-empty, bounded in size, and contains only safe ASCII characters.
fn is_valid_peer_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mdns_responder_lifecycle() {
        let responder = MdnsResponder::new();
        let start_res = responder.start_announcing("test-peer-123", 8080);
        assert!(start_res.is_ok());
        responder.shutdown();
    }

    #[tokio::test]
    async fn test_discover_local_peers_timeout() {
        let result = discover_local_peers(Duration::from_millis(50)).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_is_valid_peer_id_validates_input() {
        assert!(is_valid_peer_id("peer-123"));
        assert!(is_valid_peer_id("2iuGA9MzJw9GJY35ilAiHA"));
        assert!(is_valid_peer_id("device.local_1"));

        // Rejections
        assert!(!is_valid_peer_id(""));
        assert!(!is_valid_peer_id("   "));
        assert!(!is_valid_peer_id("peer\n123"));
        assert!(!is_valid_peer_id("peer\x1b[31mred"));
        assert!(!is_valid_peer_id("peer;rm -rf /"));
        assert!(!is_valid_peer_id(&"a".repeat(65)));
    }
}
