//! Task-scoped gateway and intake doubles for CLI outcome tests. Never compiled into binaries.
use async_trait::async_trait;
use futures::{StreamExt, stream};
use mojentic::llm::gateway::{ModelContextWindow, StreamChunk};
use mojentic::llm::models::LlmGatewayResponse;
use mojentic::llm::{CompletionConfig, LlmGateway, LlmMessage, LlmTool};
use serde_json::Value;
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

tokio::task_local! {
    pub(crate) static ACTIVE: Arc<OutcomeIo>;
}

pub(crate) struct OutcomeIo {
    pub(crate) state: PathBuf,
    pub(crate) tool_root: PathBuf,
    pub(crate) streams: Mutex<VecDeque<Vec<StreamChunk>>>,
    pub(crate) calls: AtomicUsize,
    pub(crate) discoveries: AtomicUsize,
    pub(crate) stdin: Option<String>,
    pub(crate) metadata: Option<ModelContextWindow>,
    pub(crate) requests: Mutex<Vec<Value>>,
    pub(crate) metadata_models: Mutex<Vec<String>>,
}

#[async_trait]
impl LlmGateway for OutcomeIo {
    async fn complete(
        &self,
        _: &str,
        _: &[LlmMessage],
        _: Option<&[Box<dyn LlmTool>]>,
        _: &CompletionConfig,
    ) -> mojentic::Result<LlmGatewayResponse> {
        panic!("unexpected advisory completion: intercepted before any transport")
    }
    async fn complete_json(
        &self,
        _: &str,
        _: &[LlmMessage],
        _: Value,
        _: &CompletionConfig,
    ) -> mojentic::Result<Value> {
        panic!("unexpected JSON advisory: intercepted before any transport")
    }
    async fn get_available_models(&self) -> mojentic::Result<Vec<String>> {
        panic!("unexpected model listing: intercepted before any transport")
    }
    async fn calculate_embeddings(&self, _: &str, _: Option<&str>) -> mojentic::Result<Vec<f32>> {
        panic!("unexpected embedding request: intercepted before any transport")
    }
    async fn get_model_context_window(
        &self,
        model: &str,
    ) -> mojentic::Result<Option<ModelContextWindow>> {
        self.discoveries.fetch_add(1, Ordering::SeqCst);
        self.metadata_models.lock().unwrap().push(model.to_owned());
        Ok(self.metadata.clone())
    }
    fn complete_stream<'a>(
        &'a self,
        model: &'a str,
        messages: &'a [LlmMessage],
        tools: Option<&'a [Box<dyn LlmTool>]>,
        config: &'a CompletionConfig,
    ) -> futures::stream::BoxStream<'a, mojentic::Result<StreamChunk>> {
        self.requests.lock().unwrap().push(serde_json::json!({
            "model": model,
            "messages": messages,
            "tools": tools.map(|tools| tools.iter().map(|tool| tool.descriptor()).collect::<Vec<_>>()),
            // Complete debug representation retains every completion control,
            // including fields without serde implementations. This is a gateway
            // snapshot, not a reconstruction of either provider's wire request.
            "completion": format!("{config:?}"),
            "num_ctx": config.num_ctx,
            "max_tokens": config.max_tokens,
            "temperature": config.temperature,
        }));
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut streams = self.streams.lock().unwrap();
        let chunks = if streams.len() > 1 {
            streams.pop_front().unwrap()
        } else {
            streams.front().cloned().unwrap_or_default()
        };
        stream::iter(chunks.into_iter().map(Ok)).boxed()
    }
}
