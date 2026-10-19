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
        control: Receiver::from_owned_fd(control.into())?,
        events: Sender::from_owned_fd(events.into())?,
    }))
}

struct ScriptedGateway {
    command: String,
    cancellation: tokio_util::sync::CancellationToken,
    control: Receiver,
    events: Sender,
}

impl ScriptedGateway {
    fn event(&self, text: &str) -> std::io::Result<()> {
        let line = format!("{text}\n");
        match self.events.try_write(line.as_bytes())? {
            n if n == line.len() => Ok(()),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::WriteZero,
                "partial test event",
            )),
        }
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
impl LlmGateway for ScriptedGateway {
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
                self.event("stream-ready")?;
                tokio::select! {
                    result = self.advance() => { result?; }
                    _ = self.cancellation.cancelled() => return Err(mojentic::error::MojenticError::GatewayError("scripted request cancelled".into())),
                }
                Ok(StreamChunk::Content("The reply continues after SIGHUP".into()))
            }))
            .chain(stream::once(async move {
                self.event("stream-advanced")?;
                self.cancellation.cancelled().await;
                Err(mojentic::error::MojenticError::GatewayError("scripted request cancelled".into()))
            })).boxed()
    }
}
