//! Sandboxed file-transfer MVP built on the existing stream protobuf schema.
//!
//! The service deliberately keeps filesystem policy local to an explicitly
//! canonicalized root. It does not claim to be an OS-wide sandbox or a
//! replacement for platform ACLs. Wire requests use `stream.proto::FileOp`;
//! file bytes remain SWSP DAT payloads and stream completion uses FIN.
//!
//! Validation is split into explicit size, range, path, overwrite,
//! cancellation, timeout, and list-entry profiles.

use std::fs::{self, Metadata};
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::time::{Duration, UNIX_EPOCH};

use prost::Message;
use sha2::{Digest, Sha256};

use crate::proto_generated::stream as wire;
use crate::protocol::swsp::{DEFAULT_MAX_PAYLOAD_LEN, Frame, FrameFlags};
use crate::utils::error::BlnkError;

use super::StreamResult;

const DEFAULT_MAX_FILE_SIZE: u64 = 64 * 1024 * 1024;
const DEFAULT_MAX_LIST_ENTRIES: usize = 10_000;
const DEFAULT_OPERATION_TIMEOUT: Duration = Duration::from_secs(10);
const FILE_TYPE: &str = "file";
static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileValidationProfile {
    Get,
    Put,
    List,
    Stat,
    Delete,
}

/// File-transfer operation requested by a peer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileOperation {
    Get,
    Put,
    List,
    Stat,
    Delete,
}

impl FileOperation {
    fn as_wire(self) -> wire::FileOpType {
        match self {
            Self::Get => wire::FileOpType::FileOpGet,
            Self::Put => wire::FileOpType::FileOpPut,
            Self::List => wire::FileOpType::FileOpList,
            Self::Stat => wire::FileOpType::FileOpStat,
            Self::Delete => wire::FileOpType::FileOpDelete,
        }
    }

    fn from_wire(value: i32) -> StreamResult<Self> {
        match wire::FileOpType::try_from(value) {
            Ok(wire::FileOpType::FileOpGet) => Ok(Self::Get),
            Ok(wire::FileOpType::FileOpPut) => Ok(Self::Put),
            Ok(wire::FileOpType::FileOpList) => Ok(Self::List),
            Ok(wire::FileOpType::FileOpStat) => Ok(Self::Stat),
            Ok(wire::FileOpType::FileOpDelete) => Ok(Self::Delete),
            Ok(wire::FileOpType::FileOpUnspecified) | Err(_) => {
                Err(BlnkError::Stream("unsupported file operation".into()))
            }
        }
    }
}

/// Validated request corresponding to `stream.FileOp`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTransferRequest {
    pub operation: FileOperation,
    pub path: String,
    pub size: u64,
    pub overwrite: bool,
    /// Inclusive byte range `[start, end]` for GET; empty means the full file.
    pub range: Option<(u64, u64)>,
    pub request_id: u64,
    pub checksum: String,
}

impl FileTransferRequest {
    pub fn get(path: impl Into<String>, range: Option<(u64, u64)>) -> StreamResult<Self> {
        validate_range(range)?;
        Ok(Self {
            operation: FileOperation::Get,
            path: path.into(),
            size: 0,
            overwrite: false,
            range,
            request_id: 0,
            checksum: String::new(),
        })
    }

    pub fn put(path: impl Into<String>, size: u64, overwrite: bool) -> Self {
        Self {
            operation: FileOperation::Put,
            path: path.into(),
            size,
            overwrite,
            range: None,
            request_id: 0,
            checksum: String::new(),
        }
    }

    pub fn with_request_id(mut self, request_id: u64) -> Self {
        self.request_id = request_id;
        self
    }
    pub fn with_checksum(mut self, checksum: impl Into<String>) -> Self {
        self.checksum = checksum.into();
        self
    }

    pub fn list(path: impl Into<String>) -> Self {
        Self {
            operation: FileOperation::List,
            path: path.into(),
            size: 0,
            overwrite: false,
            range: None,
            request_id: 0,
            checksum: String::new(),
        }
    }

    pub fn stat(path: impl Into<String>) -> Self {
        Self {
            operation: FileOperation::Stat,
            path: path.into(),
            size: 0,
            overwrite: false,
            range: None,
            request_id: 0,
            checksum: String::new(),
        }
    }

    pub fn delete(path: impl Into<String>) -> Self {
        Self {
            operation: FileOperation::Delete,
            path: path.into(),
            size: 0,
            overwrite: false,
            range: None,
            request_id: 0,
            checksum: String::new(),
        }
    }

    pub fn encode(&self) -> StreamResult<Vec<u8>> {
        self.validate_shape()?;
        Ok(self.to_wire().encode_to_vec())
    }

    pub fn from_wire(message: wire::FileOp) -> StreamResult<Self> {
        if message.r#type != FILE_TYPE {
            return Err(BlnkError::Protocol(
                "file request has an invalid type discriminator".into(),
            ));
        }
        let range = match message.range.as_slice() {
            [] => None,
            [start, end] => Some((*start as u64, *end as u64)),
            _ => {
                return Err(BlnkError::Protocol(
                    "file request range must contain zero or two values".into(),
                ));
            }
        };
        if message.range.iter().any(|value| *value < 0) {
            return Err(BlnkError::Protocol(
                "file request range values must be non-negative".into(),
            ));
        }
        let request = Self {
            operation: FileOperation::from_wire(message.op)?,
            path: message.path,
            size: u64::try_from(message.size).map_err(|_| {
                BlnkError::Protocol("file request size must be non-negative".into())
            })?,
            overwrite: message.overwrite,
            range,
            request_id: message.request_id,
            checksum: message.checksum,
        };
        request.validate_shape()?;
        Ok(request)
    }

    pub fn to_wire(&self) -> wire::FileOp {
        let range = self
            .range
            .map(|(start, end)| vec![start as i64, end as i64])
            .unwrap_or_default();
        wire::FileOp {
            r#type: FILE_TYPE.into(),
            op: self.operation.as_wire() as i32,
            path: self.path.clone(),
            size: i64::try_from(self.size).unwrap_or(i64::MAX),
            overwrite: self.overwrite,
            range,
            request_id: self.request_id,
            checksum: self.checksum.clone(),
        }
    }

    fn validate_shape(&self) -> StreamResult<()> {
        if self.path.trim().is_empty() {
            return Err(BlnkError::Stream("file path must not be empty".into()));
        }
        validate_range(self.range)?;
        if self.operation != FileOperation::Put && self.size != 0 {
            return Err(BlnkError::Protocol("file request size is only valid for PUT".into()));
        }
        if self.operation != FileOperation::Put && self.overwrite {
            return Err(BlnkError::Protocol("file request overwrite is only valid for PUT".into()));
        }
        if self.operation != FileOperation::Get && self.range.is_some() {
            return Err(BlnkError::Protocol("file request range is only valid for GET".into()));
        }
        if self.operation != FileOperation::Put && !self.checksum.is_empty() {
            return Err(BlnkError::Protocol("file checksum is only valid for PUT".into()));
        }
        if !self.checksum.is_empty()
            && (self.checksum.len() != 64
                || !self.checksum.bytes().all(|byte| byte.is_ascii_hexdigit()))
        {
            return Err(BlnkError::Protocol(
                "file checksum must be 64 hexadecimal characters".into(),
            ));
        }
        Ok(())
    }

    pub fn validate_profile(&self, profile: FileValidationProfile) -> StreamResult<()> {
        let expected = match profile {
            FileValidationProfile::Get => FileOperation::Get,
            FileValidationProfile::Put => FileOperation::Put,
            FileValidationProfile::List => FileOperation::List,
            FileValidationProfile::Stat => FileOperation::Stat,
            FileValidationProfile::Delete => FileOperation::Delete,
        };
        if self.operation != expected {
            return Err(BlnkError::Protocol(
                "file validation profile does not match operation".into(),
            ));
        }
        self.validate_shape()
    }
}

/// A bounded file operation result. Download bytes are kept bounded by the
/// configured maximum file size and are intended for the MVP only.
#[derive(Debug, Clone, PartialEq)]
pub enum FileTransferResponse {
    Download {
        info: wire::FileInfo,
        data: Vec<u8>,
    },
    Upload {
        info: wire::FileInfo,
    },
    List(wire::FileList),
    Stat(wire::FileInfo),
    Delete {
        path: String,
    },
}

/// Cooperative cancellation shared by file operations and frame collectors.
#[derive(Clone, Default)]
pub struct FileTransferCancellation(Arc<AtomicBool>);

impl FileTransferCancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

impl std::fmt::Debug for FileTransferCancellation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FileTransferCancellation")
            .field("cancelled", &self.is_cancelled())
            .finish()
    }
}

/// Filesystem policy for the file-transfer MVP.
#[derive(Debug, Clone)]
pub struct FileTransferConfig {
    pub root: PathBuf,
    pub max_file_size: u64,
    pub max_list_entries: usize,
    pub operation_timeout: Duration,
}

impl FileTransferConfig {
    pub fn new(root: impl Into<PathBuf>) -> StreamResult<Self> {
        let root = root
            .into()
            .canonicalize()
            .map_err(|error| io_error("canonicalize file-transfer root", error))?;
        if !root.is_dir() {
            return Err(BlnkError::Stream("file-transfer root must be a directory".into()));
        }
        Ok(Self {
            root,
            max_file_size: DEFAULT_MAX_FILE_SIZE,
            max_list_entries: DEFAULT_MAX_LIST_ENTRIES,
            operation_timeout: DEFAULT_OPERATION_TIMEOUT,
        })
    }

    pub fn with_max_file_size(mut self, max_file_size: u64) -> StreamResult<Self> {
        if max_file_size == 0 {
            return Err(BlnkError::Stream("max file size must be greater than zero".into()));
        }
        self.max_file_size = max_file_size;
        Ok(self)
    }

    pub fn with_max_list_entries(mut self, max_list_entries: usize) -> StreamResult<Self> {
        if max_list_entries == 0 {
            return Err(BlnkError::Stream("max list entries must be greater than zero".into()));
        }
        self.max_list_entries = max_list_entries;
        Ok(self)
    }

    pub fn with_operation_timeout(mut self, operation_timeout: Duration) -> StreamResult<Self> {
        if operation_timeout.is_zero() {
            return Err(BlnkError::Stream(
                "file operation timeout must be greater than zero".into(),
            ));
        }
        self.operation_timeout = operation_timeout;
        Ok(self)
    }
}

/// Sandboxed file service. The root is canonicalized once and every request
/// is checked again, including symlink and parent-directory checks.
#[derive(Debug, Clone)]
pub struct FileTransferService {
    config: FileTransferConfig,
}

impl FileTransferService {
    pub fn new(config: FileTransferConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &FileTransferConfig {
        &self.config
    }

    pub async fn execute(
        &self,
        request: FileTransferRequest,
        upload: &[u8],
        cancellation: &FileTransferCancellation,
    ) -> StreamResult<FileTransferResponse> {
        request.validate_shape()?;
        let operation = self.execute_inner(request, upload, cancellation);
        match tokio::time::timeout(self.config.operation_timeout, operation).await {
            Ok(result) => result,
            Err(_) => Err(BlnkError::Stream("file operation timed out".into())),
        }
    }

    async fn execute_inner(
        &self,
        request: FileTransferRequest,
        upload: &[u8],
        cancellation: &FileTransferCancellation,
    ) -> StreamResult<FileTransferResponse> {
        check_cancelled(cancellation)?;
        match request.operation {
            FileOperation::Get => self.download(&request, cancellation).await,
            FileOperation::Put => self.upload(&request, upload, cancellation).await,
            FileOperation::List => self.list(&request, cancellation).await,
            FileOperation::Stat => self.stat(&request, cancellation).await,
            FileOperation::Delete => self.delete(&request, cancellation).await,
        }
    }

    async fn download(
        &self,
        request: &FileTransferRequest,
        cancellation: &FileTransferCancellation,
    ) -> StreamResult<FileTransferResponse> {
        let path = self.resolve_existing(&request.path)?;
        let metadata = tokio::fs::metadata(&path)
            .await
            .map_err(|error| io_error("stat file for download", error))?;
        if metadata.is_dir() {
            return Err(BlnkError::Stream("cannot download a directory".into()));
        }
        let size = metadata.len();
        if size > self.config.max_file_size {
            return Err(BlnkError::Stream("file exceeds configured maximum size".into()));
        }
        let mut file = tokio::fs::File::open(&path)
            .await
            .map_err(|error| io_error("open file for download", error))?;
        let mut data = Vec::with_capacity(size as usize);
        let mut buffer = vec![0_u8; DEFAULT_MAX_PAYLOAD_LEN];
        loop {
            check_cancelled(cancellation)?;
            let read = tokio::io::AsyncReadExt::read(&mut file, &mut buffer)
                .await
                .map_err(|error| io_error("read file for download", error))?;
            if read == 0 {
                break;
            }
            data.extend_from_slice(&buffer[..read]);
            tokio::task::yield_now().await;
        }
        let (start, end) = request.range.unwrap_or((0, size.saturating_sub(1)));
        if size == 0 {
            if request.range.is_some() {
                return Err(BlnkError::Stream(
                    "range cannot be requested from an empty file".into(),
                ));
            }
        } else if start > end || end >= size {
            return Err(BlnkError::Stream("file range is outside file bounds".into()));
        }
        let selected = if size == 0 {
            Vec::new()
        } else {
            data[start as usize..=end as usize].to_vec()
        };
        Ok(FileTransferResponse::Download {
            info: with_request_id(file_info(&request.path, &metadata), request.request_id),
            data: selected,
        })
    }

    async fn upload(
        &self,
        request: &FileTransferRequest,
        upload: &[u8],
        cancellation: &FileTransferCancellation,
    ) -> StreamResult<FileTransferResponse> {
        if request.size > self.config.max_file_size {
            return Err(BlnkError::Stream("upload exceeds configured maximum size".into()));
        }
        if request.size != upload.len() as u64 {
            return Err(BlnkError::Stream("upload size does not match request".into()));
        }
        let path = self.resolve_for_write(&request.path, request.overwrite)?;
        let parent = path
            .parent()
            .ok_or_else(|| BlnkError::Stream("upload path has no parent".into()))?;
        let temp_name = format!(
            ".blnk-upload-{}-{}",
            std::process::id(),
            TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed)
        );
        let temp_path = parent.join(temp_name);
        let mut file = tokio::fs::File::create(&temp_path)
            .await
            .map_err(|error| io_error("create temporary upload", error))?;
        let result = async {
            for chunk in upload.chunks(DEFAULT_MAX_PAYLOAD_LEN) {
                check_cancelled(cancellation)?;
                tokio::io::AsyncWriteExt::write_all(&mut file, chunk)
                    .await
                    .map_err(|error| io_error("write temporary upload", error))?;
                tokio::task::yield_now().await;
            }
            tokio::io::AsyncWriteExt::flush(&mut file)
                .await
                .map_err(|error| io_error("flush temporary upload", error))?;
            file.sync_all()
                .await
                .map_err(|error| io_error("sync temporary upload", error))?;
            tokio::io::AsyncWriteExt::shutdown(&mut file)
                .await
                .map_err(|error| io_error("close temporary upload", error))?;
            if !request.checksum.is_empty() {
                let digest = Sha256::digest(upload);
                let actual = format!("{digest:x}");
                if !actual.eq_ignore_ascii_case(&request.checksum) {
                    return Err(BlnkError::Stream("upload checksum verification failed".into()));
                }
            }
            drop(file);
            tokio::fs::rename(&temp_path, &path)
                .await
                .map_err(|error| io_error("commit upload", error))?;
            let metadata = tokio::fs::metadata(&path)
                .await
                .map_err(|error| io_error("stat uploaded file", error))?;
            Ok::<_, BlnkError>(FileTransferResponse::Upload {
                info: with_request_id(file_info(&request.path, &metadata), request.request_id),
            })
        }
        .await;
        if result.is_err() {
            let _ = tokio::fs::remove_file(&temp_path).await;
        }
        result
    }

    async fn list(
        &self,
        request: &FileTransferRequest,
        cancellation: &FileTransferCancellation,
    ) -> StreamResult<FileTransferResponse> {
        let path = self.resolve_existing(&request.path)?;
        let metadata = tokio::fs::metadata(&path)
            .await
            .map_err(|error| io_error("stat directory", error))?;
        if !metadata.is_dir() {
            return Err(BlnkError::Stream("list path is not a directory".into()));
        }
        let mut entries = tokio::fs::read_dir(&path)
            .await
            .map_err(|error| io_error("open directory", error))?;
        let mut files = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|error| io_error("read directory", error))?
        {
            check_cancelled(cancellation)?;
            if files.len() >= self.config.max_list_entries {
                return Err(BlnkError::Stream("directory exceeds configured entry limit".into()));
            }
            let metadata = tokio::fs::symlink_metadata(entry.path())
                .await
                .map_err(|error| io_error("stat directory entry", error))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            files.push(file_info(&name, &metadata));
        }
        files.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(FileTransferResponse::List(wire::FileList {
            path: request.path.clone(),
            files,
            request_id: request.request_id,
        }))
    }

    async fn stat(
        &self,
        request: &FileTransferRequest,
        cancellation: &FileTransferCancellation,
    ) -> StreamResult<FileTransferResponse> {
        check_cancelled(cancellation)?;
        let path = self.resolve_existing(&request.path)?;
        let metadata = tokio::fs::metadata(&path)
            .await
            .map_err(|error| io_error("stat file", error))?;
        Ok(FileTransferResponse::Stat(with_request_id(
            file_info(&request.path, &metadata),
            request.request_id,
        )))
    }

    async fn delete(
        &self,
        request: &FileTransferRequest,
        cancellation: &FileTransferCancellation,
    ) -> StreamResult<FileTransferResponse> {
        check_cancelled(cancellation)?;
        let path = self.resolve_existing(&request.path)?;
        let metadata = tokio::fs::metadata(&path)
            .await
            .map_err(|error| io_error("stat file for deletion", error))?;
        if metadata.is_dir() {
            return Err(BlnkError::Stream("delete only supports files".into()));
        }
        tokio::fs::remove_file(&path)
            .await
            .map_err(|error| io_error("delete file", error))?;
        Ok(FileTransferResponse::Delete { path: request.path.clone() })
    }

    fn resolve_existing(&self, request_path: &str) -> StreamResult<PathBuf> {
        let candidate = self.resolve_candidate(request_path)?;
        let metadata = fs::symlink_metadata(&candidate)
            .map_err(|error| io_error("inspect file path", error))?;
        if metadata.file_type().is_symlink() {
            return Err(BlnkError::Stream("symlink paths are not allowed".into()));
        }
        let resolved = candidate
            .canonicalize()
            .map_err(|error| io_error("canonicalize file path", error))?;
        if !resolved.starts_with(&self.config.root) {
            return Err(BlnkError::Stream("file path escapes configured root".into()));
        }
        Ok(resolved)
    }

    fn resolve_for_write(&self, request_path: &str, overwrite: bool) -> StreamResult<PathBuf> {
        let candidate = self.resolve_candidate(request_path)?;
        if let Ok(metadata) = fs::symlink_metadata(&candidate) {
            if metadata.file_type().is_symlink() {
                return Err(BlnkError::Stream("symlink paths are not allowed".into()));
            }
            if metadata.is_dir() {
                return Err(BlnkError::Stream("cannot upload over a directory".into()));
            }
            if !overwrite {
                return Err(BlnkError::Stream(
                    "destination exists and overwrite is disabled".into(),
                ));
            }
        }
        Ok(candidate)
    }

    fn resolve_candidate(&self, request_path: &str) -> StreamResult<PathBuf> {
        let relative = safe_relative_path(request_path)?;
        let candidate = self.config.root.join(relative);
        if candidate != self.config.root {
            let parent = candidate
                .parent()
                .ok_or_else(|| BlnkError::Stream("file path has no parent".into()))?;
            let canonical_parent = parent
                .canonicalize()
                .map_err(|error| io_error("canonicalize file parent", error))?;
            if !canonical_parent.starts_with(&self.config.root) {
                return Err(BlnkError::Stream("file path escapes configured root".into()));
            }
        }
        Ok(candidate)
    }
}

/// Builds the SYN frame that opens a file stream.
pub fn encode_request_frame(stream_id: u32, request: &FileTransferRequest) -> StreamResult<Frame> {
    if stream_id == 0 {
        return Err(BlnkError::Stream("file stream id must be non-zero".into()));
    }
    Ok(Frame::new(stream_id, FrameFlags::SYN | FrameFlags::DAT, request.encode()?))
}

/// Builds a file-data frame. The final chunk carries FIN and intermediate
/// chunks carry MORE. Empty files use a DAT|FIN frame with an empty payload.
pub fn encode_data_frame(stream_id: u32, data: Vec<u8>, final_chunk: bool) -> StreamResult<Frame> {
    if stream_id == 0 {
        return Err(BlnkError::Stream("file stream id must be non-zero".into()));
    }
    let flags = if final_chunk {
        FrameFlags::DAT | FrameFlags::FIN
    } else {
        FrameFlags::DAT | FrameFlags::MORE
    };
    let frame = Frame::new(stream_id, flags, data);
    frame
        .encode()
        .map_err(|error| BlnkError::Protocol(error.to_string()))?;
    Ok(frame)
}

/// Decodes a file request from a SYN frame.
pub fn decode_request_frame(frame: &Frame) -> StreamResult<FileTransferRequest> {
    if frame.stream_id == 0 || !frame.flags.is_syn() || !frame.flags.is_dat() {
        return Err(BlnkError::Protocol("file request must be a non-control SYN|DAT frame".into()));
    }
    FileTransferRequest::from_wire(
        wire::FileOp::decode(frame.payload.as_slice()).map_err(|error| {
            BlnkError::Protocol(format!("invalid file request protobuf: {error}"))
        })?,
    )
}

/// Collects a PUT body from data frames and enforces size/cancellation limits.
pub fn collect_data_frames<I>(
    frames: I,
    expected_size: u64,
    max_file_size: u64,
    cancellation: &FileTransferCancellation,
) -> StreamResult<Vec<u8>>
where
    I: IntoIterator<Item = Frame>,
{
    if expected_size > max_file_size {
        return Err(BlnkError::Stream("upload exceeds configured maximum size".into()));
    }
    let mut data = Vec::with_capacity(expected_size as usize);
    let mut finished = false;
    for frame in frames {
        check_cancelled(cancellation)?;
        if frame.stream_id == 0 || !frame.flags.is_dat() || frame.flags.is_syn() {
            return Err(BlnkError::Protocol("file data must be a non-control DAT frame".into()));
        }
        if finished {
            return Err(BlnkError::Protocol("file data arrived after FIN".into()));
        }
        if data.len() as u64 + frame.payload.len() as u64 > expected_size {
            return Err(BlnkError::Stream("file upload contains too many bytes".into()));
        }
        data.extend_from_slice(&frame.payload);
        finished = frame.flags.is_fin();
        if finished {
            break;
        }
    }
    if !finished {
        return Err(BlnkError::Stream("file upload did not receive FIN".into()));
    }
    if data.len() as u64 != expected_size {
        return Err(BlnkError::Stream("file upload size does not match request".into()));
    }
    Ok(data)
}

/// Encodes a response into one or more SWSP data frames.
///
/// The request operation determines how the first metadata frame is decoded:
/// GET uses `FileInfo` followed by data frames, LIST uses `FileList`, and the
/// remaining operations use a terminal `FileInfo` frame.
pub fn encode_response_frames(
    stream_id: u32,
    response: &FileTransferResponse,
) -> StreamResult<Vec<Frame>> {
    if stream_id == 0 {
        return Err(BlnkError::Stream("file stream id must be non-zero".into()));
    }
    match response {
        FileTransferResponse::Download { info, data } => {
            let mut frames = Vec::new();
            let metadata_flags = if data.is_empty() {
                FrameFlags::DAT | FrameFlags::FIN
            } else {
                FrameFlags::DAT | FrameFlags::MORE
            };
            frames.push(Frame::new(stream_id, metadata_flags, info.encode_to_vec()));
            if !data.is_empty() {
                let chunks: Vec<&[u8]> = data.chunks(DEFAULT_MAX_PAYLOAD_LEN).collect();
                for (index, chunk) in chunks.iter().enumerate() {
                    frames.push(Frame::new(
                        stream_id,
                        if index + 1 == chunks.len() {
                            FrameFlags::DAT | FrameFlags::FIN
                        } else {
                            FrameFlags::DAT | FrameFlags::MORE
                        },
                        (*chunk).to_vec(),
                    ));
                }
            }
            Ok(frames)
        }
        FileTransferResponse::Upload { info } | FileTransferResponse::Stat(info) => {
            Ok(vec![Frame::new(stream_id, FrameFlags::DAT | FrameFlags::FIN, info.encode_to_vec())])
        }
        FileTransferResponse::List(list) => {
            Ok(vec![Frame::new(stream_id, FrameFlags::DAT | FrameFlags::FIN, list.encode_to_vec())])
        }
        FileTransferResponse::Delete { path } => Ok(vec![Frame::new(
            stream_id,
            FrameFlags::DAT | FrameFlags::FIN,
            wire::FileInfo {
                name: path.clone(),
                size: 0,
                is_dir: false,
                modified: String::new(),
                mode: String::new(),
                request_id: 0,
            }
            .encode_to_vec(),
        )]),
    }
}

/// Decodes one metadata response frame using the operation from the request.
pub fn decode_response_metadata(
    request: &FileTransferRequest,
    frame: &Frame,
) -> StreamResult<FileTransferResponse> {
    if frame.stream_id == 0 || !frame.flags.is_dat() || frame.flags.is_syn() {
        return Err(BlnkError::Protocol(
            "file response metadata must be a non-control DAT frame".into(),
        ));
    }
    match request.operation {
        FileOperation::List => wire::FileList::decode(frame.payload.as_slice())
            .map(FileTransferResponse::List)
            .map_err(|error| BlnkError::Protocol(format!("invalid file list protobuf: {error}"))),
        FileOperation::Get | FileOperation::Put | FileOperation::Stat | FileOperation::Delete => {
            wire::FileInfo::decode(frame.payload.as_slice())
                .map(|info| match request.operation {
                    FileOperation::Get => FileTransferResponse::Download { info, data: Vec::new() },
                    FileOperation::Put => FileTransferResponse::Upload { info },
                    FileOperation::Stat => FileTransferResponse::Stat(info),
                    FileOperation::Delete => FileTransferResponse::Delete { path: info.name },
                    FileOperation::List => unreachable!(),
                })
                .map_err(|error| {
                    BlnkError::Protocol(format!("invalid file info protobuf: {error}"))
                })
        }
    }
}

pub fn decode_response_metadata_checked(
    request: &FileTransferRequest,
    frame: &Frame,
    tracker: &super::StreamRequestTracker,
) -> StreamResult<FileTransferResponse> {
    if request.request_id != 0 {
        tracker.accept(request.request_id)?;
    }
    let response = decode_response_metadata(request, frame)?;
    let response_id = match &response {
        FileTransferResponse::Download { info, .. }
        | FileTransferResponse::Upload { info }
        | FileTransferResponse::Stat(info) => info.request_id,
        FileTransferResponse::List(list) => list.request_id,
        FileTransferResponse::Delete { .. } => request.request_id,
    };
    if request.request_id != 0 && response_id != request.request_id {
        return Err(BlnkError::Stream("response request id does not match active request".into()));
    }
    Ok(response)
}

fn safe_relative_path(request_path: &str) -> StreamResult<PathBuf> {
    // Security: reject embedded null bytes to prevent null byte injection vulnerabilities.
    if request_path.contains('\0') {
        return Err(BlnkError::Stream("null bytes in file path are not allowed".into()));
    }
    let path = Path::new(request_path);
    if path.is_absolute() {
        return Err(BlnkError::Stream("absolute file paths are not allowed".into()));
    }
    let mut relative = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(value) => relative.push(value),
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(BlnkError::Stream("file path traversal is not allowed".into()));
            }
        }
    }
    Ok(relative)
}

fn validate_range(range: Option<(u64, u64)>) -> StreamResult<()> {
    if let Some((start, end)) = range
        && start > end
    {
        return Err(BlnkError::Stream("file range start exceeds end".into()));
    }
    Ok(())
}

fn check_cancelled(cancellation: &FileTransferCancellation) -> StreamResult<()> {
    if cancellation.is_cancelled() {
        Err(BlnkError::Stream("file operation cancelled".into()))
    } else {
        Ok(())
    }
}

fn io_error(operation: &str, error: io::Error) -> BlnkError {
    BlnkError::Io(io::Error::new(error.kind(), format!("{operation}: {error}")))
}

fn file_info(name: &str, metadata: &Metadata) -> wire::FileInfo {
    wire::FileInfo {
        name: name.into(),
        size: i64::try_from(metadata.len()).unwrap_or(i64::MAX),
        is_dir: metadata.is_dir(),
        modified: modified_string(metadata),
        mode: mode_string(metadata),
        request_id: 0,
    }
}

fn with_request_id(mut info: wire::FileInfo, request_id: u64) -> wire::FileInfo {
    info.request_id = request_id;
    info
}

fn modified_string(metadata: &Metadata) -> String {
    metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_default()
}

#[cfg(unix)]
fn mode_string(metadata: &Metadata) -> String {
    use std::os::unix::fs::PermissionsExt;
    format!("{:o}", metadata.permissions().mode() & 0o777)
}

#[cfg(not(unix))]
fn mode_string(_metadata: &Metadata) -> String {
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peer::TwoPeerHarness;
    use crate::session::{SessionRuntime, SessionRuntimeConfig};
    use std::fs;
    use std::time::SystemTime;

    fn temp_root(label: &str) -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("blnk-file-{label}-{}-{suffix}", std::process::id()));
        fs::create_dir_all(&root).expect("temporary root should be created");
        root
    }

    async fn connected_runtime_pair() -> (SessionRuntime, SessionRuntime) {
        let harness = TwoPeerHarness::new("file-transfer")
            .await
            .expect("peer harness should connect");
        let mut server =
            SessionRuntime::new(harness.offerer, SessionRuntimeConfig::server("123456"))
                .expect("server runtime should build");
        let mut client =
            SessionRuntime::new(harness.answerer, SessionRuntimeConfig::client("123456"))
                .expect("client runtime should build");
        let (server_result, client_result) = tokio::join!(server.handshake(), client.handshake());
        server_result.expect("server handshake should succeed");
        client_result.expect("client handshake should succeed");
        (server, client)
    }

    #[tokio::test]
    async fn sandboxed_service_supports_file_mvp_operations() {
        let root = temp_root("operations");
        let service = FileTransferService::new(
            FileTransferConfig::new(&root)
                .expect("config")
                .with_max_file_size(1024)
                .expect("limit"),
        );
        let cancellation = FileTransferCancellation::default();

        service
            .execute(FileTransferRequest::put("nested.txt", 5, false), b"hello", &cancellation)
            .await
            .expect("upload should succeed");
        let download = service
            .execute(
                FileTransferRequest::get("nested.txt", Some((1, 3))).expect("range"),
                &[],
                &cancellation,
            )
            .await
            .expect("download should succeed");
        match download {
            FileTransferResponse::Download { data, info } => {
                assert_eq!(data, b"ell");
                assert_eq!(info.size, 5);
            }
            other => panic!("unexpected response: {other:?}"),
        }
        let listing = service
            .execute(FileTransferRequest::list("."), &[], &cancellation)
            .await
            .expect("list should succeed");
        assert!(matches!(listing, FileTransferResponse::List(_)));
        let stat = service
            .execute(FileTransferRequest::stat("nested.txt"), &[], &cancellation)
            .await
            .expect("stat should succeed");
        assert!(matches!(stat, FileTransferResponse::Stat(_)));
        service
            .execute(FileTransferRequest::delete("nested.txt"), &[], &cancellation)
            .await
            .expect("delete should succeed");
        assert!(!root.join("nested.txt").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn path_policy_rejects_traversal_overwrite_and_oversized_upload() {
        let root = temp_root("policy");
        fs::write(root.join("existing.txt"), b"old").expect("fixture");
        let service = FileTransferService::new(
            FileTransferConfig::new(&root)
                .expect("config")
                .with_max_file_size(4)
                .expect("limit"),
        );
        let cancellation = FileTransferCancellation::default();
        assert!(
            service
                .execute(FileTransferRequest::stat("../outside"), &[], &cancellation,)
                .await
                .is_err()
        );
        assert!(
            service
                .execute(FileTransferRequest::put("existing.txt", 3, false), b"new", &cancellation,)
                .await
                .is_err()
        );
        assert!(
            service
                .execute(FileTransferRequest::put("large.txt", 5, false), b"12345", &cancellation,)
                .await
                .is_err()
        );
        cancellation.cancel();
        assert!(
            service
                .execute(FileTransferRequest::list("."), &[], &cancellation)
                .await
                .is_err()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn symlink_escape_is_rejected() {
        use std::os::unix::fs::symlink;
        let root = temp_root("symlink");
        let outside = temp_root("outside");
        fs::write(outside.join("secret.txt"), b"secret").expect("fixture");
        symlink(&outside, root.join("link")).expect("symlink fixture");
        let service = FileTransferService::new(FileTransferConfig::new(&root).expect("config"));
        let cancellation = FileTransferCancellation::default();
        assert!(
            service
                .execute(FileTransferRequest::stat("link/secret.txt"), &[], &cancellation,)
                .await
                .is_err()
        );
        let _ = fs::remove_dir_all(root);
        let _ = fs::remove_dir_all(outside);
    }

    #[tokio::test]
    async fn null_byte_path_is_rejected() {
        let root = temp_root("nullbyte");
        let service = FileTransferService::new(FileTransferConfig::new(&root).expect("config"));
        let cancellation = FileTransferCancellation::default();
        assert!(
            service
                .execute(FileTransferRequest::stat("file\0.txt"), &[], &cancellation,)
                .await
                .is_err()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn request_and_data_frames_round_trip_with_limits() {
        let request = FileTransferRequest::put("upload.bin", 3, true);
        let request_frame = encode_request_frame(7, &request).expect("request frame");
        let decoded = decode_request_frame(&request_frame).expect("request decode");
        assert_eq!(decoded, request);
        let frames = vec![
            encode_data_frame(7, b"ab".to_vec(), false).expect("chunk"),
            encode_data_frame(7, b"c".to_vec(), true).expect("final chunk"),
        ];
        let cancellation = FileTransferCancellation::default();
        assert_eq!(collect_data_frames(frames, 3, 10, &cancellation).expect("body"), b"abc");
    }

    #[tokio::test]
    async fn authenticated_runtime_accepts_file_stream_frames() {
        let (mut server, mut client) = connected_runtime_pair().await;
        let client_stream = client
            .open_file_stream(".")
            .expect("client file stream should open");
        let server_stream = server
            .accept_file_stream(client_stream.id(), ".")
            .expect("server should accept client stream");
        assert_eq!(client_stream.id(), server_stream.id());
        let request = FileTransferRequest::get("fixture.txt", None).expect("request");
        client
            .send_file_request(client_stream.id(), &request)
            .await
            .expect("request should send");
        let frame = server
            .recv_file_frame()
            .await
            .expect("server should receive request");
        assert_eq!(decode_request_frame(&frame).expect("request decode"), request);
        client
            .send_file_chunk(client_stream.id(), b"ok".to_vec(), true)
            .await
            .expect("data should send");
        let data = server
            .recv_file_frame()
            .await
            .expect("server should receive data");
        let cancellation = FileTransferCancellation::default();
        assert_eq!(
            collect_data_frames(vec![data], 2, 10, &cancellation).expect("data body"),
            b"ok"
        );
        server
            .close_file_stream(server_stream.id())
            .await
            .expect("server stream should close");
        client.close().await.expect("client should close");
    }
}
