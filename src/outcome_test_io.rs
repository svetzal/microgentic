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
        _: &str,
    ) -> mojentic::Result<Option<ModelContextWindow>> {
        self.discoveries.fetch_add(1, Ordering::SeqCst);
        Ok(None)
    }
    fn complete_stream<'a>(
        &'a self,
        _: &'a str,
        _: &'a [LlmMessage],
        _: Option<&'a [Box<dyn LlmTool>]>,
        _: &'a CompletionConfig,
    ) -> futures::stream::BoxStream<'a, mojentic::Result<StreamChunk>> {
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
