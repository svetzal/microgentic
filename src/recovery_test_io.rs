//! Caller recovery tests. The double supplies Mojentic lifecycle-bearing gateway
//! output; these tests do not verify Mojentic's HTTP, retry or decoder behavior.
use crate::agent::{AgentRunConfig, run_agent};
use crate::recovery::{Evidence, RecoveryCompletion};
use async_trait::async_trait;
use futures::Stream;
use mojentic::llm::models::{LlmGatewayResponse, LlmToolCall};
use mojentic::llm::recovery::*;
use mojentic::llm::{CompletionConfig, LlmGateway, LlmMessage, LlmTool};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    path::PathBuf,
    pin::Pin,
    sync::{Arc, Mutex},
};

tokio::task_local! { pub(crate) static ACTIVE: Arc<RecoveryIo>; }
pub(crate) struct RecoveryIo {
    pub(crate) tool_root: PathBuf,
    turns: Mutex<VecDeque<Script>>,
    requests: Mutex<Vec<Value>>,
}
pub(crate) struct Gateway {
    io: Arc<RecoveryIo>,
    evidence: Arc<Evidence>,
    policy: RecoveryPolicy,
}
impl RecoveryIo {
    pub(crate) fn gateway(
        self: &Arc<Self>,
        evidence: Arc<Evidence>,
        settings: &crate::recovery::RecoverySettings,
    ) -> anyhow::Result<Gateway> {
        let policy = crate::recovery::worker_policy(settings, evidence.clone())?;
        policy.validate()?;
        Ok(Gateway {
            io: self.clone(),
            evidence,
            policy,
        })
    }
}
#[async_trait]
impl LlmGateway for Gateway {
    fn complete_stream<'a>(
        &'a self,
        _: &'a str,
        _: &'a [LlmMessage],
        _: Option<&'a [Box<dyn LlmTool>]>,
        _: &'a CompletionConfig,
    ) -> futures::stream::BoxStream<'a, mojentic::Result<mojentic::llm::gateway::StreamChunk>> {
        panic!("use lifecycle-bearing completion")
    }

    async fn complete(
        &self,
        _: &str,
        _: &[LlmMessage],
        _: Option<&[Box<dyn LlmTool>]>,
        _: &CompletionConfig,
    ) -> mojentic::Result<LlmGatewayResponse> {
        panic!("unexpected ordinary call")
    }
    async fn complete_json(
        &self,
        _: &str,
        _: &[LlmMessage],
        _: Value,
        _: &CompletionConfig,
    ) -> mojentic::Result<Value> {
        panic!("unexpected structured call")
    }
    async fn get_available_models(&self) -> mojentic::Result<Vec<String>> {
        panic!("unexpected discovery")
    }
    async fn calculate_embeddings(&self, _: &str, _: Option<&str>) -> mojentic::Result<Vec<f32>> {
        panic!("unexpected embeddings")
    }
}
fn identity() -> Identity {
    Identity {
        logical_request_id: "10000000-0000-4000-8000-000000000001".parse().unwrap(),
        attempt_id: "20000000-0000-4000-8000-000000000001".parse().unwrap(),
        wire_attempt: 1,
    }
}
fn lifecycle(transition: Transition) -> LifecycleEvent {
    LifecycleEvent {
        transition,
        identity: Some(identity()),
        phase: Some(Phase::Streaming),
        progress: Progress::default(),
        failure: None,
        delay: None,
    }
}
#[async_trait]
impl RecoveryCompletion for Gateway {
    async fn complete_with_recovery(
        &self,
        _: &str,
        _: &[LlmMessage],
        _: Option<&[Box<dyn LlmTool>]>,
        _: &CompletionConfig,
        _: &RecoveryCall,
    ) -> Result<Recovered<LlmGatewayResponse>, RecoveryError> {
        panic!("unexpected ordinary recovery")
    }
    async fn complete_json_with_recovery(
        &self,
        _: &str,
        _: &[LlmMessage],
        _: Value,
        _: &CompletionConfig,
        _: &RecoveryCall,
    ) -> Result<Recovered<LlmGatewayResponse<Value>>, RecoveryError> {
        panic!("unexpected structured recovery")
    }
    fn complete_stream_with_recovery<'a>(
        &'a self,
        model: &'a str,
        messages: &'a [LlmMessage],
        tools: Option<&'a [Box<dyn LlmTool>]>,
        config: &'a CompletionConfig,
        call: &'a RecoveryCall,
    ) -> Pin<Box<dyn Stream<Item = RecoveryStreamEvent> + Send + 'a>> {
        self.io.requests.lock().unwrap().push(json!({"model": model, "messages": messages,
            "tools": tools.map(|tools| tools.iter().map(|tool| tool.descriptor()).collect::<Vec<_>>()),
            "completion": format!("{config:?}")}));
        let script = self
            .io
            .turns
            .lock()
            .unwrap()
            .pop_front()
            .expect("unscripted gateway request");
        let guard = Terminal {
            evidence: self.evidence.clone(),
            events: script.on_drop,
        };
        Box::pin(futures::stream::unfold(
            (script.steps, guard),
            move |(mut steps, mut guard)| async move {
                loop {
                    match steps.pop_front()? {
                        Step::Lifecycle(event) => {
                            LifecycleObserver::observe(guard.evidence.as_ref(), &event)
                        }
                        Step::Wire {
                            identity,
                            request,
                            status,
                            body,
                            request_id,
                        } => {
                            let mut encoded = reqwest::Request::new(
                                reqwest::Method::POST,
                                "http://unused.invalid/completion".parse().unwrap(),
                            );
                            *encoded.body_mut() = Some(request.into());
                            let observed = (|| {
                                WireObserver::observe(
                                    guard.evidence.as_ref(),
                                    WireEvent::Request {
                                        identity: identity.clone(),
                                        request: &encoded,
                                    },
                                )?;
                                let mut headers = reqwest::header::HeaderMap::new();
                                if let Some(value) = request_id {
                                    headers.insert("x-request-id", value.parse().unwrap());
                                }
                                WireObserver::observe(
                                    guard.evidence.as_ref(),
                                    WireEvent::Headers {
                                        identity: identity.clone(),
                                        status,
                                        headers: &headers,
                                    },
                                )?;
                                for bytes in body {
                                    WireObserver::observe(
                                        guard.evidence.as_ref(),
                                        WireEvent::Body {
                                            identity: identity.clone(),
                                            bytes: &bytes,
                                        },
                                    )?;
                                }
                                Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
                            })();
                            if let Err(error) = observed {
                                let contaminated = error.to_string().contains("credential");
                                let error = failure(
                                    if contaminated {
                                        Outcome::Interrupted
                                    } else {
                                        Outcome::ObserverFailed
                                    },
                                    Some(status),
                                    Provider::Omlx,
                                );
                                let mut failed = lifecycle(Transition::AttemptFailed);
                                failed.failure = Some(*error.failure.clone());
                                LifecycleObserver::observe(guard.evidence.as_ref(), &failed);
                                LifecycleObserver::observe(
                                    guard.evidence.as_ref(),
                                    &lifecycle(Transition::Interrupted),
                                );
                                guard.events.clear();
                                steps.clear();
                                return Some((RecoveryStreamEvent::Failed(error), (steps, guard)));
                            }
                        }
                        Step::Output(event) => {
                            if matches!(
                                event,
                                RecoveryStreamEvent::Completed(_) | RecoveryStreamEvent::Failed(_)
                            ) {
                                guard.events.clear();
                            }
                            return Some((event, (steps, guard)));
                        }
                        Step::Admission { failure, decision } => {
                            let context = AdmissionContext {
                                previous_identity: failure.identity.clone().unwrap(),
                                failure: *failure,
                                next_attempt: 2,
                            };
                            assert_eq!(
                                self.policy
                                    .admission
                                    .as_ref()
                                    .expect("configured admission")
                                    .decide(context)
                                    .await,
                                decision
                            );
                        }
                        Step::CaptureUnavailable(path) => {
                            std::fs::remove_dir_all(&path).unwrap();
                            std::fs::write(path, "capture destination became a file").unwrap();
                        }
                        Step::Cancel => call.cancellation.cancel(),
                        Step::Pending => futures::future::pending().await,
                    }
                }
            },
        ))
    }
}
enum Step {
    Lifecycle(LifecycleEvent),
    Wire {
        identity: Identity,
        request: Vec<u8>,
        status: u16,
        body: Vec<Vec<u8>>,
        request_id: Option<String>,
    },
    Output(RecoveryStreamEvent),
    Pending,
    Cancel,
    CaptureUnavailable(PathBuf),
    Admission {
        failure: Box<Failure>,
        decision: AdmissionDecision,
    },
}
struct Script {
    steps: VecDeque<Step>,
    on_drop: Vec<LifecycleEvent>,
}
struct Terminal {
    evidence: Arc<Evidence>,
    events: Vec<LifecycleEvent>,
}
impl Drop for Terminal {
    fn drop(&mut self) {
        for event in &self.events {
            LifecycleObserver::observe(self.evidence.as_ref(), event);
        }
    }
}
impl Script {
    fn retag(mut self, identity: Identity) -> Self {
        for step in &mut self.steps {
            match step {
                Step::Lifecycle(event) => event.identity = Some(identity.clone()),
                Step::Wire {
                    identity: current, ..
                } => *current = identity.clone(),
                Step::Output(RecoveryStreamEvent::Completed(response)) => {
                    response.report.identity = identity.clone()
                }
                _ => {}
            }
        }
        for event in &mut self.on_drop {
            event.identity = Some(identity.clone());
        }
        self
    }
    fn turn(events: Vec<RecoveryStreamEvent>) -> Self {
        let mut steps = VecDeque::from([Step::Lifecycle(lifecycle(Transition::AttemptStarted))]);
        for event in events {
            if matches!(event, RecoveryStreamEvent::Completed(_)) {
                steps.push_back(Step::Lifecycle(lifecycle(Transition::AttemptSucceeded)));
            }
            steps.push_back(Step::Output(event));
        }
        Self {
            steps,
            on_drop: vec![
                lifecycle(Transition::AttemptFailed),
                lifecycle(Transition::Cancelled),
            ],
        }
    }
}
fn completed(tools: Vec<LlmToolCall>) -> RecoveryStreamEvent {
    RecoveryStreamEvent::Completed(Box::new(Recovered {
        value: LlmGatewayResponse {
            content: Some("DONE".into()),
            thinking: None,
            object: None,
            tool_calls: tools,
            evidence: Default::default(),
        },
        report: RecoveryReport {
            identity: identity(),
            progress: Progress::default(),
            history: vec![],
            stream_metrics: None,
        },
    }))
}
fn tool() -> LlmToolCall {
    LlmToolCall {
        id: Some("call_exact_recovery_tool_1".into()),
        name: "shell_command".into(),
        arguments: serde_json::from_value(json!({"command":"printf executed >> marker.txt"}))
            .unwrap(),
    }
}

fn config(root: &std::path::Path) -> AgentRunConfig {
    use crate::agent::*;
    use std::path::PathBuf;
    AgentRunConfig {
        recovery: Default::default(),
        provider: Provider::Omlx,
        experiment_dir: root.parent().unwrap().to_path_buf(),
        trace_dir: Some(root.parent().unwrap().join("traces")),
        goal_file: PathBuf::from("task.md"),
        contract_file: None,
        model: "qwen3.6:27b-coding-mxfp8".to_string(),
        max_iterations: 2,
        max_model_interactions: 75,
        context_window_tokens: Some(131_072),
        packet_type: "multi-file-patch".to_string(),
        expected_output_tokens: 4_096,
        max_tokens: None,
        reasoning_effort: None,
        reasoning_profile: None,
        preserve_reasoning: false,
        max_thinking_only_tokens: 4_096,
        repair_exit_thinking_tokens: 16_384,
        repair_handoff_policy: RepairHandoffPolicy::TextOnly,
        action_boundary_interrupt_tokens: 0,
        reasoning_checkpoint_tokens: 0,
        transcript_policy: TranscriptPolicy::SummarizedTranscript,
        initial_context_catalog_file: None,
        semantic_advisor_model: None,
        acceptance_ledger: false,
        conversational: false,
        context_window: None,
        standing_context_limit: None,
        guidance_fraction: None,
        calculator: false,
        project_guidance: false,
        user_guidance_file: None,
        start_task: crate::start_task::StartTaskMode::Off,
        agent: None,
    }
}
fn workspace() -> (tempfile::TempDir, Arc<RecoveryIo>) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("workspace");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(dir.path().join("task.md"), "Finish with DONE.\n").unwrap();
    let io = Arc::new(RecoveryIo {
        tool_root: root,
        turns: Mutex::new(VecDeque::new()),
        requests: Mutex::new(vec![]),
    });
    (dir, io)
}
#[tokio::test]
async fn public_run_rejects_fragmented_credentials_before_tool_execution() {
    let (_dir, io) = workspace();
    let secret = "credential-private-123456789";
    io.turns.lock().unwrap().push_back(Script::turn(vec![
        RecoveryStreamEvent::Content(secret[..12].into()),
        RecoveryStreamEvent::Reasoning("unrelated".into()),
        RecoveryStreamEvent::Content(secret[12..].into()),
        completed(vec![tool()]),
    ]));
    io.turns.lock().unwrap().push_back(
        Script::turn(vec![
            RecoveryStreamEvent::Content("DONE".into()),
            completed(vec![]),
        ])
        .retag(second_identity()),
    );
    let result = crate::recovery::TEST_CREDENTIALS
        .scope(
            vec![secret.into()],
            ACTIVE.scope(io.clone(), run_agent(config(&io.tool_root))),
        )
        .await;
    assert!(
        !io.tool_root.join("marker.txt").exists(),
        "contaminated stream executed a tool"
    );
    let error = result.expect_err("contamination must reject the caller's run");
    assert!(
        format!("{error:#}").contains("safe worker completion rejected"),
        "{error:#}"
    );
    let requests = io.requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0]["model"], "qwen3.6:27b-coding-mxfp8");
    assert_eq!(
        requests[0]["completion"],
        "CompletionConfig { temperature: Some(1.0), num_ctx: Some(131072), max_tokens: Some(16384), top_p: None, top_k: None, response_format: None, reasoning_effort: None, reasoning_profile: None, preserve_reasoning: false, max_tool_iterations: 75 }"
    );
}
fn events(dir: &std::path::Path) -> Vec<Value> {
    let trace = std::fs::read_dir(dir.join("traces"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    std::fs::read_to_string(trace)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
fn rendered(dir: &std::path::Path) -> Value {
    let output = dir.join("transcript.html");
    let summary = crate::transcript::render_transcript(crate::transcript::RenderTranscriptConfig {
        inputs: vec![dir.join("traces")],
        output: output.clone(),
        title: "Recovery caller evidence".into(),
        evidence_file: None,
    })
    .unwrap();
    assert_eq!(summary.session_count, 1);
    let html = std::fs::read_to_string(output).unwrap();
    assert!(html.contains("Request recovery"));
    let data = html
        .split_once("<script id=\"transcriptData\" type=\"application/json\">")
        .unwrap()
        .1
        .split_once("</script>")
        .unwrap()
        .0;
    serde_json::from_str(data).unwrap()
}
fn payloads<'a>(events: &'a [Value], kind: &str) -> Vec<&'a Value> {
    events
        .iter()
        .filter(|e| e["kind"] == kind)
        .map(|e| &e["payload"])
        .collect()
}
fn wire() -> Step {
    Step::Wire {
        identity: identity(),
        request: b"{\"model\":\"qwen\",\"stream\":true}".to_vec(),
        status: 200,
        body: vec![b"first\n".to_vec(), "second café\n".as_bytes().to_vec()],
        request_id: None,
    }
}
#[tokio::test]
async fn public_run_retains_complete_lifecycle_identity_and_exact_capture_without_optional_metrics()
{
    let (dir, io) = workspace();
    let mut script = Script::turn(vec![
        RecoveryStreamEvent::Reasoning("reasoning intact".into()),
        RecoveryStreamEvent::Content("DONE".into()),
        completed(vec![]),
    ]);
    script.steps.insert(1, wire());
    io.turns.lock().unwrap().push_back(script);
    let mut config = config(&io.tool_root);
    let capture = dir.path().join("capture");
    config.recovery.recovery_capture_dir = Some(capture.clone());
    let result = ACTIVE.scope(io.clone(), run_agent(config)).await.unwrap();
    assert_eq!(result.final_summary, "DONE");
    let trace = events(dir.path());
    assert_eq!(
        payloads(&trace, "llm.recovery.lifecycle"),
        [
            serde_json::to_value(lifecycle(Transition::AttemptStarted)).unwrap(),
            serde_json::to_value(lifecycle(Transition::AttemptSucceeded)).unwrap()
        ]
        .iter()
        .collect::<Vec<_>>()
    );
    assert_eq!(
        payloads(&trace, "llm.recovery.accounting"),
        [&json!({"wire_attempts":1})]
    );
    assert!(
        payloads(&trace, "llm.stream.metrics").is_empty(),
        "unsupported metrics must remain absent"
    );
    let key = identity().attempt_id.to_string();
    assert_eq!(
        std::fs::read(capture.join(format!("{key}.request.body"))).unwrap(),
        b"{\"model\":\"qwen\",\"stream\":true}"
    );
    assert_eq!(
        std::fs::read(capture.join(format!("{key}.response.body"))).unwrap(),
        "first\nsecond café\n".as_bytes()
    );
    assert_eq!(
        serde_json::from_slice::<Value>(
            &std::fs::read(capture.join(format!("{key}.json"))).unwrap()
        )
        .unwrap(),
        json!({
            "attempt_id":key, "request_sha256":"6ccbc6ad9607d93963e3a10728424b467fa70f8b79c3d0bc5e59b3d081a22ae3", "http_status":200,
            "credential_redacted":false, "exact":true, "headers_captured":false, "identifying_metadata_redacted":false
        })
    );
    assert!(!capture.join(format!("{key}.pending.json")).exists());
}
#[tokio::test]
async fn public_run_capture_failure_rejects_completed_tools() {
    let (dir, io) = workspace();
    let capture = dir.path().join("capture");
    let mut script = Script::turn(vec![completed(vec![tool()])]);
    script.steps.insert(1, wire());
    script
        .steps
        .insert(2, Step::CaptureUnavailable(capture.clone()));
    io.turns.lock().unwrap().push_back(script);
    let mut config = config(&io.tool_root);
    config.recovery.recovery_capture_dir = Some(capture);
    let error = ACTIVE
        .scope(io.clone(), run_agent(config))
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("safe worker completion rejected"));
    assert!(!io.tool_root.join("marker.txt").exists());
    let trace = events(dir.path());
    assert_eq!(
        payloads(&trace, "llm.recovery.accounting"),
        [&json!({"wire_attempts":1})]
    );
    assert_eq!(
        payloads(&trace, "llm.recovery.final_failure")[0]["outcome"],
        "observer_failed"
    );
}
#[tokio::test]
async fn public_run_settlement_uses_exactly_five_seconds_of_injected_time() {
    let (dir, io) = workspace();
    io.turns.lock().unwrap().push_back(Script {
        steps: VecDeque::from([
            Step::Lifecycle(lifecycle(Transition::AttemptStarted)),
            Step::Output(RecoveryStreamEvent::Content("DONE".into())),
        ]),
        on_drop: vec![],
    });
    let clock = Arc::new(crate::test_clock::Clock::default());
    let finished = std::sync::atomic::AtomicBool::new(false);
    crate::test_clock::ACTIVE
        .scope(clock.clone(), async {
            let run = async {
                let result = ACTIVE
                    .scope(io.clone(), run_agent(config(&io.tool_root)))
                    .await;
                finished.store(true, std::sync::atomic::Ordering::SeqCst);
                result
            };
            let time = async {
                clock.wait_for(std::time::Duration::from_secs(5)).await;
                assert_eq!(clock.requested(), [std::time::Duration::from_secs(5)]);
                clock.advance(std::time::Duration::from_millis(4999));
                tokio::task::yield_now().await;
                assert!(!finished.load(std::sync::atomic::Ordering::SeqCst));
                clock.advance(std::time::Duration::from_millis(1));
            };
            let (result, ()) = tokio::join!(run, time);
            assert_eq!(
                result.unwrap_err().to_string(),
                "request cancellation did not retain terminal lifecycle evidence"
            );
        })
        .await;
    assert_eq!(
        payloads(&events(dir.path()), "llm.recovery.final_failure"),
        [&json!({"outcome":"terminal_evidence_drain_failed"})]
    );
}
#[tokio::test]
async fn public_run_dropped_reasoning_stream_retains_ordered_cancellation_and_capture() {
    let (dir, io) = workspace();
    let mut script = Script::turn(vec![RecoveryStreamEvent::Reasoning(
        "reasoning beyond the caller's cap".into(),
    )]);
    script.steps.insert(1, wire());
    script.steps.push_back(Step::Pending);
    for event in &mut script.on_drop {
        event.progress.headers_received = true;
        event.progress.raw_bytes = "first\nsecond café\n".len() as u64;
        event.progress.observed.reasoning_bytes = "reasoning beyond the caller's cap".len() as u64;
        event.progress.delivered.reasoning_bytes = "reasoning beyond the caller's cap".len() as u64;
    }
    io.turns.lock().unwrap().extend([
        script,
        Script::turn(vec![
            RecoveryStreamEvent::Content("DONE".into()),
            completed(vec![]),
        ])
        .retag(second_identity()),
    ]);
    let mut config = config(&io.tool_root);
    config.max_thinking_only_tokens = 1;
    config.recovery.recovery_capture_dir = Some(dir.path().join("capture"));
    let result = ACTIVE.scope(io.clone(), run_agent(config)).await.unwrap();
    assert_eq!(result.final_summary, "DONE");
    let trace = events(dir.path());
    assert_eq!(
        payloads(&trace, "llm.recovery.lifecycle"),
        [
            Transition::AttemptStarted,
            Transition::AttemptFailed,
            Transition::Cancelled,
            Transition::AttemptStarted,
            Transition::AttemptSucceeded
        ]
        .into_iter()
        .enumerate()
        .map(|(index, t)| {
            let mut event = lifecycle(t);
            if index >= 3 {
                event.identity = Some(second_identity());
            } else if index > 0 {
                event.progress.headers_received = true;
                event.progress.raw_bytes = "first\nsecond café\n".len() as u64;
                event.progress.observed.reasoning_bytes =
                    "reasoning beyond the caller's cap".len() as u64;
                event.progress.delivered.reasoning_bytes =
                    "reasoning beyond the caller's cap".len() as u64;
            }
            serde_json::to_value(event).unwrap()
        })
        .collect::<Vec<_>>()
        .iter()
        .collect::<Vec<_>>()
    );
    assert_eq!(
        payloads(&trace, "llm.recovery.accounting"),
        [&json!({"wire_attempts":2})]
    );
    assert!(
        trace
            .iter()
            .any(|event| event["kind"] == "llm.thinking_only_stream.action_transitioned")
    );
    assert_eq!(
        std::fs::read(
            dir.path()
                .join("capture")
                .join(format!("{}.response.body", identity().attempt_id))
        )
        .unwrap(),
        "first\nsecond café\n".as_bytes()
    );
}

// Failure has private cause/body fields and no public constructor. Obtain an
// empty typed error by rejecting request preparation before any HTTP operation,
// then specify its public safe metadata. This is fixture construction only.
impl RecoveryProvider for Gateway {
    fn recovery_provider(&self) -> Provider {
        Provider::Omlx
    }
    fn prepare_recovery(
        &self,
        _: &str,
        _: &[LlmMessage],
        _: Option<&[Box<dyn LlmTool>]>,
        _: &CompletionConfig,
        _: Option<Value>,
        _: bool,
    ) -> mojentic::Result<reqwest::Request> {
        Err(mojentic::error::MojenticError::GatewayError(
            "fixture encoding rejected before transport".into(),
        ))
    }
}
fn failure(outcome: Outcome, status: Option<u16>, provider: Provider) -> RecoveryError {
    let io = Arc::new(RecoveryIo {
        tool_root: PathBuf::new(),
        turns: Mutex::new(VecDeque::new()),
        requests: Mutex::new(vec![]),
    });
    let settings = crate::recovery::RecoverySettings::default();
    let gateway = io
        .gateway(crate::recovery::evidence(&settings), &settings)
        .unwrap();
    let gateway = RecoveryGateway::new(gateway, RecoveryPolicy::default()).unwrap();
    let mut error = futures::executor::block_on(gateway.complete_with_recovery(
        "fixture",
        &[],
        None,
        &CompletionConfig::default(),
        &RecoveryCall::default(),
    ))
    .err()
    .unwrap();
    error.outcome = outcome;
    error.failure.provider = provider;
    error.failure.operation = Operation::Streaming;
    error.failure.category = Category::Http;
    error.failure.http_status = status;
    error.failure.phase = Some(Phase::AwaitingHeaders);
    error.failure.acceptance = Acceptance::Unknown;
    error.failure.identity = Some(identity());
    error.failure.classification = Classification {
        eligible: true,
        reason: Reason::Transient,
    };
    error
}
fn expected_failure(provider: &str, status: Option<u16>) -> Value {
    json!({"provider":provider,"operation":"streaming","category":"http","http_status":status,
        "provider_code":null,"retry_after":"absent","provider_request_id":null,"phase":"awaiting_headers",
        "acceptance":"unknown","progress":{"headers_received":false,"raw_bytes":0,
        "observed":{"reasoning_bytes":0,"content_bytes":0,"tool_fragments":0,"completed_tool_calls":0},
        "delivered":{"reasoning_bytes":0,"content_bytes":0,"tool_fragments":0,"completed_tool_calls":0}},
        "classification":{"eligible":true,"reason":"transient"},"identity":identity(),"remote_termination_confirmed":false})
}
#[tokio::test]
async fn public_run_propagates_all_terminal_outcomes_with_full_failure_payloads_both_providers() {
    for (provider, native) in [
        (Provider::Omlx, crate::agent::Provider::Omlx),
        (Provider::Ollama, crate::agent::Provider::Ollama),
    ] {
        for (outcome, name) in [
            (Outcome::Failed, "failed"),
            (Outcome::Exhausted, "exhausted"),
            (Outcome::Interrupted, "interrupted"),
            (Outcome::Cancelled, "cancelled"),
            (Outcome::AdmissionRequired, "admission_required"),
            (Outcome::AdmissionRejected, "admission_rejected"),
            (Outcome::AdmissionUnresolved, "admission_unresolved"),
            (Outcome::DelayCeiling, "delay_ceiling"),
            (Outcome::BudgetExpired, "budget_expired"),
            (Outcome::ObserverFailed, "observer_failed"),
        ] {
            let (dir, io) = workspace();
            let error = failure(outcome, Some(504), provider);
            let expected = json!({"outcome":name,"failure":expected_failure(if native == crate::agent::Provider::Omlx {"omlx"} else {"ollama"},Some(504)),"history":[]});
            assert_eq!(serde_json::to_value(&error).unwrap(), expected);
            let mut failed = lifecycle(Transition::AttemptFailed);
            failed.failure = Some(*error.failure.clone());
            io.turns.lock().unwrap().push_back(Script {
                steps: VecDeque::from([
                    Step::Lifecycle(lifecycle(Transition::AttemptStarted)),
                    Step::Lifecycle(failed.clone()),
                    Step::Output(RecoveryStreamEvent::Failed(error)),
                ]),
                on_drop: vec![],
            });
            let mut config = config(&io.tool_root);
            config.provider = native;
            let error = ACTIVE
                .scope(io.clone(), run_agent(config))
                .await
                .unwrap_err();
            let typed = error
                .chain()
                .find_map(|cause| cause.downcast_ref::<mojentic::error::MojenticError>())
                .unwrap();
            let mojentic::error::MojenticError::Recovery(actual) = typed else {
                panic!("lost typed recovery error: {error:#}")
            };
            assert_eq!(serde_json::to_value(actual.as_ref()).unwrap(), expected);
            let trace = events(dir.path());
            assert_eq!(
                payloads(&trace, "llm.recovery.final_failure"),
                [&json!({"error":expected,"credential_redacted":false})]
            );
            assert_eq!(
                payloads(&trace, "llm.recovery.lifecycle"),
                [
                    &serde_json::to_value(lifecycle(Transition::AttemptStarted)).unwrap(),
                    &serde_json::to_value(failed).unwrap()
                ]
            );
            assert_eq!(
                payloads(&trace, "llm.recovery.accounting"),
                [&json!({"wire_attempts":1})]
            );
            assert!(!io.tool_root.join("marker.txt").exists());
        }
    }
}
#[tokio::test]
async fn public_run_tool_fragments_are_not_executed_and_completed_tool_executes_once() {
    let (dir, io) = workspace();
    let raw = json!({"index":0,"id":"call_exact_recovery_tool_1","function":{"name":"shell_command","arguments":"{\"command\":\"printf executed >> marker.txt\"}"}});
    io.turns.lock().unwrap().extend([
        Script::turn(vec![
            RecoveryStreamEvent::ToolFragment(raw.clone()),
            RecoveryStreamEvent::ToolFragment(raw),
            completed(vec![tool()]),
        ]),
        Script::turn(vec![
            RecoveryStreamEvent::Content("DONE".into()),
            completed(vec![]),
        ])
        .retag(second_identity()),
    ]);
    let result = ACTIVE
        .scope(io.clone(), run_agent(config(&io.tool_root)))
        .await
        .unwrap();
    assert_eq!(result.final_summary, "DONE");
    assert_eq!(
        std::fs::read(io.tool_root.join("marker.txt")).unwrap(),
        b"executed"
    );
    let requests = io.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    let messages = requests[1]["messages"].as_array().unwrap();
    let assistant = messages
        .iter()
        .find(|message| message["tool_calls"].is_array())
        .unwrap();
    assert_eq!(assistant["tool_calls"], json!([tool()]));
    assert_eq!(
        payloads(&events(dir.path()), "llm.recovery.accounting"),
        [&json!({"wire_attempts":2})]
    );
}
#[tokio::test]
async fn public_run_filters_credentials_in_every_semantic_and_tool_fragment_channel() {
    let secret = "abcdef";
    for variant in [
        "reasoning_interleaved",
        "content_interleaved",
        "tool_arguments",
        "tool_name",
        "tool_id",
        "tool_completed",
        "tool_escaped",
        "tool_mixed",
        "tool_key_boundary_truncated",
        "tool_key_boundary_completed",
        "tool_key_boundary_escape_tail",
    ] {
        let (dir, io) = workspace();
        let mut fragments = Vec::new();
        match variant {
            "reasoning_interleaved" => fragments.extend([
                RecoveryStreamEvent::Reasoning("abc".into()),
                RecoveryStreamEvent::Content("harmless".into()),
                RecoveryStreamEvent::Reasoning("def".into()),
            ]),
            "content_interleaved" => fragments.extend([
                RecoveryStreamEvent::Content("abc".into()),
                RecoveryStreamEvent::Reasoning("harmless".into()),
                RecoveryStreamEvent::Content("def".into()),
            ]),
            "tool_completed" => {}
            "tool_mixed" => fragments.extend([
                RecoveryStreamEvent::Reasoning("abc".into()),
                RecoveryStreamEvent::ToolFragment(
                    json!({"index":0,"function":{"arguments":{"command":"def"}}}),
                ),
            ]),
            "tool_key_boundary_truncated"
            | "tool_key_boundary_completed"
            | "tool_key_boundary_escape_tail" => {
                fragments.push(RecoveryStreamEvent::ToolFragment(
                    json!({"index":0,"function":{"arguments":"{\"command\""}}),
                ));
                fragments.push(RecoveryStreamEvent::ToolFragment(json!({"index":0,"function":{"arguments":if variant.ends_with("escape_tail") {r#": "\u0061\u0062\u0063\u0064\u0065\u0066\u00"#} else {r#": "\u0061\u0062\u0063\u0064\u0065\u0066"#}}})));
            }
            field => {
                for (n, piece) in ["abc", "def"].into_iter().enumerate() {
                    let mut fragment = json!({"index":0,"function":{}});
                    match field {
                        "tool_id" => fragment["id"] = json!(piece),
                        "tool_name" => fragment["function"]["name"] = json!(piece),
                        "tool_escaped" => {
                            fragment["function"]["arguments"] = json!(if n == 0 {
                                r#"{"command":"\u0061\u0062\u0063"#
                            } else {
                                r#"\u0064\u0065\u0066"}"#
                            })
                        }
                        _ => fragment["function"]["arguments"] = json!({"command":piece}),
                    }
                    fragments.push(RecoveryStreamEvent::ToolFragment(fragment));
                    if n == 0 {
                        fragments.push(RecoveryStreamEvent::Content("harmless".into()));
                    }
                }
            }
        }
        let mut call = tool();
        if variant == "tool_completed" {
            call.arguments.insert("command".into(), json!(secret));
        }
        fragments.push(completed(vec![call]));
        io.turns.lock().unwrap().push_back(Script::turn(fragments));
        let error = crate::recovery::TEST_CREDENTIALS
            .scope(
                vec![secret.into()],
                ACTIVE.scope(io.clone(), run_agent(config(&io.tool_root))),
            )
            .await
            .unwrap_err();
        assert!(
            format!("{error:#}").contains("safe worker completion rejected"),
            "{variant}: {error:#}"
        );
        assert!(!io.tool_root.join("marker.txt").exists(), "{variant}");
        let trace = events(dir.path());
        assert_eq!(
            payloads(&trace, "llm.recovery.accounting"),
            [&json!({"wire_attempts":1})],
            "{variant}"
        );
        assert_eq!(
            payloads(&trace, "llm.recovery.final_failure")[0]["credential_redacted"],
            true,
            "{variant}"
        );
        let serialized = serde_json::to_string(&trace).unwrap();
        assert!(
            !serialized.contains(secret),
            "{variant}: credential in persisted trace"
        );
        let document = rendered(dir.path());
        let calls = document["sessions"][0]["calls"].as_array().unwrap();
        let semantic = calls
            .iter()
            .map(|call| {
                format!(
                    "{}{}",
                    call["reasoning"].as_str().unwrap(),
                    call["assistant"].as_str().unwrap()
                )
            })
            .collect::<String>();
        assert!(
            !semantic.contains(secret),
            "{variant}: reconstructed rendered credential"
        );
        assert_eq!(
            io.requests.lock().unwrap().len(),
            1,
            "{variant}: contaminated request retried"
        );
    }
}
#[cfg(unix)]
#[tokio::test]
async fn public_run_admission_binds_full_failure_digest_and_attempt_identity() {
    use std::os::unix::fs::PermissionsExt;
    for variant in [
        "bound_allow",
        "wrong_digest",
        "plain_allow",
        "reject",
        "malformed",
        "failed_helper",
        "credential_header",
    ] {
        let (dir, io) = workspace();
        let input = dir.path().join("admission-input.json");
        let command = dir.path().join("admit");
        let digest = "6ccbc6ad9607d93963e3a10728424b467fa70f8b79c3d0bc5e59b3d081a22ae3";
        let mut proof = json!({"decision":"allow","previous_attempt_id":identity().attempt_id,
            "logical_request_id":identity().logical_request_id,"request_sha256":digest,
            "provider_request_id":null,"evidence_kind":"not_forwarded"});
        match variant {
            "wrong_digest" => proof["request_sha256"] = json!("0".repeat(64)),
            "plain_allow" => proof = json!({"decision":"allow"}),
            "reject" => proof["decision"] = json!("reject"),
            _ => {}
        }
        let response = if variant == "malformed" {
            "invalid JSON".into()
        } else {
            proof.to_string()
        };
        std::fs::write(
            &command,
            format!(
                "#!/bin/sh\ncat > '{}'\nprintf '%s' '{}'\nexit {}\n",
                input.display(),
                response,
                if variant == "failed_helper" { 1 } else { 0 }
            ),
        )
        .unwrap();
        std::fs::set_permissions(&command, std::fs::Permissions::from_mode(0o700)).unwrap();
        let allowed = variant == "bound_allow";
        let error = failure(Outcome::AdmissionRejected, Some(503), Provider::Omlx);
        let mut script = Script::turn(if allowed {
            vec![
                RecoveryStreamEvent::Content("DONE".into()),
                completed(vec![]),
            ]
        } else {
            vec![RecoveryStreamEvent::Failed(error.clone())]
        });
        let mut captured = wire();
        if variant == "credential_header"
            && let Step::Wire { request_id, .. } = &mut captured
        {
            *request_id = Some("private-header-token".into());
        }
        script.steps.insert(1, captured);
        script
            .steps
            .insert(2, Step::Lifecycle(lifecycle(Transition::AdmissionPending)));
        script.steps.insert(
            3,
            Step::Admission {
                failure: error.failure.clone(),
                decision: if allowed {
                    AdmissionDecision::Allow
                } else {
                    AdmissionDecision::Reject
                },
            },
        );
        io.turns.lock().unwrap().push_back(script);
        let mut config = config(&io.tool_root);
        config.recovery.recover_requests = true;
        config.recovery.recovery_max_attempts = 2;
        config.recovery.recovery_admission_command = Some(command);
        let result = crate::recovery::TEST_CREDENTIALS
            .scope(
                vec!["private-header-token".into()],
                ACTIVE.scope(io.clone(), run_agent(config)),
            )
            .await;
        assert_eq!(result.is_ok(), allowed, "{variant}: {result:?}");
        if variant == "credential_header" {
            assert!(
                !input.exists(),
                "redacted identifying metadata must not invoke admission"
            );
            assert_eq!(
                payloads(&events(dir.path()), "llm.recovery.lifecycle")[1]["identifying_metadata_redacted"],
                true
            );
            continue;
        }
        let actual: Value = serde_json::from_slice(&std::fs::read(input).unwrap()).unwrap();
        assert_eq!(
            actual,
            json!({"failure":expected_failure("omlx",Some(503)),"request_sha256":digest}),
            "{variant}"
        );
        assert_eq!(io.requests.lock().unwrap().len(), 1);
        assert_eq!(
            payloads(&events(dir.path()), "run.started")[0]["recovery"]["recovery_max_attempts"],
            2
        );
    }
}
#[tokio::test]
async fn public_run_validates_every_recovery_setting_before_dispatch_or_trace_creation() {
    for scenario in [
        "disabled_tuning",
        "zero_attempts",
        "zero_budget",
        "delay_order",
        "tool_admission",
        "tool_capture",
        "parent_capture",
    ] {
        let (dir, io) = workspace();
        let mut config = config(&io.tool_root);
        let settings = &mut config.recovery;
        let expected = match scenario {
            "disabled_tuning" => {
                settings.recovery_max_attempts = 2;
                "recovery tuning requires --recover-requests; one-attempt capture is allowed independently"
            }
            "zero_attempts" => {
                settings.recover_requests = true;
                settings.recovery_max_attempts = 0;
                "recovery-max-attempts must be positive"
            }
            "zero_budget" => {
                settings.recover_requests = true;
                settings.recovery_budget_ms = Some(0);
                "invalid recovery delay or budget"
            }
            "delay_order" => {
                settings.recover_requests = true;
                settings.recovery_base_delay_ms = 2;
                settings.recovery_delay_ceiling_ms = 1;
                "invalid recovery delay or budget"
            }
            "tool_admission" => {
                let command = io.tool_root.join("untrusted-helper");
                std::fs::write(&command, "untrusted").unwrap();
                settings.recover_requests = true;
                settings.recovery_admission_command = Some(command);
                "admission executable must be outside the model tool root"
            }
            "tool_capture" => {
                settings.recovery_capture_dir = Some(io.tool_root.join("capture"));
                "capture directory must be outside the model tool root"
            }
            _ => {
                settings.recovery_capture_dir = Some(dir.path().join("new/../workspace/capture"));
                "capture directory must be outside the model tool root"
            }
        };
        let error = ACTIVE
            .scope(io.clone(), run_agent(config))
            .await
            .unwrap_err();
        assert_eq!(error.to_string(), expected, "{scenario}");
        assert!(io.requests.lock().unwrap().is_empty(), "{scenario}");
        assert!(!dir.path().join("traces").exists(), "{scenario}");
    }
}

fn second_identity() -> Identity {
    Identity {
        logical_request_id: "10000000-0000-4000-8000-000000000002".parse().unwrap(),
        attempt_id: "20000000-0000-4000-8000-000000000002".parse().unwrap(),
        wire_attempt: 1,
    }
}
#[tokio::test]
async fn public_run_wire_credentials_are_redacted_in_exactly_identified_captures() {
    for escaped in [false, true] {
        let (dir, io) = workspace();
        let capture = dir.path().join("capture");
        let response = if escaped {
            br#"{"content":"\u0061\u0062\u0063\u0064\u0065\u0066"}"#.to_vec()
        } else {
            b"abcdef".to_vec()
        };
        let mut script = Script::turn(vec![completed(vec![tool()])]);
        script.steps.insert(
            1,
            Step::Wire {
                identity: identity(),
                request: b"{}".to_vec(),
                status: 200,
                body: vec![
                    response[..response.len() / 2].to_vec(),
                    response[response.len() / 2..].to_vec(),
                ],
                request_id: None,
            },
        );
        io.turns.lock().unwrap().push_back(script);
        let mut config = config(&io.tool_root);
        config.recovery.recovery_capture_dir = Some(capture.clone());
        let error = crate::recovery::TEST_CREDENTIALS
            .scope(
                vec!["abcdef".into()],
                ACTIVE.scope(io.clone(), run_agent(config)),
            )
            .await
            .unwrap_err();
        assert!(format!("{error:#}").contains("Interrupted"));
        assert!(!io.tool_root.join("marker.txt").exists());
        let key = identity().attempt_id.to_string();
        assert_eq!(
            std::fs::read(capture.join(format!("{key}.request.body"))).unwrap(),
            b"{}"
        );
        assert_eq!(
            std::fs::read(capture.join(format!("{key}.response.body"))).unwrap(),
            b"[credential-contaminated response withheld]"
        );
        let metadata: Value =
            serde_json::from_slice(&std::fs::read(capture.join(format!("{key}.json"))).unwrap())
                .unwrap();
        assert_eq!(
            metadata,
            json!({"attempt_id":key,"request_sha256":"44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a","http_status":200,"credential_redacted":true,"exact":false,"headers_captured":false,"identifying_metadata_redacted":false})
        );
        assert_eq!(io.requests.lock().unwrap().len(), 1);
    }
}
#[tokio::test]
async fn public_run_safe_prefix_mismatch_and_empty_fragments_preserve_visible_bytes() {
    let (_dir, io) = workspace();
    let text = "abc mismatch defDONE";
    io.turns.lock().unwrap().push_back(Script::turn(vec![
        RecoveryStreamEvent::Content("abc".into()),
        RecoveryStreamEvent::Content("".into()),
        RecoveryStreamEvent::Content(" mismatch ".into()),
        RecoveryStreamEvent::Content("defDONE".into()),
        completed(vec![]),
    ]));
    let mut config = config(&io.tool_root);
    config.conversational = true;
    let result = crate::recovery::TEST_CREDENTIALS
        .scope(
            vec!["abcdef".into()],
            ACTIVE.scope(io.clone(), run_agent(config)),
        )
        .await
        .unwrap();
    assert_eq!(result.final_summary, text);
}
#[tokio::test]
async fn public_run_retains_retry_identities_ordered_lifecycle_metrics_and_both_capture_bodies() {
    use mojentic::llm::gateway::StreamMetrics;
    let (dir, io) = workspace();
    let mut retry = identity();
    retry.attempt_id = "20000000-0000-4000-8000-000000000002".parse().unwrap();
    retry.wire_attempt = 2;
    let error = failure(Outcome::Failed, Some(503), Provider::Ollama);
    let mut failed = lifecycle(Transition::AttemptFailed);
    failed.failure = Some(*error.failure.clone());
    let mut delay = lifecycle(Transition::DelayScheduled);
    delay.delay = Some(std::time::Duration::from_millis(17));
    let mut restarted = lifecycle(Transition::RetryStarted);
    restarted.identity = Some(retry.clone());
    let mut started = lifecycle(Transition::AttemptStarted);
    started.identity = Some(retry.clone());
    let mut succeeded = lifecycle(Transition::AttemptSucceeded);
    succeeded.identity = Some(retry.clone());
    let expected = [
        lifecycle(Transition::AttemptStarted),
        failed,
        lifecycle(Transition::AdmissionPending),
        lifecycle(Transition::AdmissionAllowed),
        delay,
        restarted,
        started,
        succeeded,
    ];
    let mut steps = VecDeque::new();
    for (index, event) in expected.iter().enumerate() {
        steps.push_back(Step::Lifecycle(event.clone()));
        if index == 0 || index == 6 {
            let mut capture = wire();
            if let Step::Wire {
                identity, status, ..
            } = &mut capture
            {
                if index == 6 {
                    *identity = retry.clone();
                } else {
                    *status = 503;
                }
            }
            steps.push_back(capture);
        }
    }
    let metrics = StreamMetrics {
        provider: "ollama".into(),
        eval_count: Some(7),
        prompt_eval_count: Some(11),
        eval_duration_ns: Some(1_000_000_000),
        tokens_per_second: Some(7.0),
        ..Default::default()
    };
    steps.push_back(Step::Output(RecoveryStreamEvent::Metrics {
        identity: retry.clone(),
        metrics: metrics.clone(),
    }));
    steps.push_back(Step::Output(RecoveryStreamEvent::Content("DONE".into())));
    let mut terminal = completed(vec![]);
    if let RecoveryStreamEvent::Completed(response) = &mut terminal {
        response.report.identity = retry.clone();
        response.report.history = vec![*error.failure];
        response.report.stream_metrics = Some(Box::new(metrics));
    }
    steps.push_back(Step::Output(terminal));
    io.turns.lock().unwrap().push_back(Script {
        steps,
        on_drop: vec![],
    });
    let mut config = config(&io.tool_root);
    config.provider = crate::agent::Provider::Ollama;
    config.recovery.recover_requests = true;
    config.recovery.recovery_max_attempts = 2;
    config.recovery.recovery_capture_dir = Some(dir.path().join("capture"));
    let result = ACTIVE.scope(io.clone(), run_agent(config)).await.unwrap();
    assert_eq!(result.final_summary, "DONE");
    let trace = events(dir.path());
    let expected: Vec<Value> = expected
        .iter()
        .map(|event| serde_json::to_value(event).unwrap())
        .collect();
    assert_eq!(
        payloads(&trace, "llm.recovery.lifecycle"),
        expected.iter().collect::<Vec<_>>()
    );
    assert_eq!(
        payloads(&trace, "llm.recovery.accounting"),
        [&json!({"wire_attempts":2})]
    );
    let metrics = payloads(&trace, "llm.stream.metrics");
    assert_eq!(metrics.len(), 1, "raw metrics must not be forwarded twice");
    assert_eq!(metrics[0]["eval_count"], 7);
    assert_eq!(metrics[0]["prompt_eval_count"], 11);
    assert_eq!(metrics[0]["eval_duration_ns"], 1_000_000_000u64);
    assert_eq!(metrics[0]["tokens_per_second"], 7.0);
    for id in [identity(), retry] {
        let key = id.attempt_id;
        assert_eq!(
            std::fs::read(dir.path().join(format!("capture/{key}.request.body"))).unwrap(),
            b"{\"model\":\"qwen\",\"stream\":true}"
        );
        assert_eq!(
            std::fs::read(dir.path().join(format!("capture/{key}.response.body"))).unwrap(),
            "first\nsecond café\n".as_bytes()
        );
    }
}
#[tokio::test]
async fn public_run_failure_without_identity_uses_history_for_settlement_and_retains_absent_evidence()
 {
    let (dir, io) = workspace();
    let mut error = failure(Outcome::Interrupted, None, Provider::Omlx);
    error.history.push(*error.failure.clone());
    error.failure.identity = None;
    error.failure.phase = None;
    let expected = serde_json::to_value(&error).unwrap();
    io.turns.lock().unwrap().push_back(Script {
        steps: VecDeque::from([
            Step::Lifecycle(lifecycle(Transition::AttemptStarted)),
            Step::Output(RecoveryStreamEvent::Failed(error)),
        ]),
        on_drop: vec![],
    });
    let error = ACTIVE
        .scope(io.clone(), run_agent(config(&io.tool_root)))
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("Interrupted"));
    let trace = events(dir.path());
    assert_eq!(
        payloads(&trace, "llm.recovery.final_failure"),
        [&json!({"error":expected,"credential_redacted":false})]
    );
    assert_eq!(
        payloads(&trace, "llm.recovery.accounting"),
        [&json!({"wire_attempts":0})]
    );
    assert!(payloads(&trace, "llm.stream.metrics").is_empty());
}
#[tokio::test]
async fn public_run_preserves_permanent_status_and_partial_output_failure_evidence() {
    for status in [400, 401, 403] {
        let (dir, io) = workspace();
        let mut error = failure(Outcome::Failed, Some(status), Provider::Omlx);
        error.failure.classification = Classification {
            eligible: false,
            reason: Reason::Permanent,
        };
        error.failure.progress = Progress {
            headers_received: true,
            raw_bytes: 31,
            observed: SemanticProgress {
                content_bytes: 7,
                tool_fragments: 1,
                ..Default::default()
            },
            delivered: SemanticProgress {
                content_bytes: 7,
                ..Default::default()
            },
        };
        let mut expected = expected_failure("omlx", Some(status));
        expected["classification"] = json!({"eligible":false,"reason":"permanent"});
        expected["progress"] = json!({"headers_received":true,"raw_bytes":31,
            "observed":{"content_bytes":7,"reasoning_bytes":0,"tool_fragments":1,"completed_tool_calls":0},
            "delivered":{"content_bytes":7,"reasoning_bytes":0,"tool_fragments":0,"completed_tool_calls":0}});
        let mut failed = lifecycle(Transition::AttemptFailed);
        failed.progress = error.failure.progress.clone();
        failed.failure = Some(*error.failure.clone());
        io.turns.lock().unwrap().push_back(Script {
            steps: VecDeque::from([
                Step::Lifecycle(lifecycle(Transition::AttemptStarted)),
                Step::Output(RecoveryStreamEvent::Content("partial".into())),
                Step::Lifecycle(failed.clone()),
                Step::Output(RecoveryStreamEvent::Failed(error)),
            ]),
            on_drop: vec![],
        });
        let error = ACTIVE
            .scope(io.clone(), run_agent(config(&io.tool_root)))
            .await
            .unwrap_err();
        assert!(format!("{error:#}").contains(&format!("HTTP Some({status})")));
        let trace = events(dir.path());
        assert_eq!(
            payloads(&trace, "llm.recovery.final_failure"),
            [
                &json!({"error":{"outcome":"failed","failure":expected,"history":[]},"credential_redacted":false})
            ]
        );
        assert_eq!(
            payloads(&trace, "llm.recovery.lifecycle")[1],
            &serde_json::to_value(failed).unwrap()
        );
        let document = rendered(dir.path());
        let assistant = document["sessions"][0]["calls"]
            .as_array()
            .unwrap()
            .iter()
            .map(|call| call["assistant"].as_str().unwrap())
            .collect::<String>();
        assert_eq!(assistant, "partial");
        assert_eq!(
            payloads(&trace, "llm.recovery.accounting"),
            [&json!({"wire_attempts":1})]
        );
    }
}
#[tokio::test]
async fn public_run_safe_tool_key_boundaries_keep_truncated_fragments_inert() {
    for complete in [false, true] {
        let (_dir, io) = workspace();
        let mut output = vec![
            RecoveryStreamEvent::ToolFragment(
                json!({"index":0,"function":{"arguments":"{\"command\""}}),
            ),
            RecoveryStreamEvent::ToolFragment(
                json!({"index":0,"function":{"arguments":r#": "printf 'boundary-safe café'"#}}),
            ),
        ];
        if complete {
            output.push(completed(vec![tool()]));
        } else {
            output.extend([
                RecoveryStreamEvent::Content("DONE".into()),
                completed(vec![]),
            ]);
        }
        io.turns.lock().unwrap().push_back(Script::turn(output));
        if complete {
            io.turns.lock().unwrap().push_back(
                Script::turn(vec![
                    RecoveryStreamEvent::Content("DONE".into()),
                    completed(vec![]),
                ])
                .retag(second_identity()),
            );
        }
        let result = crate::recovery::TEST_CREDENTIALS
            .scope(
                vec!["abcdef".into()],
                ACTIVE.scope(io.clone(), run_agent(config(&io.tool_root))),
            )
            .await
            .unwrap();
        assert_eq!(result.final_summary, "DONE");
        if complete {
            assert_eq!(
                std::fs::read(io.tool_root.join("marker.txt")).unwrap(),
                b"executed"
            );
        } else {
            assert!(!io.tool_root.join("marker.txt").exists());
        }
    }
}
#[cfg(unix)]
#[tokio::test]
async fn public_run_pending_admission_expires_on_injected_time_without_replaying() {
    use std::io::BufRead;
    use std::os::unix::fs::PermissionsExt;
    let (dir, io) = workspace();
    for name in ["admission-ready", "admission-control"] {
        assert!(
            std::process::Command::new("mkfifo")
                .arg(dir.path().join(name))
                .status()
                .unwrap()
                .success()
        );
    }
    let _control = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(dir.path().join("admission-control"))
        .unwrap();
    let ready_path = dir.path().join("admission-ready");
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let reader = std::thread::spawn(move || {
        let mut line = String::new();
        std::io::BufReader::new(std::fs::File::open(ready_path).unwrap())
            .read_line(&mut line)
            .unwrap();
        ready_tx.send(line).unwrap();
    });
    let command = dir.path().join("admit");
    std::fs::write(
        &command,
        format!(
            "#!/bin/sh\ncat > '{}'\nprintf 'admission-ready\\n' > '{}'\nread -r release < '{}'\n",
            dir.path().join("input.json").display(),
            dir.path().join("admission-ready").display(),
            dir.path().join("admission-control").display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&command, std::fs::Permissions::from_mode(0o700)).unwrap();
    let error = failure(Outcome::AdmissionRejected, Some(503), Provider::Omlx);
    let expected = serde_json::to_value(&error).unwrap();
    io.turns.lock().unwrap().push_back(Script {
        steps: VecDeque::from([
            Step::Lifecycle(lifecycle(Transition::AttemptStarted)),
            wire(),
            Step::Admission {
                failure: error.failure.clone(),
                decision: AdmissionDecision::Reject,
            },
            Step::Output(RecoveryStreamEvent::Failed(error)),
        ]),
        on_drop: vec![],
    });
    let mut config = config(&io.tool_root);
    config.recovery.recover_requests = true;
    config.recovery.recovery_max_attempts = 2;
    config.recovery.recovery_admission_command = Some(command);
    let clock = Arc::new(crate::test_clock::Clock::default());
    crate::test_clock::ACTIVE
        .scope(clock.clone(), async {
            let time = async {
                assert_eq!(ready_rx.await.unwrap(), "admission-ready\n");
                clock.wait_for(std::time::Duration::from_secs(5)).await;
                clock.advance(std::time::Duration::from_secs(5));
            };
            let (result, ()) = tokio::join!(ACTIVE.scope(io.clone(), run_agent(config)), time);
            assert!(format!("{:#}", result.unwrap_err()).contains("AdmissionRejected"));
        })
        .await;
    reader.join().unwrap();
    assert_eq!(io.requests.lock().unwrap().len(), 1);
    assert_eq!(
        payloads(&events(dir.path()), "llm.recovery.final_failure"),
        [&json!({"error":expected,"credential_redacted":false})]
    );
}
#[tokio::test]
async fn public_run_cancellation_before_terminal_delivery_prevents_completed_tool_execution() {
    let (dir, io) = workspace();
    let mut script = Script::turn(vec![completed(vec![tool()])]);
    script.steps.insert(2, Step::Cancel);
    io.turns.lock().unwrap().push_back(script);
    let error = ACTIVE
        .scope(io.clone(), run_agent(config(&io.tool_root)))
        .await
        .unwrap_err();
    assert!(format!("{error:#}").contains("safe worker completion rejected"));
    assert!(!io.tool_root.join("marker.txt").exists());
    let trace = events(dir.path());
    assert_eq!(
        payloads(&trace, "llm.recovery.lifecycle"),
        [
            &serde_json::to_value(lifecycle(Transition::AttemptStarted)).unwrap(),
            &serde_json::to_value(lifecycle(Transition::AttemptSucceeded)).unwrap()
        ]
    );
    assert_eq!(
        payloads(&trace, "llm.recovery.final_failure"),
        [
            &json!({"outcome":"cancelled","credential_redacted":false,"report":{"identity":identity(),"progress":{"headers_received":false,"raw_bytes":0,"observed":{"reasoning_bytes":0,"content_bytes":0,"tool_fragments":0,"completed_tool_calls":0},"delivered":{"reasoning_bytes":0,"content_bytes":0,"tool_fragments":0,"completed_tool_calls":0}},"history":[]}})
        ]
    );
    assert_eq!(
        payloads(&trace, "llm.recovery.accounting"),
        [&json!({"wire_attempts":1})]
    );
}
