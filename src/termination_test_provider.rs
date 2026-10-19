//! Gateway substitution for real-process signal tests. Compiled only with the
//! explicit test feature and the development profile; release artifacts have no hook.
use async_trait::async_trait;
use futures::{StreamExt, stream};
use mojentic::llm::gateway::StreamChunk;
use mojentic::llm::models::{LlmGatewayResponse, LlmToolCall};
use mojentic::llm::{CompletionConfig, LlmGateway, LlmMessage, LlmTool};
use serde_json::Value;
use std::fs::OpenOptions;
use tokio::net::unix::pipe::{Receiver, Sender};

pub(crate) async fn override_gateway(
    original: Box<dyn LlmGateway>,
    cancellation: tokio_util::sync::CancellationToken,
) -> anyhow::Result<Box<dyn LlmGateway>> {
    let Some(root) = std::env::var_os("MICROGENTIC_TERMINATION_SCRIPT") else {
        return Ok(original);
    };
    let root = std::path::PathBuf::from(root);
    let (command, control, events) = tokio::task::spawn_blocking(move || -> std::io::Result<_> {
        let command = std::fs::read_to_string(root.join("command"))?;
        let control = OpenOptions::new()
            .read(true)
            .write(true)
            .open(root.join("control"))?;
        let events = OpenOptions::new().write(true).open(root.join("events"))?;
        Ok((command, control, events))
    })
    .await??;
    Ok(Box::new(ScriptedGateway {
        command,
        cancellation,
        io: PipeIo {
            control: Receiver::from_owned_fd(control.into())?,
            events: Sender::from_owned_fd(events.into())?,
        },
    }))
}

struct ScriptedGateway<I = PipeIo> {
    command: String,
    cancellation: tokio_util::sync::CancellationToken,
    io: I,
}

impl<I: ScriptIo> ScriptedGateway<I> {
    async fn event(&self, text: &str) -> std::io::Result<()> {
        let line = format!("{text}\n");
        let mut remaining = line.as_bytes();
        while !remaining.is_empty() {
            self.io.writable().await?;
            match self.io.try_write(remaining) {
                Ok(0) => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::WriteZero,
                        "could not write test event",
                    ));
                }
                Ok(written) => remaining = &remaining[written..],
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
}

// The named pipes are the I/O boundary; tests supply deterministic backpressure.
#[async_trait]
trait ScriptIo: Send + Sync {
    async fn writable(&self) -> std::io::Result<()>;
    fn try_write(&self, bytes: &[u8]) -> std::io::Result<usize>;
    async fn advance(&self) -> std::io::Result<()>;
}

struct PipeIo {
    control: Receiver,
    events: Sender,
}

#[async_trait]
impl ScriptIo for PipeIo {
    async fn writable(&self) -> std::io::Result<()> {
        self.events.writable().await
    }

    fn try_write(&self, bytes: &[u8]) -> std::io::Result<usize> {
        self.events.try_write(bytes)
    }

    async fn advance(&self) -> std::io::Result<()> {
        loop {
            self.control.readable().await?;
            match self.control.try_read(&mut [0u8; 1]) {
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => continue,
                result => return result.map(|_| ()),
            }
        }
    }
}

fn unsupported<T>() -> mojentic::Result<T> {
    Err(mojentic::error::MojenticError::GatewayError(
        "unexpected termination-test gateway operation".into(),
    ))
}

#[async_trait]
impl<I: ScriptIo> LlmGateway for ScriptedGateway<I> {
    async fn complete(
        &self,
        _: &str,
        _: &[LlmMessage],
        _: Option<&[Box<dyn LlmTool>]>,
        _: &CompletionConfig,
    ) -> mojentic::Result<LlmGatewayResponse> {
        unsupported()
    }
    async fn complete_json(
        &self,
        _: &str,
        _: &[LlmMessage],
        _: Value,
        _: &CompletionConfig,
    ) -> mojentic::Result<Value> {
        unsupported()
    }
    async fn get_available_models(&self) -> mojentic::Result<Vec<String>> {
        unsupported()
    }
    async fn calculate_embeddings(&self, _: &str, _: Option<&str>) -> mojentic::Result<Vec<f32>> {
        unsupported()
    }

    fn complete_stream<'a>(
        &'a self,
        _: &'a str,
        _: &'a [LlmMessage],
        _: Option<&'a [Box<dyn LlmTool>]>,
        _: &'a CompletionConfig,
    ) -> futures::stream::BoxStream<'a, mojentic::Result<StreamChunk>> {
        if self.cancellation.is_cancelled() {
            return stream::once(async {
                Err(mojentic::error::MojenticError::GatewayError(
                    "scripted request cancelled".into(),
                ))
            })
            .boxed();
        }
        if !self.command.is_empty() {
            return stream::iter(vec![Ok(StreamChunk::ToolCalls(vec![LlmToolCall {
                id: Some("call-shell_command".into()),
                name: "shell_command".into(),
                arguments: std::collections::HashMap::from([(
                    "command".into(),
                    Value::String(self.command.clone()),
                )]),
            }]))])
            .boxed();
        }
        stream::once(async { Ok(StreamChunk::Content("The reply begins".into())) })
            .chain(stream::once(async move {
                self.event("stream-ready").await?;
                tokio::select! {
                    result = self.io.advance() => { result?; }
                    _ = self.cancellation.cancelled() => return Err(mojentic::error::MojenticError::GatewayError("scripted request cancelled".into())),
                }
                Ok(StreamChunk::Content("The reply continues after SIGHUP".into()))
            }))
            .chain(stream::once(async move {
                self.event("stream-advanced").await?;
                self.cancellation.cancelled().await;
                Err(mojentic::error::MojenticError::GatewayError("scripted request cancelled".into()))
            })).boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::io::{Error, ErrorKind};
    use std::sync::Mutex;
    use std::task::Poll;

    struct WriteStep {
        expected: &'static [u8],
        result: Result<usize, ErrorKind>,
    }

    struct FakeIo {
        writes: Mutex<VecDeque<WriteStep>>,
        delivered: Mutex<Vec<u8>>,
        readiness_error: Option<ErrorKind>,
    }

    impl FakeIo {
        fn new(steps: Vec<WriteStep>) -> Self {
            Self {
                writes: Mutex::new(steps.into()),
                delivered: Mutex::new(Vec::new()),
                readiness_error: None,
            }
        }
    }

    #[async_trait]
    impl ScriptIo for FakeIo {
        async fn writable(&self) -> std::io::Result<()> {
            // A scheduler turn, without a timer, makes premature stream progress visible.
            let mut pending = true;
            futures::future::poll_fn(|cx| {
                if pending {
                    pending = false;
                    cx.waker().wake_by_ref();
                    Poll::Pending
                } else {
                    Poll::Ready(self.readiness_error.map_or(Ok(()), |kind| Err(kind.into())))
                }
            })
            .await
        }

        fn try_write(&self, bytes: &[u8]) -> std::io::Result<usize> {
            let step = self
                .writes
                .lock()
                .unwrap()
                .pop_front()
                .expect("extra write");
            assert_eq!(bytes, step.expected, "must resume at the unwritten suffix");
            let n = step.result.map_err(Error::from)?;
            self.delivered
                .lock()
                .unwrap()
                .extend_from_slice(&bytes[..n]);
            Ok(n)
        }

        async fn advance(&self) -> std::io::Result<()> {
            assert_eq!(*self.delivered.lock().unwrap(), b"stream-ready\n");
            Ok(())
        }
    }

    fn gateway(io: FakeIo) -> ScriptedGateway<FakeIo> {
        ScriptedGateway {
            command: String::new(),
            cancellation: tokio_util::sync::CancellationToken::new(),
            io,
        }
    }

    fn step(expected: &'static [u8], result: Result<usize, ErrorKind>) -> WriteStep {
        WriteStep { expected, result }
    }

    fn assert_content(chunk: Option<mojentic::Result<StreamChunk>>, expected: &str) {
        match chunk.unwrap().unwrap() {
            StreamChunk::Content(text) => assert_eq!(text, expected),
            other => panic!("expected content, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn readiness_delivery_survives_backpressure_and_partial_progress() {
        let gateway = gateway(FakeIo::new(vec![
            step(b"stream-ready\n", Err(ErrorKind::WouldBlock)),
            step(b"stream-ready\n", Ok(3)),
            step(b"eam-ready\n", Err(ErrorKind::WouldBlock)),
            step(b"eam-ready\n", Ok(10)),
            step(b"stream-advanced\n", Ok(7)),
            step(b"advanced\n", Err(ErrorKind::WouldBlock)),
            step(b"advanced\n", Ok(9)),
        ]));
        let config = CompletionConfig::default();
        let mut stream = gateway.complete_stream("model", &[], None, &config);
        assert_content(stream.next().await, "The reply begins");
        // Readiness must finish before the next content can reach the caller.
        assert!(futures::poll!(stream.next()).is_pending());
        assert!(gateway.io.delivered.lock().unwrap().is_empty());
        assert_content(stream.next().await, "The reply continues after SIGHUP");
        assert_eq!(*gateway.io.delivered.lock().unwrap(), b"stream-ready\n");
        assert!(futures::poll!(stream.next()).is_pending());
        // Each partial write and false readiness can suspend independently.
        while !gateway.io.writes.lock().unwrap().is_empty() {
            assert!(futures::poll!(stream.next()).is_pending());
        }
        assert_eq!(
            *gateway.io.delivered.lock().unwrap(),
            b"stream-ready\nstream-advanced\n"
        );
        gateway.cancellation.cancel();
        let error = stream.next().await.unwrap().unwrap_err();
        assert!(error.to_string().contains("scripted request cancelled"));
        assert!(stream.next().await.is_none());
        assert_eq!(
            *gateway.io.delivered.lock().unwrap(),
            b"stream-ready\nstream-advanced\n"
        );
    }

    #[tokio::test]
    async fn terminal_write_errors_preserve_progress_and_reach_the_stream_caller() {
        for advanced in [false, true] {
            for (result, expected_kind) in [
                (Err(ErrorKind::BrokenPipe), ErrorKind::BrokenPipe),
                (
                    Err(ErrorKind::PermissionDenied),
                    ErrorKind::PermissionDenied,
                ),
                (Ok(0), ErrorKind::WriteZero),
            ] {
                let mut steps = Vec::new();
                let mut expected = Vec::new();
                if advanced {
                    steps.push(step(b"stream-ready\n", Ok(13)));
                    expected.extend_from_slice(b"stream-ready\n");
                    steps.push(step(b"stream-advanced\n", Ok(7)));
                    steps.push(step(b"advanced\n", result));
                    expected.extend_from_slice(b"stream-");
                } else {
                    steps.push(step(b"stream-ready\n", Ok(3)));
                    steps.push(step(b"eam-ready\n", result));
                    expected.extend_from_slice(b"str");
                }
                let gateway = gateway(FakeIo::new(steps));
                let config = CompletionConfig::default();
                let mut stream = gateway.complete_stream("model", &[], None, &config);
                assert_content(stream.next().await, "The reply begins");
                if advanced {
                    assert_content(stream.next().await, "The reply continues after SIGHUP");
                }
                match stream.next().await.unwrap().unwrap_err() {
                    mojentic::error::MojenticError::IoError(error) => {
                        assert_eq!(error.kind(), expected_kind);
                    }
                    error => panic!("expected original I/O error, got {error:?}"),
                }
                assert!(gateway.io.writes.lock().unwrap().is_empty());
                assert_eq!(*gateway.io.delivered.lock().unwrap(), expected);
            }
        }
    }

    #[tokio::test]
    async fn terminal_readiness_error_reaches_the_stream_caller_without_writing() {
        let mut io = FakeIo::new(Vec::new());
        io.readiness_error = Some(ErrorKind::BrokenPipe);
        let gateway = gateway(io);
        let config = CompletionConfig::default();
        let mut stream = gateway.complete_stream("model", &[], None, &config);
        assert_content(stream.next().await, "The reply begins");
        match stream.next().await.unwrap().unwrap_err() {
            mojentic::error::MojenticError::IoError(error) => {
                assert_eq!(error.kind(), ErrorKind::BrokenPipe);
            }
            error => panic!("expected original readiness error, got {error:?}"),
        }
        assert!(gateway.io.delivered.lock().unwrap().is_empty());
    }
}
