//! vLLM and LM Studio: OpenAI-compatible servers reached through Mojentic's
//! oMLX gateway.
//!
//! Mojentic has no gateway for either server. Both speak OpenAI chat
//! completions under `/v1`, as oMLX does, so this adapter lets the oMLX gateway
//! build each request and then rewrites the JSON body for the server's dialect.
//! [`rewrite_body`] is the whole difference between the three servers on the
//! wire. Mojentic's recovery engine sends the request and reads the reply as
//! an oMLX reply.
//!
//! Known limit: Mojentic 3.0.1 reads reasoning from `reasoning_content` only.
//! Current vLLM reports it as `reasoning`, so vLLM reasoning text does not
//! reach Microgentic. Answers and tool calls are not affected.
use async_trait::async_trait;
use futures::Stream;
use mojentic::error::{MojenticError, Result};
use mojentic::llm::gateway::{
    CompletionConfig, LlmGateway, ModelContextWindow, ReasoningEffort, StreamChunk,
};
use mojentic::llm::gateways::{OmlxConfig, OmlxGateway};
use mojentic::llm::models::{LlmGatewayResponse, LlmMessage};
use mojentic::llm::recovery::{RecoveryGateway, RecoveryPolicy, RecoveryProvider};
use mojentic::llm::stream_events::{StreamEventError, StreamEventStream};
use mojentic::llm::tools::LlmTool;
use serde_json::{Value, json};
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::time::Duration;

/// Bounds requests that do not stream, including model load: ten minutes.
const TIMEOUT: Duration = Duration::from_secs(600);

/// Bounds the LM Studio model metadata request.
const METADATA_TIMEOUT: Duration = Duration::from_secs(10);

/// The server whose request form the adapter produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dialect {
    Vllm,
    Lmstudio,
}

impl Dialect {
    fn name(self) -> &'static str {
        match self {
            Self::Vllm => "vLLM",
            Self::Lmstudio => "LM Studio",
        }
    }

    fn default_host(self) -> &'static str {
        match self {
            Self::Vllm => "http://localhost:8000",
            Self::Lmstudio => "http://localhost:1234",
        }
    }

    fn host_variable(self) -> &'static str {
        match self {
            Self::Vllm => "VLLM_HOST",
            Self::Lmstudio => "LMSTUDIO_HOST",
        }
    }

    pub(crate) fn key_variable(self) -> &'static str {
        match self {
            Self::Vllm => "VLLM_API_KEY",
            Self::Lmstudio => "LM_API_TOKEN",
        }
    }
}

/// Rewrite a chat completion body from the oMLX gateway for `dialect`.
///
/// The oMLX gateway sends `enable_thinking` at the top level for the
/// `disabled` and `enabled` efforts, `reasoning_effort` for the named levels,
/// and `chat_template_kwargs.preserve_thinking` when reasoning is preserved.
///
/// - vLLM ignores a top-level `enable_thinking`; it reads the flag from
///   `chat_template_kwargs`. It accepts `reasoning_effort` as sent.
/// - LM Studio documents neither `enable_thinking` nor `chat_template_kwargs`
///   on `/v1/chat/completions`. `disabled` becomes `reasoning_effort: "none"`
///   and `enabled` sends nothing, which keeps the model's default. LM Studio
///   documents that this endpoint accepts `reasoning_effort` (0.4.8 and
///   later) but does not document the accepted values, so `"none"` and the
///   named levels are sent on that basis and are not verified here.
/// - Both servers report token usage on a stream only when asked.
///
/// Every other field, including `reasoning_content` in message history, is
/// left as the oMLX gateway wrote it.
pub(crate) fn rewrite_body(dialect: Dialect, mut body: Value) -> Value {
    let Some(fields) = body.as_object_mut() else {
        return body;
    };
    let thinking = fields.remove("enable_thinking");
    match dialect {
        Dialect::Vllm => {
            if let Some(thinking) = thinking {
                let kwargs = fields
                    .entry("chat_template_kwargs")
                    .or_insert_with(|| json!({}));
                if let Some(kwargs) = kwargs.as_object_mut() {
                    kwargs.insert("enable_thinking".into(), thinking);
                }
            }
        }
        Dialect::Lmstudio => {
            fields.remove("chat_template_kwargs");
            if thinking == Some(Value::Bool(false)) {
                fields.insert("reasoning_effort".into(), json!("none"));
            }
        }
    }
    if fields.get("stream") == Some(&Value::Bool(true)) {
        fields.insert("stream_options".into(), json!({"include_usage": true}));
    }
    body
}

/// The output and reasoning controls a streamed worker request carries for
/// `dialect`, for the provider-request trace. The controls pass through
/// [`rewrite_body`], so the trace records what the wire carries.
pub(crate) fn provider_controls(dialect: Dialect, config: &CompletionConfig) -> Value {
    // The same mapping as the oMLX gateway's request body.
    let mut body = json!({"stream": true});
    if let Some(max_tokens) = config.max_tokens {
        body["max_tokens"] = json!(max_tokens);
    }
    if config.preserve_reasoning {
        body["chat_template_kwargs"] = json!({"preserve_thinking": true});
    }
    match config.reasoning_effort {
        Some(ReasoningEffort::Disabled) => body["enable_thinking"] = json!(false),
        Some(ReasoningEffort::Enabled) => body["enable_thinking"] = json!(true),
        Some(effort) => body["reasoning_effort"] = json!(effort),
        None => {}
    }
    let mut controls = rewrite_body(dialect, body);
    if let Some(fields) = controls.as_object_mut() {
        fields.remove("stream");
        fields.insert(
            "context_window_source".into(),
            json!("server_model_configuration"),
        );
    }
    controls
}

/// The context window of `model` in the reply of LM Studio's native
/// `GET /api/v1/models`. A loaded instance's `config.context_length` is the
/// limit the server runs with, so it wins over the model's maximum.
pub(crate) fn lmstudio_context_window(models: &Value, model: &str) -> Option<ModelContextWindow> {
    fn instances(entry: &Value) -> impl Iterator<Item = &Value> {
        entry["loaded_instances"].as_array().into_iter().flatten()
    }
    fn window(value: &Value, field: &str) -> Option<ModelContextWindow> {
        let tokens = usize::try_from(value.as_u64()?).ok()?;
        Some(ModelContextWindow {
            max_context_tokens: NonZeroUsize::new(tokens)?,
            field: field.into(),
            model_num_ctx: None,
        })
    }
    let entry = models["models"].as_array()?.iter().find(|entry| {
        entry["key"] == model || instances(entry).any(|instance| instance["id"] == model)
    })?;
    // An instance named by the model ID wins over another instance of the model.
    let named = instances(entry).filter(|instance| instance["id"] == model);
    named
        .chain(instances(entry))
        .find_map(|instance| {
            window(
                &instance["config"]["context_length"],
                "loaded_instances.config.context_length",
            )
        })
        .or_else(|| window(&entry["max_context_length"], "max_context_length"))
}

/// The host and key for `dialect` from the values of its environment
/// variables. An empty value counts as not set.
fn endpoint(
    dialect: Dialect,
    host: Option<String>,
    key: Option<String>,
) -> (String, Option<String>) {
    (
        host.filter(|host| !host.trim().is_empty())
            .unwrap_or_else(|| dialect.default_host().into()),
        key.filter(|key| !key.trim().is_empty()),
    )
}

/// Gateway for a vLLM or LM Studio server.
///
/// Completions go through Mojentic's recovery engine, which takes each request
/// from [`RecoveryProvider::prepare_recovery`]. The worker wraps this gateway
/// in its own recovery policy. Other callers use [`Self::direct`]. The
/// completion methods of [`LlmGateway`] on this type refuse to run: they would
/// send the oMLX request form.
pub(crate) struct OpenAiCompatibleGateway {
    dialect: Dialect,
    inner: OmlxGateway,
    client: reqwest::Client,
    host: String,
    api_key: Option<String>,
    /// Bounds a request that does not stream. The worker sets no bound, like
    /// the oMLX gateway under recovery.
    non_streaming_timeout: Option<Duration>,
}

impl OpenAiCompatibleGateway {
    /// A gateway for the server at `host` (without `/v1`). `api_key` is sent
    /// as a bearer token; no header is sent without one.
    pub(crate) fn new(dialect: Dialect, host: String, api_key: Option<String>) -> Self {
        Self {
            dialect,
            inner: OmlxGateway::with_config(OmlxConfig {
                host: host.clone(),
                api_key: api_key.clone(),
                timeout: Some(TIMEOUT),
            }),
            client: reqwest::Client::new(),
            host,
            api_key,
            non_streaming_timeout: None,
        }
    }

    /// A gateway configured from the dialect's host and key variables.
    pub(crate) fn from_env(dialect: Dialect) -> Self {
        let (host, api_key) = endpoint(
            dialect,
            std::env::var(dialect.host_variable()).ok(),
            std::env::var(dialect.key_variable()).ok(),
        );
        Self::new(dialect, host, api_key)
    }

    /// This gateway for callers outside the worker: one attempt per request,
    /// and ten minutes for a request that does not stream.
    pub(crate) fn direct(mut self) -> Result<RecoveryGateway<Self>> {
        self.non_streaming_timeout = Some(TIMEOUT);
        RecoveryGateway::new(self, RecoveryPolicy::default())
    }

    fn refusal(&self) -> MojenticError {
        MojenticError::GatewayError(format!(
            "{} completions need Mojentic recovery to apply the request dialect",
            self.dialect.name()
        ))
    }

    async fn lmstudio_models(&self) -> Result<Value> {
        let url = format!("{}/api/v1/models", self.host.trim_end_matches('/'));
        let mut request = self.client.get(url).timeout(METADATA_TIMEOUT);
        if let Some(key) = &self.api_key {
            request = request.bearer_auth(key);
        }
        let response = request.send().await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(MojenticError::GatewayError(format!(
                "LM Studio API error: {status} - {body}"
            )));
        }
        Ok(response.json().await?)
    }
}

#[async_trait]
impl LlmGateway for OpenAiCompatibleGateway {
    fn stream_clock(&self) -> std::sync::Arc<dyn mojentic::llm::stream_timing::StreamClock> {
        self.inner.stream_clock()
    }

    async fn get_model_context_window(&self, model: &str) -> Result<Option<ModelContextWindow>> {
        match self.dialect {
            // vLLM reports `max_model_len` in `/v1/models`, as oMLX does.
            Dialect::Vllm => self.inner.get_model_context_window(model).await,
            // LM Studio's `/v1/models` has no window; its native API does.
            Dialect::Lmstudio => Ok(lmstudio_context_window(
                &self.lmstudio_models().await?,
                model,
            )),
        }
    }

    async fn complete(
        &self,
        _model: &str,
        _messages: &[LlmMessage],
        _tools: Option<&[Box<dyn LlmTool>]>,
        _config: &CompletionConfig,
    ) -> Result<LlmGatewayResponse> {
        Err(self.refusal())
    }

    async fn complete_json(
        &self,
        _model: &str,
        _messages: &[LlmMessage],
        _schema: Value,
        _config: &CompletionConfig,
    ) -> Result<Value> {
        Err(self.refusal())
    }

    async fn get_available_models(&self) -> Result<Vec<String>> {
        self.inner.get_available_models().await
    }

    async fn calculate_embeddings(&self, text: &str, model: Option<&str>) -> Result<Vec<f32>> {
        self.inner.calculate_embeddings(text, model).await
    }

    fn complete_stream<'a>(
        &'a self,
        _model: &'a str,
        _messages: &'a [LlmMessage],
        _tools: Option<&'a [Box<dyn LlmTool>]>,
        _config: &'a CompletionConfig,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamChunk>> + Send + 'a>> {
        Box::pin(futures::stream::once(async { Err(self.refusal()) }))
    }

    fn complete_stream_events<'a>(
        &'a self,
        _model: &'a str,
        _messages: &'a [LlmMessage],
        _config: &'a CompletionConfig,
    ) -> std::result::Result<StreamEventStream<'a>, StreamEventError> {
        Err(StreamEventError::RequestFailed(self.refusal()))
    }
}

impl RecoveryProvider for OpenAiCompatibleGateway {
    /// Both servers answer in the form Mojentic reads for oMLX.
    fn recovery_provider(&self) -> mojentic::llm::recovery::Provider {
        mojentic::llm::recovery::Provider::Omlx
    }

    fn prepare_recovery(
        &self,
        model: &str,
        messages: &[LlmMessage],
        tools: Option<&[Box<dyn LlmTool>]>,
        config: &CompletionConfig,
        schema: Option<Value>,
        stream: bool,
    ) -> Result<reqwest::Request> {
        let mut request = self
            .inner
            .prepare_recovery(model, messages, tools, config, schema, stream)?;
        let body = request
            .body()
            .and_then(reqwest::Body::as_bytes)
            .ok_or_else(|| MojenticError::GatewayError("chat request has no body".into()))?;
        let body = rewrite_body(self.dialect, serde_json::from_slice(body)?);
        // The oMLX gateway sets no Content-Length; the client derives it from
        // the body when it sends the request.
        request
            .headers_mut()
            .remove(reqwest::header::CONTENT_LENGTH);
        *request.body_mut() = Some(serde_json::to_vec(&body)?.into());
        if !stream {
            *request.timeout_mut() = self.non_streaming_timeout;
        }
        Ok(request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIALECTS: [Dialect; 2] = [Dialect::Vllm, Dialect::Lmstudio];

    /// A body as the oMLX gateway writes it, with fields no dialect may touch.
    fn omlx_body(stream: bool) -> Value {
        json!({
            "model": "m",
            "messages": [
                {"role": "user", "content": "Hi"},
                {"role": "assistant", "content": "Hello", "reasoning_content": "kept"}
            ],
            "temperature": 1.0,
            "max_tokens": 8192,
            "top_p": 0.9,
            "response_format": {"type": "json_object"},
            "tools": [{"type": "function", "function": {"name": "list_tree"}}],
            "stream": stream
        })
    }

    fn with(mut body: Value, fields: Value) -> Value {
        for (key, value) in fields.as_object().unwrap() {
            body[key] = value.clone();
        }
        body
    }

    #[test]
    fn a_body_without_reasoning_controls_is_unchanged_unless_it_streams() {
        for dialect in DIALECTS {
            assert_eq!(rewrite_body(dialect, omlx_body(false)), omlx_body(false));
            assert_eq!(
                rewrite_body(dialect, omlx_body(true)),
                with(
                    omlx_body(true),
                    json!({"stream_options": {"include_usage": true}})
                )
            );
        }
    }

    #[test]
    fn vllm_reads_the_thinking_switch_from_chat_template_kwargs() {
        for enabled in [false, true] {
            assert_eq!(
                rewrite_body(
                    Dialect::Vllm,
                    with(omlx_body(false), json!({"enable_thinking": enabled}))
                ),
                with(
                    omlx_body(false),
                    json!({"chat_template_kwargs": {"enable_thinking": enabled}})
                )
            );
            assert_eq!(
                rewrite_body(
                    Dialect::Vllm,
                    with(
                        omlx_body(false),
                        json!({
                            "enable_thinking": enabled,
                            "chat_template_kwargs": {"preserve_thinking": true}
                        })
                    )
                ),
                with(
                    omlx_body(false),
                    json!({"chat_template_kwargs": {
                        "preserve_thinking": true, "enable_thinking": enabled
                    }})
                )
            );
        }
    }

    #[test]
    fn vllm_keeps_named_efforts_and_preserved_thinking() {
        for effort in ["low", "medium", "high", "xhigh"] {
            let body = with(
                omlx_body(false),
                json!({
                    "reasoning_effort": effort,
                    "chat_template_kwargs": {"preserve_thinking": true}
                }),
            );
            assert_eq!(rewrite_body(Dialect::Vllm, body.clone()), body);
        }
    }

    #[test]
    fn lmstudio_turns_thinking_off_with_the_none_effort() {
        assert_eq!(
            rewrite_body(
                Dialect::Lmstudio,
                with(omlx_body(false), json!({"enable_thinking": false}))
            ),
            with(omlx_body(false), json!({"reasoning_effort": "none"}))
        );
    }

    #[test]
    fn lmstudio_leaves_enabled_thinking_to_the_model_default() {
        assert_eq!(
            rewrite_body(
                Dialect::Lmstudio,
                with(omlx_body(false), json!({"enable_thinking": true}))
            ),
            omlx_body(false)
        );
    }

    #[test]
    fn lmstudio_keeps_named_efforts_and_drops_chat_template_kwargs() {
        for effort in ["low", "medium", "high", "xhigh"] {
            assert_eq!(
                rewrite_body(
                    Dialect::Lmstudio,
                    with(
                        omlx_body(true),
                        json!({
                            "reasoning_effort": effort,
                            "chat_template_kwargs": {"preserve_thinking": true}
                        })
                    )
                ),
                with(
                    omlx_body(true),
                    json!({
                        "reasoning_effort": effort,
                        "stream_options": {"include_usage": true}
                    })
                )
            );
        }
    }

    #[test]
    fn trace_controls_are_the_rewritten_request_controls() {
        let config = |effort, preserve_reasoning| CompletionConfig {
            max_tokens: Some(8192),
            reasoning_effort: effort,
            preserve_reasoning,
            ..Default::default()
        };
        let usage = json!({"include_usage": true});
        let source = "server_model_configuration";
        assert_eq!(
            provider_controls(Dialect::Vllm, &config(None, false)),
            json!({"max_tokens": 8192, "stream_options": usage, "context_window_source": source})
        );
        assert_eq!(
            provider_controls(
                Dialect::Vllm,
                &config(Some(ReasoningEffort::Disabled), true)
            ),
            json!({
                "max_tokens": 8192,
                "chat_template_kwargs": {"preserve_thinking": true, "enable_thinking": false},
                "stream_options": usage, "context_window_source": source
            })
        );
        assert_eq!(
            provider_controls(Dialect::Vllm, &config(Some(ReasoningEffort::XHigh), false)),
            json!({
                "max_tokens": 8192, "reasoning_effort": "xhigh",
                "stream_options": usage, "context_window_source": source
            })
        );
        assert_eq!(
            provider_controls(
                Dialect::Lmstudio,
                &config(Some(ReasoningEffort::Disabled), true)
            ),
            json!({
                "max_tokens": 8192, "reasoning_effort": "none",
                "stream_options": usage, "context_window_source": source
            })
        );
        assert_eq!(
            provider_controls(
                Dialect::Lmstudio,
                &config(Some(ReasoningEffort::Enabled), false)
            ),
            json!({"max_tokens": 8192, "stream_options": usage, "context_window_source": source})
        );
        assert_eq!(
            provider_controls(
                Dialect::Lmstudio,
                &CompletionConfig {
                    max_tokens: None,
                    reasoning_effort: Some(ReasoningEffort::Low),
                    ..Default::default()
                }
            ),
            json!({
                "reasoning_effort": "low",
                "stream_options": usage, "context_window_source": source
            })
        );
    }

    fn prepared(gateway: &OpenAiCompatibleGateway, stream: bool) -> reqwest::Request {
        let config = CompletionConfig {
            temperature: Some(1.0),
            max_tokens: Some(128),
            reasoning_effort: Some(ReasoningEffort::Disabled),
            preserve_reasoning: true,
            ..Default::default()
        };
        gateway
            .prepare_recovery("m", &[LlmMessage::user("Hi")], None, &config, None, stream)
            .unwrap()
    }

    fn body(request: &reqwest::Request) -> Value {
        serde_json::from_slice(request.body().unwrap().as_bytes().unwrap()).unwrap()
    }

    #[test]
    fn the_prepared_request_carries_the_dialect_body_to_the_chat_endpoint() {
        for (dialect, controls) in [
            (
                Dialect::Vllm,
                json!({"chat_template_kwargs": {"preserve_thinking": true, "enable_thinking": false}}),
            ),
            (Dialect::Lmstudio, json!({"reasoning_effort": "none"})),
        ] {
            let gateway = OpenAiCompatibleGateway::new(
                dialect,
                "http://server.test:9000/".into(),
                Some("sentinel-key".into()),
            );
            let request = prepared(&gateway, true);
            assert_eq!(request.method(), reqwest::Method::POST);
            assert_eq!(
                request.url().as_str(),
                "http://server.test:9000/v1/chat/completions"
            );
            assert_eq!(
                request.headers()[reqwest::header::AUTHORIZATION],
                "Bearer sentinel-key"
            );
            assert_eq!(
                request.headers()[reqwest::header::CONTENT_TYPE],
                "application/json"
            );
            assert!(
                !request
                    .headers()
                    .contains_key(reqwest::header::CONTENT_LENGTH)
            );
            assert_eq!(
                body(&request),
                with(
                    json!({
                        "model": "m",
                        "messages": [{"role": "user", "content": "Hi"}],
                        "temperature": 1.0,
                        "max_tokens": 128,
                        "stream": true,
                        "stream_options": {"include_usage": true}
                    }),
                    controls
                )
            );
            assert_eq!(body(&prepared(&gateway, false))["stream"], false);
            assert!(
                body(&prepared(&gateway, false))
                    .get("stream_options")
                    .is_none()
            );
        }
    }

    #[test]
    fn no_authorization_header_is_sent_without_a_key() {
        for dialect in DIALECTS {
            let gateway = OpenAiCompatibleGateway::new(dialect, "http://server.test".into(), None);
            assert!(
                !prepared(&gateway, true)
                    .headers()
                    .contains_key(reqwest::header::AUTHORIZATION)
            );
        }
    }

    #[test]
    fn only_a_direct_request_that_does_not_stream_has_a_timeout() {
        for dialect in DIALECTS {
            let worker = OpenAiCompatibleGateway::new(dialect, "http://server.test".into(), None);
            assert_eq!(prepared(&worker, false).timeout(), None);
            assert_eq!(prepared(&worker, true).timeout(), None);
            let direct = OpenAiCompatibleGateway {
                non_streaming_timeout: Some(TIMEOUT),
                ..worker
            };
            assert_eq!(prepared(&direct, false).timeout(), Some(&TIMEOUT));
            assert_eq!(prepared(&direct, true).timeout(), None);
        }
    }

    #[tokio::test]
    async fn completions_outside_recovery_are_refused_before_any_request() {
        use futures::StreamExt;
        let gateway =
            OpenAiCompatibleGateway::new(Dialect::Vllm, "http://server.test".into(), None);
        let messages = [LlmMessage::user("Hi")];
        let config = CompletionConfig::default();
        let expected = "LLM gateway error: vLLM completions need Mojentic recovery to apply the request dialect";
        let error = gateway
            .complete("m", &messages, None, &config)
            .await
            .unwrap_err();
        assert_eq!(error.to_string(), expected);
        let error = gateway
            .complete_json("m", &messages, json!({}), &config)
            .await
            .unwrap_err();
        assert_eq!(error.to_string(), expected);
        let mut stream = gateway.complete_stream("m", &messages, None, &config);
        assert_eq!(
            stream.next().await.unwrap().unwrap_err().to_string(),
            expected
        );
        assert!(stream.next().await.is_none());
        assert!(matches!(
            gateway.complete_stream_events("m", &messages, &config),
            Err(StreamEventError::RequestFailed(_))
        ));
    }

    #[test]
    fn endpoints_fall_back_to_the_dialect_default_and_ignore_empty_values() {
        assert_eq!(
            endpoint(Dialect::Vllm, None, None),
            ("http://localhost:8000".into(), None)
        );
        assert_eq!(
            endpoint(Dialect::Lmstudio, Some(String::new()), Some(String::new())),
            ("http://localhost:1234".into(), None)
        );
        assert_eq!(
            endpoint(
                Dialect::Lmstudio,
                Some("http://studio.test:1234".into()),
                Some("token".into())
            ),
            ("http://studio.test:1234".into(), Some("token".into()))
        );
        assert_eq!(
            (
                Dialect::Vllm.host_variable(),
                Dialect::Vllm.key_variable(),
                Dialect::Lmstudio.host_variable(),
                Dialect::Lmstudio.key_variable()
            ),
            ("VLLM_HOST", "VLLM_API_KEY", "LMSTUDIO_HOST", "LM_API_TOKEN")
        );
    }

    fn window(tokens: usize, field: &str) -> Option<ModelContextWindow> {
        Some(ModelContextWindow {
            max_context_tokens: NonZeroUsize::new(tokens).unwrap(),
            field: field.into(),
            model_num_ctx: None,
        })
    }

    fn lmstudio_models() -> Value {
        json!({"models": [
            {"key": "other/model", "max_context_length": 4096, "loaded_instances": []},
            {
                "key": "qwen/qwen3.8-27b",
                "max_context_length": 262144,
                "loaded_instances": [
                    {"id": "qwen/qwen3.8-27b", "config": {"context_length": 32768}},
                    {"id": "qwen/qwen3.8-27b:2", "config": {"context_length": 65536}}
                ]
            },
            {"key": "not/loaded", "max_context_length": 131072, "loaded_instances": []},
            {"key": "no/limit", "max_context_length": null},
            {"key": "zero/limit", "max_context_length": 0}
        ]})
    }

    #[test]
    fn a_loaded_lmstudio_instance_sets_the_window() {
        assert_eq!(
            lmstudio_context_window(&lmstudio_models(), "qwen/qwen3.8-27b"),
            window(32768, "loaded_instances.config.context_length")
        );
    }

    #[test]
    fn an_lmstudio_instance_id_selects_that_instance() {
        assert_eq!(
            lmstudio_context_window(&lmstudio_models(), "qwen/qwen3.8-27b:2"),
            window(65536, "loaded_instances.config.context_length")
        );
    }

    #[test]
    fn an_lmstudio_model_that_is_not_loaded_reports_its_maximum() {
        assert_eq!(
            lmstudio_context_window(&lmstudio_models(), "not/loaded"),
            window(131072, "max_context_length")
        );
    }

    #[test]
    fn an_lmstudio_instance_without_a_length_falls_back_to_the_maximum() {
        let models = json!({"models": [{
            "key": "m", "max_context_length": 8192,
            "loaded_instances": [{"id": "m", "config": {}}]
        }]});
        assert_eq!(
            lmstudio_context_window(&models, "m"),
            window(8192, "max_context_length")
        );
    }

    #[test]
    fn lmstudio_reports_no_window_when_it_has_none() {
        for model in ["no/limit", "zero/limit", "unknown/model"] {
            assert_eq!(lmstudio_context_window(&lmstudio_models(), model), None);
        }
        assert_eq!(lmstudio_context_window(&json!({}), "m"), None);
        assert_eq!(lmstudio_context_window(&json!({"models": {}}), "m"), None);
    }
}
