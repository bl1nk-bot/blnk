//! Policy-constrained pipe-based shell MVP.
//!
//! This module deliberately does not claim PTY, browser, Android production, or
//! original-client interoperability. Commands are executed directly through
//! [`tokio::process::Command`]; no implicit shell interpolation is performed.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use prost::Message;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use crate::proto_generated::stream as wire;
use crate::protocol::swsp::{Frame, FrameFlags};
use crate::utils::error::BlnkError;

use super::{StreamKind, StreamResult};

const DEFAULT_MAX_OUTPUT_BYTES: usize = 1024 * 1024;
const DEFAULT_MAX_ARGUMENTS: usize = 32;
const DEFAULT_MAX_ARGUMENT_BYTES: usize = 4096;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// The source of a pipe-based shell output event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellOutputKind {
    Stdout,
    Stderr,
}

/// One bounded output chunk emitted by the child process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellOutputChunk {
    pub kind: ShellOutputKind,
    pub data: Vec<u8>,
}

/// The validated command specification sent to the local policy checker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellCommand {
    program: String,
    args: Vec<String>,
    cwd: PathBuf,
}

impl ShellCommand {
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            cwd: PathBuf::new(),
        }
    }

    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    pub fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    pub fn cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = cwd.into();
        self
    }

    pub fn program(&self) -> &str {
        &self.program
    }

    pub fn arguments(&self) -> &[String] {
        &self.args
    }

    pub fn working_directory(&self) -> &Path {
        &self.cwd
    }
}

/// Local policy for shell process execution.
#[derive(Debug, Clone)]
pub struct ShellPolicy {
    root_dir: PathBuf,
    allowed_programs: BTreeSet<String>,
    environment: BTreeMap<String, String>,
    max_output_bytes: usize,
    max_arguments: usize,
    max_argument_bytes: usize,
    timeout: Duration,
}

impl ShellPolicy {
    /// Creates a restrictive policy rooted at `root_dir` with no inherited environment.
    pub fn new(root_dir: impl Into<PathBuf>) -> StreamResult<Self> {
        let root_dir = root_dir.into();
        if !root_dir.is_absolute() {
            return Err(BlnkError::Stream(
                "shell policy root must be an absolute path".into(),
            ));
        }
        Ok(Self {
            root_dir,
            allowed_programs: BTreeSet::new(),
            environment: BTreeMap::new(),
            max_output_bytes: DEFAULT_MAX_OUTPUT_BYTES,
            max_arguments: DEFAULT_MAX_ARGUMENTS,
            max_argument_bytes: DEFAULT_MAX_ARGUMENT_BYTES,
            timeout: DEFAULT_TIMEOUT,
        })
    }

    pub fn allow_program(mut self, program: impl Into<String>) -> Self {
        self.allowed_programs.insert(program.into());
        self
    }

    pub fn with_environment(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.environment.insert(name.into(), value.into());
        self
    }

    pub fn with_max_output_bytes(mut self, limit: usize) -> StreamResult<Self> {
        if limit == 0 {
            return Err(BlnkError::Stream(
                "shell output limit must be greater than zero".into(),
            ));
        }
        self.max_output_bytes = limit;
        Ok(self)
    }

    pub fn with_max_arguments(mut self, limit: usize) -> StreamResult<Self> {
        if limit == 0 {
            return Err(BlnkError::Stream(
                "shell argument limit must be greater than zero".into(),
            ));
        }
        self.max_arguments = limit;
        Ok(self)
    }

    pub fn with_timeout(mut self, duration: Duration) -> StreamResult<Self> {
        if duration.is_zero() {
            return Err(BlnkError::Stream(
                "shell timeout must be greater than zero".into(),
            ));
        }
        self.timeout = duration;
        Ok(self)
    }

    pub fn root_dir(&self) -> &Path {
        &self.root_dir
    }

    pub fn max_output_bytes(&self) -> usize {
        self.max_output_bytes
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    fn validate(&self, command: &ShellCommand) -> StreamResult<PathBuf> {
        if command.program.trim().is_empty() || command.program.contains('\0') {
            return Err(BlnkError::Stream("shell program must not be empty".into()));
        }
        if !self.allowed_programs.is_empty() && !self.allowed_programs.contains(&command.program) {
            return Err(BlnkError::Stream(format!(
                "shell program is not allowed by policy: {}",
                command.program
            )));
        }
        if command.args.len() > self.max_arguments {
            return Err(BlnkError::Stream(
                "shell argument count exceeds policy".into(),
            ));
        }
        if command
            .args
            .iter()
            .any(|arg| arg.contains('\0') || arg.len() > self.max_argument_bytes)
        {
            return Err(BlnkError::Stream("shell argument exceeds policy".into()));
        }

        let cwd = if command.cwd.as_os_str().is_empty() {
            self.root_dir.clone()
        } else {
            if command.cwd.is_absolute()
                || command.cwd.components().any(|component| {
                    matches!(
                        component,
                        Component::ParentDir | Component::RootDir | Component::Prefix(_)
                    )
                })
            {
                return Err(BlnkError::Stream(
                    "shell working directory must stay relative to policy root".into(),
                ));
            }
            self.root_dir.join(&command.cwd)
        };

        let canonical_root = std::fs::canonicalize(&self.root_dir).map_err(BlnkError::Io)?;
        let canonical_cwd = std::fs::canonicalize(&cwd).map_err(BlnkError::Io)?;
        if !canonical_cwd.starts_with(&canonical_root) || !canonical_cwd.is_dir() {
            return Err(BlnkError::Stream(
                "shell working directory escapes policy root".into(),
            ));
        }
        Ok(canonical_cwd)
    }
}

/// Result returned after a shell process exits or is cancelled.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellRunResult {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub exit_code: Option<i32>,
    pub signaled: bool,
}

/// Pipe-based shell handler with bounded local execution policy.
#[derive(Debug, Clone)]
pub struct ShellStreamHandler {
    policy: ShellPolicy,
}

impl ShellStreamHandler {
    pub fn new(policy: ShellPolicy) -> Self {
        Self { policy }
    }

    pub fn policy(&self) -> &ShellPolicy {
        &self.policy
    }

    /// Runs one command, collecting bounded stdout and stderr separately.
    pub async fn run(
        &self,
        command: ShellCommand,
        cancellation: CancellationToken,
    ) -> StreamResult<ShellRunResult> {
        let cwd = self.policy.validate(&command)?;
        let mut process = Command::new(&command.program);
        process
            .args(&command.args)
            .current_dir(cwd)
            .env_clear()
            .envs(&self.policy.environment)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let mut child = process.spawn().map_err(BlnkError::Io)?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| BlnkError::Stream("shell stdout pipe unavailable".into()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| BlnkError::Stream("shell stderr pipe unavailable".into()))?;
        let total = Arc::new(AtomicUsize::new(0));
        let stdout_task = tokio::spawn(read_limited(
            stdout,
            total.clone(),
            self.policy.max_output_bytes,
        ));
        let stderr_task = tokio::spawn(read_limited(stderr, total, self.policy.max_output_bytes));

        let status = tokio::select! {
            result = child.wait() => result.map_err(BlnkError::Io)?,
            _ = tokio::time::sleep(self.policy.timeout) => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                stdout_task.abort();
                stderr_task.abort();
                return Err(BlnkError::Stream("shell command timed out".into()));
            }
            _ = cancellation.cancelled() => {
                let _ = child.kill().await;
                let _ = child.wait().await;
                stdout_task.abort();
                stderr_task.abort();
                return Err(BlnkError::Stream("shell command cancelled".into()));
            }
        };

        let stdout = stdout_task
            .await
            .map_err(|error| BlnkError::Stream(format!("stdout task failed: {error}")))??;
        let stderr = stderr_task
            .await
            .map_err(|error| BlnkError::Stream(format!("stderr task failed: {error}")))??;
        let (exit_code, signaled) = exit_status(status);
        Ok(ShellRunResult {
            stdout,
            stderr,
            exit_code,
            signaled,
        })
    }

    /// Returns the local stream kind used by the runtime adapter.
    pub const fn stream_kind(&self) -> StreamKind {
        StreamKind::Shell
    }
}

async fn read_limited<R>(
    mut reader: R,
    total: Arc<AtomicUsize>,
    limit: usize,
) -> StreamResult<Vec<u8>>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut output = Vec::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let count = reader.read(&mut buffer).await.map_err(BlnkError::Io)?;
        if count == 0 {
            return Ok(output);
        }
        reserve_output(&total, count, limit)?;
        output.extend_from_slice(&buffer[..count]);
    }
}

fn reserve_output(total: &AtomicUsize, amount: usize, limit: usize) -> StreamResult<()> {
    let mut current = total.load(Ordering::Relaxed);
    loop {
        let next = current
            .checked_add(amount)
            .ok_or_else(|| BlnkError::Stream("shell output accounting overflow".into()))?;
        if next > limit {
            return Err(BlnkError::Stream(
                "shell output exceeds configured limit".into(),
            ));
        }
        match total.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return Ok(()),
            Err(observed) => current = observed,
        }
    }
}

fn exit_status(status: std::process::ExitStatus) -> (Option<i32>, bool) {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        (status.code(), status.signal().is_some())
    }
    #[cfg(not(unix))]
    {
        (status.code(), false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shell_program() -> String {
        #[cfg(windows)]
        {
            std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_owned())
        }
        #[cfg(not(windows))]
        {
            "/bin/sh".to_owned()
        }
    }

    fn shell_args(script: &str) -> Vec<String> {
        #[cfg(windows)]
        {
            vec!["/C".to_owned(), script.to_owned()]
        }
        #[cfg(not(windows))]
        {
            vec!["-c".to_owned(), script.to_owned()]
        }
    }

    fn capture_script() -> &'static str {
        #[cfg(windows)]
        {
            "echo(out&echo(err>&2&exit /b 7"
        }
        #[cfg(not(windows))]
        {
            "printf out; printf err >&2; exit 7"
        }
    }

    fn long_running_script() -> &'static str {
        #[cfg(windows)]
        {
            "for /L %i in (1,1,2147483647) do @rem"
        }
        #[cfg(not(windows))]
        {
            "sleep 1"
        }
    }

    fn oversized_script() -> &'static str {
        #[cfg(windows)]
        {
            "echo 123456789"
        }
        #[cfg(not(windows))]
        {
            "printf 123456789"
        }
    }

    fn expected_capture_stdout() -> &'static [u8] {
        #[cfg(windows)]
        {
            b"out\r\n"
        }
        #[cfg(not(windows))]
        {
            b"out"
        }
    }

    fn expected_capture_stderr() -> &'static [u8] {
        #[cfg(windows)]
        {
            b"err\r\n"
        }
        #[cfg(not(windows))]
        {
            b"err"
        }
    }

    fn test_policy() -> ShellPolicy {
        ShellPolicy::new(std::env::temp_dir())
            .expect("temp root")
            .allow_program(shell_program())
            .with_max_output_bytes(128)
            .expect("output limit")
            .with_timeout(Duration::from_millis(500))
            .expect("timeout")
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn pipe_shell_captures_stdout_and_stderr_and_exit_status() {
        let handler = ShellStreamHandler::new(test_policy());
        let result = handler
            .run(
                ShellCommand::new(shell_program()).args(shell_args(capture_script())),
                CancellationToken::new(),
            )
            .await
            .expect("shell should run");
        assert_eq!(result.stdout, expected_capture_stdout());
        assert_eq!(result.stderr, expected_capture_stderr());
        assert_eq!(result.exit_code, Some(7));
        assert!(!result.signaled);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn timeout_kills_long_running_command() {
        let handler = ShellStreamHandler::new(
            ShellPolicy::new(std::env::temp_dir())
                .expect("temp root")
                .allow_program(shell_program())
                .with_timeout(Duration::from_millis(20))
                .expect("timeout"),
        );
        let error = handler
            .run(
                ShellCommand::new(shell_program()).args(shell_args(long_running_script())),
                CancellationToken::new(),
            )
            .await
            .expect_err("command should time out");
        assert!(error.to_string().contains("timed out"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn cancellation_stops_long_running_command() {
        let handler = ShellStreamHandler::new(
            ShellPolicy::new(std::env::temp_dir())
                .expect("temp root")
                .allow_program(shell_program())
                .with_timeout(Duration::from_secs(2))
                .expect("timeout"),
        );
        let token = CancellationToken::new();
        let cancel = token.clone();
        let task = tokio::spawn(async move {
            handler
                .run(
                    ShellCommand::new(shell_program()).args(shell_args(long_running_script())),
                    token,
                )
                .await
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        cancel.cancel();
        let error = task
            .await
            .expect("task join")
            .expect_err("command should cancel");
        assert!(error.to_string().contains("cancelled"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn oversized_output_is_rejected_without_inheriting_environment() {
        let handler = ShellStreamHandler::new(
            ShellPolicy::new(std::env::temp_dir())
                .expect("temp root")
                .allow_program(shell_program())
                .with_max_output_bytes(8)
                .expect("output limit"),
        );
        let error = handler
            .run(
                ShellCommand::new(shell_program()).args(shell_args(oversized_script())),
                CancellationToken::new(),
            )
            .await
            .expect_err("output should exceed limit");
        assert!(error.to_string().contains("output exceeds"));
    }

    #[test]
    fn policy_rejects_unapproved_program_and_parent_directory() {
        let policy = test_policy();
        assert!(policy.validate(&ShellCommand::new("/usr/bin/id")).is_err());
        assert!(
            policy
                .validate(&ShellCommand::new(shell_program()).cwd("../outside"))
                .is_err()
        );
    }

    #[test]
    fn shell_frames_round_trip_and_reject_invalid_flags() {
        let command = ShellCommand::new(shell_program())
            .args(shell_args("printf hello"))
            .cwd("work");
        let open = encode_open_frame(7, &command).expect("open frame");
        assert!(open.flags.is_syn());
        assert!(open.flags.is_dat());
        assert!(!open.flags.is_fin());
        let decoded = decode_open_frame(&open).expect("open decode");
        assert_eq!(decoded, command);

        let input = encode_input_frame(7, b"stdin".to_vec(), false).expect("input frame");
        assert!(input.flags.is_more());
        assert_eq!(
            decode_input_frame(&input).expect("input decode").data,
            b"stdin"
        );

        let output = encode_output_frame(7, b"stdout".to_vec(), false).expect("output frame");
        assert_eq!(
            decode_output_frame(&output).expect("output decode").data,
            b"stdout"
        );

        let resize = encode_resize_frame(7, 120, 40).expect("resize frame");
        assert_eq!(
            decode_resize_frame(&resize).expect("resize decode").cols,
            120
        );

        let exit = encode_exit_frame(7, 3, false).expect("exit frame");
        assert!(exit.flags.is_fin());
        assert_eq!(decode_exit_frame(&exit).expect("exit decode").code, 3);

        let error = encode_error_frame(7, "denied").expect("error frame");
        assert_eq!(
            decode_error_frame(&error).expect("error decode").message,
            "denied"
        );

        let cancel = encode_cancel_frame(7).expect("cancel frame");
        assert!(
            decode_cancel_frame(&cancel)
                .expect("cancel decode")
                .requested
        );

        let invalid_open = Frame::new(
            7,
            FrameFlags::SYN | FrameFlags::DAT | FrameFlags::FIN,
            open.payload,
        );
        assert!(decode_open_frame(&invalid_open).is_err());
        let invalid_resize = encode_resize_frame(7, 80, 24).expect("resize frame");
        let invalid_resize =
            Frame::new(7, FrameFlags::DAT | FrameFlags::FIN, invalid_resize.payload);
        assert!(decode_resize_frame(&invalid_resize).is_err());
        assert!(encode_cancel_frame(0).is_err());
    }
}

/// A typed shell-stream frame directionally decoded by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellFrame {
    Open(wire::ShellOpen),
    Input(wire::ShellInput),
    Output(wire::ShellOutput),
    Resize(wire::TerminalResize),
    Exit(wire::ShellExit),
    Error(wire::ShellError),
    Cancel(wire::ShellCancel),
    Fin,
}

/// Builds a shell open SYN frame.
pub fn encode_open_frame(stream_id: u32, command: &ShellCommand) -> StreamResult<Frame> {
    if stream_id == 0 {
        return Err(BlnkError::Stream("shell stream id must be non-zero".into()));
    }
    let message = wire::ShellOpen {
        r#type: "shell".into(),
        program: command.program.clone(),
        args: command.args.clone(),
        cwd: command.cwd.to_string_lossy().into_owned(),
    };
    Ok(Frame::new(
        stream_id,
        FrameFlags::SYN | FrameFlags::DAT,
        message.encode_to_vec(),
    ))
}

/// Decodes a shell open SYN frame.
pub fn decode_open_frame(frame: &Frame) -> StreamResult<ShellCommand> {
    require_shell_frame(frame, true, false)?;
    let message = wire::ShellOpen::decode(frame.payload.as_slice())
        .map_err(|error| BlnkError::Protocol(format!("invalid shell open protobuf: {error}")))?;
    if message.r#type != "shell" || message.program.trim().is_empty() {
        return Err(BlnkError::Protocol("invalid shell open message".into()));
    }
    Ok(ShellCommand::new(message.program)
        .args(message.args)
        .cwd(message.cwd))
}

/// Builds a shell input frame. The final input frame carries FIN.
pub fn encode_input_frame(stream_id: u32, data: Vec<u8>, final_chunk: bool) -> StreamResult<Frame> {
    encode_data_message(stream_id, wire::ShellInput { data }, final_chunk)
}

/// Decodes a shell input frame.
pub fn decode_input_frame(frame: &Frame) -> StreamResult<wire::ShellInput> {
    decode_data_message(frame, "shell input")
}

/// Builds a terminal resize frame.
pub fn encode_resize_frame(stream_id: u32, cols: i32, rows: i32) -> StreamResult<Frame> {
    if cols <= 0 || rows <= 0 {
        return Err(BlnkError::Stream(
            "terminal dimensions must be positive".into(),
        ));
    }
    encode_data_message(stream_id, wire::TerminalResize { cols, rows }, false)
}

/// Decodes a terminal resize frame.
pub fn decode_resize_frame(frame: &Frame) -> StreamResult<wire::TerminalResize> {
    let resize: wire::TerminalResize = decode_data_message(frame, "terminal resize")?;
    if resize.cols <= 0 || resize.rows <= 0 {
        return Err(BlnkError::Protocol(
            "terminal dimensions must be positive".into(),
        ));
    }
    Ok(resize)
}

/// Builds one stdout/stderr output frame.
pub fn encode_output_frame(
    stream_id: u32,
    data: Vec<u8>,
    final_chunk: bool,
) -> StreamResult<Frame> {
    encode_data_message(stream_id, wire::ShellOutput { data }, final_chunk)
}

/// Decodes one stdout/stderr output frame.
pub fn decode_output_frame(frame: &Frame) -> StreamResult<wire::ShellOutput> {
    decode_data_message(frame, "shell output")
}

/// Builds a process-exit frame. Exit is terminal and always carries FIN.
pub fn encode_exit_frame(stream_id: u32, code: i32, signaled: bool) -> StreamResult<Frame> {
    encode_terminal_message(stream_id, wire::ShellExit { code, signaled })
}

/// Decodes a process-exit frame.
pub fn decode_exit_frame(frame: &Frame) -> StreamResult<wire::ShellExit> {
    decode_terminal_message(frame, "shell exit")
}

/// Builds a policy/process error frame. Errors are terminal and carry FIN.
pub fn encode_error_frame(stream_id: u32, message: impl Into<String>) -> StreamResult<Frame> {
    let message = message.into();
    if message.trim().is_empty() {
        return Err(BlnkError::Stream(
            "shell error message must not be empty".into(),
        ));
    }
    encode_terminal_message(stream_id, wire::ShellError { message })
}

/// Decodes a shell error frame.
pub fn decode_error_frame(frame: &Frame) -> StreamResult<wire::ShellError> {
    decode_terminal_message(frame, "shell error")
}

/// Builds a cancellation frame. Cancellation is terminal and carries FIN.
pub fn encode_cancel_frame(stream_id: u32) -> StreamResult<Frame> {
    encode_terminal_message(stream_id, wire::ShellCancel { requested: true })
}

/// Decodes a cancellation frame.
pub fn decode_cancel_frame(frame: &Frame) -> StreamResult<wire::ShellCancel> {
    let cancel: wire::ShellCancel = decode_terminal_message(frame, "shell cancellation")?;
    if !cancel.requested {
        return Err(BlnkError::Protocol(
            "shell cancellation must set requested=true".into(),
        ));
    }
    Ok(cancel)
}

fn encode_data_message<M: Message>(
    stream_id: u32,
    message: M,
    final_chunk: bool,
) -> StreamResult<Frame> {
    if stream_id == 0 {
        return Err(BlnkError::Stream("shell stream id must be non-zero".into()));
    }
    let flags = if final_chunk {
        FrameFlags::DAT | FrameFlags::FIN
    } else {
        FrameFlags::DAT | FrameFlags::MORE
    };
    Ok(Frame::new(stream_id, flags, message.encode_to_vec()))
}

fn encode_terminal_message<M: Message>(stream_id: u32, message: M) -> StreamResult<Frame> {
    if stream_id == 0 {
        return Err(BlnkError::Stream("shell stream id must be non-zero".into()));
    }
    Ok(Frame::new(
        stream_id,
        FrameFlags::DAT | FrameFlags::FIN,
        message.encode_to_vec(),
    ))
}

fn decode_data_message<M: Message + Default>(frame: &Frame, label: &str) -> StreamResult<M> {
    require_shell_frame(frame, false, false)?;
    M::decode(frame.payload.as_slice())
        .map_err(|error| BlnkError::Protocol(format!("invalid {label} protobuf: {error}")))
}

fn decode_terminal_message<M: Message + Default>(frame: &Frame, label: &str) -> StreamResult<M> {
    require_shell_frame(frame, false, true)?;
    M::decode(frame.payload.as_slice())
        .map_err(|error| BlnkError::Protocol(format!("invalid {label} protobuf: {error}")))
}

fn require_shell_frame(frame: &Frame, syn: bool, fin: bool) -> StreamResult<()> {
    if frame.stream_id == 0 || !frame.flags.is_dat() || frame.flags.is_syn() != syn {
        return Err(BlnkError::Protocol(
            "shell frame must be a non-control DAT frame with the expected SYN flag".into(),
        ));
    }
    if frame.flags.is_fin() != fin {
        return Err(BlnkError::Protocol(
            "shell frame has an invalid FIN flag".into(),
        ));
    }
    Ok(())
}
