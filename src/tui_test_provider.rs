//! Socket-free gateway for executable TUI tests, excluded from every release profile.
use async_trait::async_trait;
use futures::{StreamExt, stream};
use mojentic::llm::gateway::{ModelContextWindow, StreamChunk};
use mojentic::llm::models::{LlmGatewayResponse, LlmToolCall};
use mojentic::llm::{CompletionConfig, LlmGateway, LlmMessage, LlmTool};
use serde_json::{Value, json};
use std::io::Write;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::unix::pipe::Receiver;

pub(crate) fn gateway(
    cancellation: tokio_util::sync::CancellationToken,
) -> Option<Box<dyn LlmGateway>> {
    std::env::var_os("MICROGENTIC_TUI_SCRIPT")
        .map(|root| Box::new(Gateway(PathBuf::from(root), cancellation)) as Box<dyn LlmGateway>)
}

struct Gateway(PathBuf, tokio_util::sync::CancellationToken);
fn error(error: impl std::fmt::Display) -> mojentic::error::MojenticError {
    mojentic::error::MojenticError::GatewayError(error.to_string())
}
impl Gateway {
    fn metadata(&self, operation: &str, model: Option<&str>) -> mojentic::Result<Value> {
        let mut log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.0.join("metadata.jsonl"))
            .map_err(error)?;
        writeln!(log, "{}", json!({"operation":operation,"model":model})).map_err(error)?;
        serde_json::from_slice(&std::fs::read(self.0.join("metadata.json")).map_err(error)?)
            .map_err(error)
    }
}
#[async_trait]
impl LlmGateway for Gateway {
    async fn complete(
        &self,
        _: &str,
        _: &[LlmMessage],
        _: Option<&[Box<dyn LlmTool>]>,
        _: &CompletionConfig,
    ) -> mojentic::Result<LlmGatewayResponse> {
        Err(error("unexpected non-stream completion"))
    }
    async fn complete_json(
        &self,
        _: &str,
        _: &[LlmMessage],
        _: Value,
        _: &CompletionConfig,
    ) -> mojentic::Result<Value> {
        Err(error("unexpected JSON completion"))
    }
    async fn calculate_embeddings(&self, _: &str, _: Option<&str>) -> mojentic::Result<Vec<f32>> {
        Err(error("unexpected embeddings"))
    }
    async fn get_available_models(&self) -> mojentic::Result<Vec<String>> {
        let metadata = self.metadata("models", None)?;
        if let Some(message) = metadata["models_error"].as_str() {
            return Err(error(message));
        }
        serde_json::from_value(metadata["models"].clone()).map_err(error)
    }
    async fn get_model_context_window(
        &self,
        model: &str,
    ) -> mojentic::Result<Option<ModelContextWindow>> {
        serde_json::from_value(self.metadata("window", Some(model))?["window"].clone())
            .map_err(error)
    }
    fn complete_stream<'a>(
        &'a self,
        model: &'a str,
        messages: &'a [LlmMessage],
        tools: Option<&'a [Box<dyn LlmTool>]>,
        config: &'a CompletionConfig,
    ) -> futures::stream::BoxStream<'a, mojentic::Result<StreamChunk>> {
        let mut request = json!({"model":model,"messages":messages,
            "tools":tools.map(|tools| tools.iter().map(|tool| tool.descriptor()).collect::<Vec<_>>()),
            "completion":format!("{config:?}"),"effort":format!("{:?}",config.reasoning_effort),
            "num_ctx":config.num_ctx,"max_tokens":config.max_tokens,"temperature":config.temperature});
        let root = self.0.clone();
        let cancellation = self.1.clone();
        stream::once(async move {
            // Opening and writing named pipes can block. Only their asynchronous
            // read half runs on the runtime, so cancellation can interrupt it.
            let reader = tokio::task::spawn_blocking(move || {
                static NEXT: std::sync::atomic::AtomicUsize =
                    std::sync::atomic::AtomicUsize::new(0);
                let reply = root.join(format!(
                    "reply-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                ));
                let status = std::process::Command::new("mkfifo")
                    .arg(&reply)
                    .status()
                    .map_err(error)?;
                if !status.success() {
                    return Err(error("could not create reply pipe"));
                }
                let reader = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(&reply)
                    .map_err(error)?;
                request["reply"] = json!(reply);
                let mut writer = std::fs::OpenOptions::new()
                    .write(true)
                    .open(root.join("requests"))
                    .map_err(error)?;
                writeln!(writer, "{request}").map_err(error)?;
                Ok(reader)
            })
            .await
            .map_err(error)??;
            Ok::<_, mojentic::error::MojenticError>(BufReader::new(
                Receiver::from_owned_fd(reader.into()).map_err(error)?,
            ))
        })
        .flat_map(move |reader| {
            let cancellation = cancellation.clone();
            stream::unfold(Some(reader), move |state| {
                let cancellation = cancellation.clone();
                async move {
                    let mut reader = match state? {
                        Ok(reader) => reader,
                        Err(error) => return Some((Err(error), None)),
                    };
                    let mut line = String::new();
                    let read = tokio::select! {
                        result = reader.read_line(&mut line) => result,
                        _ = cancellation.cancelled() => {
                            return Some((Err(error("scripted stream cancelled")), None));
                        }
                    };
                    let chunk = match read {
                        Ok(0) => return None,
                        Err(e) => Err(error(e)),
                        Ok(_) => decode_frame(&line),
                    };
                    match chunk {
                        Ok(None) => None,
                        Ok(Some(chunk)) => Some((Ok(chunk), Some(Ok(reader)))),
                        Err(error) => Some((Err(error), None)),
                    }
                }
            })
        })
        .boxed()
    }
}

fn decode_frame(line: &str) -> mojentic::Result<Option<StreamChunk>> {
    let value: Value = serde_json::from_str(line).map_err(error)?;
    if value["done"] == true {
        return Ok(None);
    }
    if let Some(text) = value["content"].as_str() {
        return Ok(Some(StreamChunk::Content(text.into())));
    }
    if let Some(text) = value["thinking"].as_str() {
        return Ok(Some(StreamChunk::Thinking(text.into())));
    }
    if let Some(calls) = value.get("tools") {
        let calls: Vec<LlmToolCall> = serde_json::from_value(calls.clone()).map_err(error)?;
        return Ok(Some(StreamChunk::ToolCalls(calls)));
    }
    Err(error(
        value["error"].as_str().unwrap_or("invalid scripted frame"),
    ))
}
