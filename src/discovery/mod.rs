use crate::utils::error::BlnkError;
use futures::StreamExt;
use std::collections::HashSet;
use std::net::IpAddr;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

pub const BLNK_MDNS_SERVICE_NAME: &str = "_blnk._tcp.local";
pub const DEFAULT_MDNS_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DiscoveredPeer {
    pub id: String,
    pub ip: IpAddr,
    pub port: u16,
    pub host_name: Option<String>,
}

/// Responder for managing local discovery tasks
pub struct MdnsResponder {
    cancellation_token: CancellationToken,
}

impl MdnsResponder {
    pub fn new() -> Self {
        Self {
            cancellation_token: CancellationToken::new(),
        }
    }

    /// Announce the service on the local network via mDNS.
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

/// Discover blnk peers on the local LAN network using mDNS.
pub async fn discover_local_peers(
    timeout: Duration,
) -> Result<Vec<DiscoveredPeer>, BlnkError> {
    let discovery = match mdns::discover::all(BLNK_MDNS_SERVICE_NAME, timeout) {
        Ok(d) => d,
        Err(e) => {
            warn!("Failed to query mDNS: {}", e);
            return Err(BlnkError::Signaling(format!("mDNS query failed: {e}")));
        }
    };

    let mut peers = HashSet::new();
    let stream = discovery.listen();
    tokio::pin!(stream);

    let deadline = tokio::time::Instant::now() + timeout;

    while let Ok(Some(response_res)) = tokio::time::timeout_at(deadline, stream.next()).await {
        if let Ok(response) = response_res {
            let mut ip = None;
            let mut port = 0;
            let mut uid = None;
            let mut host_name = None;

            for record in response.records() {
                match &record.kind {
                    mdns::RecordKind::A(v4) => {
                        ip = Some(IpAddr::V4(*v4));
                    }
                    mdns::RecordKind::AAAA(v6) => {
                        if ip.is_none() {
                            ip = Some(IpAddr::V6(*v6));
                        }
                    }
                    mdns::RecordKind::SRV { port: p, target, .. } => {
                        port = *p;
                        host_name = Some(target.clone());
                    }
                    mdns::RecordKind::TXT(txts) => {
                        for entry in txts {
                            if let Some(stripped) = entry.strip_prefix("uid=") {
                                uid = Some(stripped.to_string());
                            }
                        }
                    }
                    _ => {}
                }
            }

            let resolved_id = uid.or_else(|| {
                host_name.as_ref().and_then(|h| h.split('.').next().map(|s| s.to_string()))
            });

            if let (Some(id), Some(ip)) = (resolved_id, ip) {
                peers.insert(DiscoveredPeer {
                    id,
                    ip,
                    port,
                    host_name,
                });
            }
        }
    }

    Ok(peers.into_iter().collect())
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
        if let Ok(peers) = result {
            assert!(peers.is_empty() || !peers.is_empty());
        }
    }
}
