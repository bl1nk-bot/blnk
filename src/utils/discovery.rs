use std::net::IpAddr;
use std::time::Duration;

use crate::utils::error::BlnkError;

pub const BLNK_MDNS_SERVICE_TYPE: &str = "_blnk._tcp.local";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredPeer {
    pub id: String,
    pub endpoint: String,
    pub ip: Option<IpAddr>,
    pub port: Option<u16>,
}

/// Discovers blnk peers on the local network via mDNS for the specified duration.
pub async fn discover_local_peers(
    timeout_duration: Duration,
) -> Result<Vec<DiscoveredPeer>, BlnkError> {
    let mut peers = Vec::new();
    let service_name = BLNK_MDNS_SERVICE_TYPE;

    let response = mdns::discover::all(service_name, timeout_duration)
        .map_err(|e| BlnkError::Signaling(format!("mDNS discovery failed: {e}")))?;

    // We collect records safely bounded by timeout
    let stream = response.listen();
    tokio::pin!(stream);

    let start = std::time::Instant::now();
    while start.elapsed() < timeout_duration {
        let sleep_left = timeout_duration.saturating_sub(start.elapsed());
        match tokio::time::timeout(sleep_left, futures::StreamExt::next(&mut stream)).await {
            Ok(Some(Ok(response))) => {
                let addr = response.socket_address();
                if let Some(socket_addr) = addr {
                    let host = socket_addr.ip().to_string();
                    let port = socket_addr.port();
                    let id = response
                        .hostname()
                        .map(|h| h.trim_end_matches('.').to_string())
                        .unwrap_or_else(|| format!("peer-{}", host));

                    let endpoint = format!("ws://{}:{}", host, port);
                    peers.push(DiscoveredPeer {
                        id,
                        endpoint,
                        ip: Some(socket_addr.ip()),
                        port: Some(port),
                    });
                }
            }
            Ok(Some(Err(_))) | Ok(None) | Err(_) => break,
        }
    }

    Ok(peers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_discover_local_peers_short_timeout() {
        let peers = discover_local_peers(Duration::from_millis(50)).await;
        assert!(peers.is_ok());
    }
}
