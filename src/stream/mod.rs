//! Stream registry and capability boundaries.
//!
//! Concrete handlers remain separate from the stream identity registry.
//! Issue #39 provides the sandboxed file-transfer handler; other handlers remain
//! follow-up work.

pub mod file;
pub mod shell;

pub mod proxy;
pub mod proxy_handler;

use std::collections::BTreeMap;

use crate::utils::error::BlnkError;

pub type StreamResult<T> = Result<T, BlnkError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum StreamKind {
    Http,
    File,
    Tcp,
    WebSocket,
    Shell,
    Adapter,
}

impl StreamKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Http => "http",
            Self::File => "file",
            Self::Tcp => "tcp",
            Self::WebSocket => "websocket",
            Self::Shell => "shell",
            Self::Adapter => "adapter",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamEntry {
    stream_id: u32,
    kind: StreamKind,
    connect_path: String,
}

impl StreamEntry {
    pub fn id(&self) -> u32 {
        self.stream_id
    }

    pub fn kind(&self) -> StreamKind {
        self.kind
    }

    pub fn connect_path(&self) -> &str {
        &self.connect_path
    }
}

#[derive(Debug, Default)]
pub struct StreamRegistry {
    next_id: u32,
    active: BTreeMap<u32, StreamEntry>,
    closed_count: u64,
}

impl StreamRegistry {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            active: BTreeMap::new(),
            closed_count: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.active.len()
    }

    pub fn is_empty(&self) -> bool {
        self.active.is_empty()
    }

    pub fn contains(&self, stream_id: u32) -> bool {
        self.active.contains_key(&stream_id)
    }

    pub fn get(&self, stream_id: u32) -> Option<&StreamEntry> {
        self.active.get(&stream_id)
    }

    pub fn closed_count(&self) -> u64 {
        self.closed_count
    }

    pub fn open(
        &mut self,
        kind: StreamKind,
        connect_path: impl Into<String>,
    ) -> StreamResult<StreamEntry> {
        let connect_path = connect_path.into();
        if connect_path.trim().is_empty() {
            return Err(BlnkError::Stream("connect_path must not be empty".into()));
        }

        let stream_id = self.allocate_id()?;
        let entry = StreamEntry {
            stream_id,
            kind,
            connect_path,
        };
        self.active.insert(stream_id, entry.clone());
        Ok(entry)
    }

    pub fn accept(
        &mut self,
        stream_id: u32,
        kind: StreamKind,
        connect_path: impl Into<String>,
    ) -> StreamResult<StreamEntry> {
        if stream_id == 0 {
            return Err(BlnkError::Stream("stream id must be non-zero".into()));
        }
        let connect_path = connect_path.into();
        if connect_path.trim().is_empty() {
            return Err(BlnkError::Stream("connect_path must not be empty".into()));
        }
        if self.active.contains_key(&stream_id) {
            return Err(BlnkError::Stream(format!(
                "stream id already active: {stream_id}"
            )));
        }
        if stream_id >= self.next_id {
            self.next_id = stream_id.wrapping_add(1).max(1);
        }
        let entry = StreamEntry {
            stream_id,
            kind,
            connect_path,
        };
        self.active.insert(stream_id, entry.clone());
        Ok(entry)
    }

    pub fn close(&mut self, stream_id: u32) -> StreamResult<StreamEntry> {
        let entry = self
            .active
            .remove(&stream_id)
            .ok_or_else(|| BlnkError::Stream(format!("unknown stream id: {stream_id}")))?;
        self.closed_count = self.closed_count.saturating_add(1);
        Ok(entry)
    }

    pub fn clear(&mut self) {
        self.closed_count = self
            .closed_count
            .saturating_add(self.active.len().try_into().unwrap_or(u64::MAX));
        self.active.clear();
    }

    fn allocate_id(&mut self) -> StreamResult<u32> {
        let start = self.next_id.max(1);
        loop {
            let candidate = self.next_id.max(1);
            self.next_id = candidate.wrapping_add(1).max(1);
            if !self.active.contains_key(&candidate) {
                return Ok(candidate);
            }
            if self.next_id == start {
                return Err(BlnkError::Stream("stream id space exhausted".into()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocates_non_zero_unique_ids_and_releases_them() {
        let mut registry = StreamRegistry::new();
        let first = registry.open(StreamKind::Http, "/").expect("first stream");
        let second = registry
            .open(StreamKind::Shell, "/shell")
            .expect("second stream");
        assert_ne!(first.id(), 0);
        assert_ne!(first.id(), second.id());
        assert_eq!(registry.len(), 2);
        registry.close(first.id()).expect("close first");
        assert!(!registry.contains(first.id()));
        assert_eq!(registry.closed_count(), 1);
    }

    #[test]
    fn unknown_close_and_empty_path_are_rejected() {
        let mut registry = StreamRegistry::new();
        assert!(registry.open(StreamKind::File, "").is_err());
        assert!(registry.close(99).is_err());
    }

    #[test]
    fn clear_closes_all_active_streams() {
        let mut registry = StreamRegistry::new();
        registry.open(StreamKind::Tcp, "tcp").expect("stream");
        registry.open(StreamKind::WebSocket, "ws").expect("stream");
        registry.clear();
        assert!(registry.is_empty());
        assert_eq!(registry.closed_count(), 2);
    }
}
