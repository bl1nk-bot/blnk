//! WebRTC peer lifecycle and local two-peer test harness.
//!
//! This module owns the WebRTC connection, ICE/SDP lifecycle, data-channel
//! events, and conversion between binary data-channel messages and SWSP frames.
//! Signaling remains outside this module: the local harness exchanges the
//! non-trickle SDP descriptions in process, while production callers can use
//! the same [`PeerHandle`] methods with the signaling transport.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_channel::{Receiver, Sender};
use async_trait::async_trait;
use bytes::BytesMut;
use tokio::sync::{Mutex, Notify};
use webrtc::data_channel::{DataChannel, DataChannelEvent};
use webrtc::peer_connection::{
    MediaEngine, PeerConnection, PeerConnectionBuilder, PeerConnectionEventHandler,
    RTCConfigurationBuilder, RTCIceConnectionState, RTCIceGatheringState,
    RTCPeerConnectionIceErrorEvent, RTCPeerConnectionIceEvent, RTCPeerConnectionState,
    RTCSignalingState, Registry, register_default_interceptors,
};
use webrtc::runtime::TokioRuntime;

use crate::protocol::swsp::Frame;
use crate::utils::error::BlnkError;

const DEFAULT_WAIT_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_UDP_BIND: &str = "127.0.0.1:0";

/// A WebRTC peer with a binary SWSP data-channel boundary.
pub struct PeerHandle {
    connection: Arc<dyn PeerConnection>,
    events: Arc<PeerEvents>,
    inbound_rx: Receiver<Vec<u8>>,
}

struct PeerEvents {
    inbound_tx: Sender<Vec<u8>>,
    channel: Mutex<Option<Arc<dyn DataChannel>>>,
    channel_open: Arc<Notify>,
    channel_opened: Arc<AtomicBool>,
    channel_state_changed: Arc<Notify>,
    channel_closed: Arc<AtomicBool>,
    channel_error: Arc<Mutex<Option<String>>>,
    connected: Arc<Notify>,
    connected_state: Arc<AtomicBool>,
    connection_state_changed: Arc<Notify>,
    connection_failure: Arc<Mutex<Option<String>>>,
    gathering_complete: Arc<Notify>,
    gathering_done: Arc<AtomicBool>,
}

#[async_trait]
impl PeerConnectionEventHandler for PeerEvents {
    async fn on_ice_gathering_state_change(&self, state: RTCIceGatheringState) {
        if state == RTCIceGatheringState::Complete {
            self.gathering_done.store(true, Ordering::Release);
            self.gathering_complete.notify_waiters();
        }
    }

    async fn on_connection_state_change(&self, state: RTCPeerConnectionState) {
        tracing::debug!(?state, "WebRTC peer connection state changed");
        match state {
            RTCPeerConnectionState::Connected => {
                self.connected_state.store(true, Ordering::Release);
            }
            RTCPeerConnectionState::Failed | RTCPeerConnectionState::Closed => {
                let mut failure = self.connection_failure.lock().await;
                *failure = Some(format!("peer connection entered {state:?} state"));
            }
            _ => {}
        }
        self.connection_state_changed.notify_waiters();
        if state == RTCPeerConnectionState::Connected {
            self.connected.notify_waiters();
        }
    }

    async fn on_ice_connection_state_change(&self, state: RTCIceConnectionState) {
        tracing::debug!(?state, "WebRTC ICE connection state changed");
        if state == RTCIceConnectionState::Failed {
            let mut failure = self.connection_failure.lock().await;
            *failure = Some("ICE connection entered Failed state".to_owned());
            self.connection_state_changed.notify_waiters();
        }
    }

    async fn on_data_channel(&self, channel: Arc<dyn DataChannel>) {
        self.install_channel(channel).await;
    }

    async fn on_ice_candidate(&self, _event: RTCPeerConnectionIceEvent) {}

    async fn on_ice_candidate_error(&self, _event: RTCPeerConnectionIceErrorEvent) {}

    async fn on_signaling_state_change(&self, _state: RTCSignalingState) {}
}

impl PeerEvents {
    async fn install_channel(&self, channel: Arc<dyn DataChannel>) {
        {
            let mut slot = self.channel.lock().await;
            if slot.is_some() {
                return;
            }
            *slot = Some(channel.clone());
        }

        let inbound_tx = self.inbound_tx.clone();
        let channel_open = self.channel_open.clone();
        let channel_opened = self.channel_opened.clone();
        let channel_state_changed = self.channel_state_changed.clone();
        let channel_closed = self.channel_closed.clone();
        let channel_error = self.channel_error.clone();
        tokio::spawn(async move {
            loop {
                match channel.poll().await {
                    Some(DataChannelEvent::OnOpen) => {
                        tracing::debug!("WebRTC data channel opened");
                        channel_opened.store(true, Ordering::Release);
                        channel_state_changed.notify_waiters();
                        channel_open.notify_waiters();
                    }
                    Some(DataChannelEvent::OnMessage(message)) => {
                        if !message.is_string {
                            let _ = inbound_tx.send(message.data.to_vec()).await;
                        }
                    }
                    Some(DataChannelEvent::OnError) => {
                        tracing::debug!("WebRTC data channel reported an error");
                        let mut error = channel_error.lock().await;
                        *error = Some("data channel reported an error".to_owned());
                        channel_state_changed.notify_waiters();
                    }
                    Some(DataChannelEvent::OnClose) | None => {
                        tracing::debug!("WebRTC data channel closed");
                        channel_closed.store(true, Ordering::Release);
                        channel_state_changed.notify_waiters();
                        inbound_tx.close();
                        break;
                    }
                    Some(DataChannelEvent::OnClosing)
                    | Some(DataChannelEvent::OnBufferedAmountLow)
                    | Some(DataChannelEvent::OnBufferedAmountHigh) => {}
                }
            }
        });
    }
}

impl PeerHandle {
    /// Builds a peer using loopback UDP sockets and the Tokio runtime.
    pub async fn new() -> Result<Self, BlnkError> {
        let (inbound_tx, inbound_rx) = async_channel::bounded(32);
        let events = Arc::new(PeerEvents {
            inbound_tx,
            channel: Mutex::new(None),
            channel_open: Arc::new(Notify::new()),
            channel_opened: Arc::new(AtomicBool::new(false)),
            channel_state_changed: Arc::new(Notify::new()),
            channel_closed: Arc::new(AtomicBool::new(false)),
            channel_error: Arc::new(Mutex::new(None)),
            connected: Arc::new(Notify::new()),
            connected_state: Arc::new(AtomicBool::new(false)),
            connection_state_changed: Arc::new(Notify::new()),
            connection_failure: Arc::new(Mutex::new(None)),
            gathering_complete: Arc::new(Notify::new()),
            gathering_done: Arc::new(AtomicBool::new(false)),
        });

        let mut media = MediaEngine::default();
        media
            .register_default_codecs()
            .map_err(|error| peer_error("register codecs", error))?;
        let registry = register_default_interceptors(Registry::new(), &mut media)
            .map_err(|error| peer_error("register interceptors", error))?;

        let connection = PeerConnectionBuilder::new()
            .with_configuration(RTCConfigurationBuilder::new().build())
            .with_media_engine(media)
            .with_interceptor_registry(registry)
            .with_handler(events.clone())
            .with_runtime(Arc::new(TokioRuntime))
            .with_udp_addrs(vec![DEFAULT_UDP_BIND.to_owned()])
            .build()
            .await
            .map_err(|error| peer_error("build peer connection", error))?;

        Ok(Self {
            connection: Arc::new(connection),
            events,
            inbound_rx,
        })
    }

    /// Creates the application data channel and starts its event pump.
    pub async fn create_data_channel(&self, label: &str) -> Result<(), BlnkError> {
        let channel = self
            .connection
            .create_data_channel(label, None)
            .await
            .map_err(|error| peer_error("create data channel", error))?;
        self.events.install_channel(channel).await;
        Ok(())
    }

    /// Creates and sets a local SDP offer, then waits for non-trickle ICE.
    pub async fn create_offer(
        &self,
    ) -> Result<webrtc::peer_connection::RTCSessionDescription, BlnkError> {
        let offer = self
            .connection
            .create_offer(None)
            .await
            .map_err(|error| peer_error("create offer", error))?;
        self.connection
            .set_local_description(offer)
            .await
            .map_err(|error| peer_error("set local offer", error))?;
        self.wait_for_gathering().await?;
        self.connection
            .local_description()
            .await
            .ok_or_else(|| BlnkError::Peer("local offer is unavailable".to_owned()))
    }

    /// Applies a remote offer, creates an answer, and waits for non-trickle ICE.
    pub async fn accept_offer(
        &self,
        offer: webrtc::peer_connection::RTCSessionDescription,
    ) -> Result<webrtc::peer_connection::RTCSessionDescription, BlnkError> {
        self.connection
            .set_remote_description(offer)
            .await
            .map_err(|error| peer_error("set remote offer", error))?;
        let answer = self
            .connection
            .create_answer(None)
            .await
            .map_err(|error| peer_error("create answer", error))?;
        self.connection
            .set_local_description(answer)
            .await
            .map_err(|error| peer_error("set local answer", error))?;
        self.wait_for_gathering().await?;
        self.connection
            .local_description()
            .await
            .ok_or_else(|| BlnkError::Peer("local answer is unavailable".to_owned()))
    }

    /// Applies the remote SDP answer.
    pub async fn set_remote_answer(
        &self,
        answer: webrtc::peer_connection::RTCSessionDescription,
    ) -> Result<(), BlnkError> {
        self.connection
            .set_remote_description(answer)
            .await
            .map_err(|error| peer_error("set remote answer", error))
    }

    /// Waits until the peer connection reaches the connected state.
    pub async fn wait_connected(&self) -> Result<(), BlnkError> {
        tokio::time::timeout(DEFAULT_WAIT_TIMEOUT, async {
            loop {
                if self.events.connected_state.load(Ordering::Acquire) {
                    return Ok(());
                }
                if let Some(failure) = self.events.connection_failure.lock().await.clone() {
                    return Err(BlnkError::Peer(failure));
                }
                let notified = self.events.connection_state_changed.notified();
                if self.events.connected_state.load(Ordering::Acquire) {
                    return Ok(());
                }
                notified.await;
            }
        })
        .await
        .map_err(|_| BlnkError::Peer("timed out waiting for peer connection".to_owned()))?
    }

    /// Waits until the application data channel is open.
    pub async fn wait_channel_open(&self) -> Result<(), BlnkError> {
        tokio::time::timeout(DEFAULT_WAIT_TIMEOUT, async {
            loop {
                if self.events.channel_opened.load(Ordering::Acquire) {
                    return Ok(());
                }
                if let Some(error) = self.events.channel_error.lock().await.clone() {
                    return Err(BlnkError::Peer(error));
                }
                if self.events.channel_closed.load(Ordering::Acquire) {
                    return Err(BlnkError::Peer(
                        "data channel closed before opening".to_owned(),
                    ));
                }
                let notified = self.events.channel_state_changed.notified();
                if self.events.channel_opened.load(Ordering::Acquire) {
                    return Ok(());
                }
                notified.await;
            }
        })
        .await
        .map_err(|_| BlnkError::Peer("timed out waiting for data channel".to_owned()))?
    }

    /// Waits until the peer connection reports a closed or failed state.
    pub async fn wait_disconnected(&self) -> Result<(), BlnkError> {
        tokio::time::timeout(DEFAULT_WAIT_TIMEOUT, async {
            loop {
                if self.events.connection_failure.lock().await.is_some() {
                    return Ok(());
                }
                let notified = self.events.connection_state_changed.notified();
                if self.events.connection_failure.lock().await.is_some() {
                    return Ok(());
                }
                notified.await;
            }
        })
        .await
        .map_err(|_| BlnkError::Peer("timed out waiting for peer disconnect".to_owned()))?
    }

    /// Waits until the data-channel send buffer has been released by SCTP.
    pub async fn wait_send_buffer_empty(&self) -> Result<(), BlnkError> {
        let channel = self.channel().await?;
        tokio::time::timeout(DEFAULT_WAIT_TIMEOUT, async {
            loop {
                if channel
                    .outstanding_bytes()
                    .await
                    .map_err(|error| peer_error("read data-channel send buffer", error))?
                    == 0
                {
                    return Ok(());
                }
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .map_err(|_| BlnkError::Peer("timed out waiting for data-channel send buffer".to_owned()))?
    }

    /// Sends one encoded SWSP frame over the open data channel.
    pub async fn send_frame(&self, frame: &Frame) -> Result<(), BlnkError> {
        let channel = self.channel().await?;
        let payload = frame
            .encode()
            .map_err(|error| BlnkError::Protocol(error.to_string()))?;
        channel
            .send(BytesMut::from(payload.as_slice()))
            .await
            .map_err(|error| peer_error("send SWSP frame", error))
    }

    /// Receives and decodes one SWSP frame from the data channel.
    pub async fn recv_frame(&self) -> Result<Frame, BlnkError> {
        let payload = self
            .inbound_rx
            .recv()
            .await
            .map_err(|_| BlnkError::Peer("data channel receive loop closed".to_owned()))?;
        let (frame, consumed) =
            Frame::decode(&payload).map_err(|error| BlnkError::Protocol(error.to_string()))?;
        if consumed != payload.len() {
            return Err(BlnkError::Protocol(
                "data channel message contains trailing bytes".to_owned(),
            ));
        }
        Ok(frame)
    }

    /// Closes the data channel and the underlying peer connection.
    pub async fn close(&self) -> Result<(), BlnkError> {
        if let Some(channel) = self.events.channel.lock().await.take() {
            // A remote close can race with local teardown. The channel close is
            // therefore best-effort; the peer connection close below remains
            // the authoritative teardown result.
            let _ = channel.close().await;
        }

        match self.connection.close().await {
            Ok(()) => Ok(()),
            Err(_error) if self.events.channel_closed.load(Ordering::Acquire) => Ok(()),
            Err(error) => Err(peer_error("close peer connection", error)),
        }
    }

    async fn channel(&self) -> Result<Arc<dyn DataChannel>, BlnkError> {
        self.wait_channel_open().await?;
        self.events
            .channel
            .lock()
            .await
            .clone()
            .ok_or_else(|| BlnkError::Peer("data channel is unavailable".to_owned()))
    }

    async fn wait_for_gathering(&self) -> Result<(), BlnkError> {
        wait_for_flag(
            self.events.gathering_complete.as_ref(),
            self.events.gathering_done.as_ref(),
            "ICE gathering",
        )
        .await
    }
}

async fn wait_for_flag(notify: &Notify, flag: &AtomicBool, label: &str) -> Result<(), BlnkError> {
    wait_for_flag_with_timeout(notify, flag, label, DEFAULT_WAIT_TIMEOUT).await
}

async fn wait_for_flag_with_timeout(
    notify: &Notify,
    flag: &AtomicBool,
    label: &str,
    timeout: Duration,
) -> Result<(), BlnkError> {
    tokio::time::timeout(timeout, async {
        loop {
            if flag.load(Ordering::Acquire) {
                return;
            }
            let notified = notify.notified();
            if flag.load(Ordering::Acquire) {
                return;
            }
            notified.await;
        }
    })
    .await
    .map_err(|_| BlnkError::Peer(format!("timed out waiting for {label}")))
}

/// A deterministic in-process offer/answer harness for two loopback peers.
pub struct TwoPeerHarness {
    pub offerer: PeerHandle,
    pub answerer: PeerHandle,
}

impl TwoPeerHarness {
    /// Builds two peers, exchanges non-trickle SDP locally, and opens one channel.
    pub async fn new(label: &str) -> Result<Self, BlnkError> {
        let offerer = PeerHandle::new().await?;
        let answerer = PeerHandle::new().await?;
        offerer.create_data_channel(label).await?;

        let offer = offerer.create_offer().await?;
        let answer = answerer.accept_offer(offer).await?;
        offerer.set_remote_answer(answer).await?;

        offerer.wait_connected().await?;
        answerer.wait_connected().await?;
        offerer.wait_channel_open().await?;
        answerer.wait_channel_open().await?;

        Ok(Self { offerer, answerer })
    }
}

fn peer_error<T: std::fmt::Display>(operation: &str, error: T) -> BlnkError {
    BlnkError::Peer(format!("{operation}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::swsp::{FrameFlags, HEADER_LEN};
    use async_trait::async_trait;
    use std::collections::VecDeque;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn local_two_peer_harness_completes_offer_answer_and_ice() {
        let harness = TwoPeerHarness::new("control")
            .await
            .expect("offer/answer and ICE should complete");

        assert!(
            harness
                .offerer
                .events
                .gathering_done
                .load(Ordering::Acquire)
        );
        assert!(
            harness
                .answerer
                .events
                .gathering_done
                .load(Ordering::Acquire)
        );
        assert!(
            harness
                .offerer
                .events
                .connected_state
                .load(Ordering::Acquire)
        );
        assert!(
            harness
                .answerer
                .events
                .connected_state
                .load(Ordering::Acquire)
        );
        assert!(
            harness
                .offerer
                .events
                .channel_opened
                .load(Ordering::Acquire)
        );
        assert!(
            harness
                .answerer
                .events
                .channel_opened
                .load(Ordering::Acquire)
        );

        harness.offerer.close().await.expect("offerer should close");
        harness
            .answerer
            .close()
            .await
            .expect("answerer should close");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn local_two_peer_harness_round_trips_swsp_frame() {
        let harness = TwoPeerHarness::new("swsp")
            .await
            .expect("harness should connect");
        let frame = Frame::new(7, FrameFlags::SYN | FrameFlags::DAT, b"hello".to_vec());

        harness
            .offerer
            .send_frame(&frame)
            .await
            .expect("frame should send");
        let received = harness
            .answerer
            .recv_frame()
            .await
            .expect("frame should receive");

        assert_eq!(received, frame);
        assert_eq!(
            frame.encode().expect("frame should encode").len(),
            HEADER_LEN + 5
        );

        harness.offerer.close().await.expect("offerer should close");
        harness
            .answerer
            .close()
            .await
            .expect("answerer should close");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn failed_ice_state_is_reported_without_waiting_for_timeout() {
        let peer = PeerHandle::new().await.expect("peer should build");
        peer.events
            .on_ice_connection_state_change(RTCIceConnectionState::Failed)
            .await;

        let error = peer
            .wait_connected()
            .await
            .expect_err("failed ICE should prevent connection");
        assert!(error.to_string().contains("ICE connection entered Failed"));
        peer.close().await.expect("peer close should succeed");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn wait_for_flag_reports_timeout() {
        let notify = Notify::new();
        let flag = AtomicBool::new(false);
        let error =
            wait_for_flag_with_timeout(&notify, &flag, "test flag", Duration::from_millis(10))
                .await
                .expect_err("unset flag should time out");

        assert!(
            error
                .to_string()
                .contains("timed out waiting for test flag")
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn duplicate_close_is_idempotent_and_send_after_close_fails_cleanly() {
        let harness = TwoPeerHarness::new("close")
            .await
            .expect("harness should connect");
        harness
            .offerer
            .close()
            .await
            .expect("first close should succeed");
        harness
            .offerer
            .close()
            .await
            .expect("duplicate close should be harmless");

        let frame = Frame::new(1, FrameFlags::DAT, b"closed".to_vec());
        let error = harness
            .offerer
            .send_frame(&frame)
            .await
            .expect_err("send after close should fail");
        assert!(error.to_string().contains("data channel is unavailable"));
        harness
            .answerer
            .close()
            .await
            .expect("answerer should close");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn channel_error_is_reported_to_waiter() {
        let peer = PeerHandle::new().await.expect("peer should build");
        let channel = Arc::new(ErrorDataChannel::new());
        peer.events.install_channel(channel).await;

        let error = peer
            .wait_channel_open()
            .await
            .expect_err("channel error should prevent opening");
        assert!(error.to_string().contains("data channel reported an error"));
        peer.close().await.expect("peer close should succeed");
    }

    struct ErrorDataChannel {
        events: Mutex<VecDeque<DataChannelEvent>>,
    }

    impl ErrorDataChannel {
        fn new() -> Self {
            Self {
                events: Mutex::new(VecDeque::from([
                    DataChannelEvent::OnError,
                    DataChannelEvent::OnClose,
                ])),
            }
        }
    }

    #[async_trait]
    impl DataChannel for ErrorDataChannel {
        async fn label(&self) -> webrtc::error::Result<String> {
            Ok("test".to_owned())
        }

        async fn ordered(&self) -> webrtc::error::Result<bool> {
            Ok(true)
        }

        async fn max_packet_life_time(&self) -> webrtc::error::Result<Option<u16>> {
            Ok(None)
        }

        async fn max_retransmits(&self) -> webrtc::error::Result<Option<u16>> {
            Ok(None)
        }

        async fn protocol(&self) -> webrtc::error::Result<String> {
            Ok(String::new())
        }

        async fn negotiated(&self) -> webrtc::error::Result<bool> {
            Ok(false)
        }

        fn id(&self) -> webrtc::data_channel::RTCDataChannelId {
            0
        }

        async fn ready_state(
            &self,
        ) -> webrtc::error::Result<webrtc::data_channel::RTCDataChannelState> {
            Ok(webrtc::data_channel::RTCDataChannelState::Connecting)
        }

        async fn buffered_amount_high_threshold(&self) -> webrtc::error::Result<u32> {
            Ok(u32::MAX)
        }

        async fn set_buffered_amount_high_threshold(
            &self,
            _threshold: u32,
        ) -> webrtc::error::Result<()> {
            Ok(())
        }

        async fn buffered_amount_low_threshold(&self) -> webrtc::error::Result<u32> {
            Ok(0)
        }

        async fn set_buffered_amount_low_threshold(
            &self,
            _threshold: u32,
        ) -> webrtc::error::Result<()> {
            Ok(())
        }

        async fn send(&self, _data: BytesMut) -> webrtc::error::Result<()> {
            Ok(())
        }

        async fn send_text(&self, _text: &str) -> webrtc::error::Result<()> {
            Ok(())
        }

        async fn poll(&self) -> Option<DataChannelEvent> {
            self.events.lock().await.pop_front()
        }

        async fn close(&self) -> webrtc::error::Result<()> {
            Ok(())
        }
    }
}
