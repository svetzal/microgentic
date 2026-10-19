//! Consumer baseline at the normalized gateway boundary; not Mojentic verification.
use super::gateway_requests::{expected_request, initial_messages};
use super::test_support::*;
use mojentic::llm::gateway::{StreamChunk, StreamMetrics, StreamProgress};
use serde_json::{Value, json};

const CREDENTIAL: &str = "prerequisite-private-transport-key";

fn progress(
    frame_index: usize,
    content_chars: usize,
    thinking_chars: usize,
    done: bool,
) -> StreamChunk {
    StreamChunk::Progress(StreamProgress {
        provider: "ollama".into(),
        frame_index,
        done,
        content_chars,
        thinking_chars,
        tool_call_count: 0,
        accumulated_tool_call_count: 0,
    })
}

fn baseline(wire: Wire) {
    let workspace = Workspace::new();
    std::fs::write(workspace.base.join("task.md"), "Finish with DONE.\n").unwrap();
    let mut chunks = vec![];
    if wire == Wire::Ollama {
        chunks.extend([progress(1, 0, 0, false), progress(2, 0, 20, false)]);
    }
    chunks.push(StreamChunk::Thinking("Synthetic reasoning.".into()));
    if wire == Wire::Ollama {
        chunks.push(progress(3, 4, 0, false));
    }
    chunks.push(content("DONE"));
    if wire == Wire::Ollama {
        chunks.extend([
            progress(4, 0, 0, true),
            StreamChunk::Metrics(StreamMetrics {
                provider: "ollama".into(),
                total_duration_ns: Some(2_000_000_000),
                load_duration_ns: Some(100_000_000),
                prompt_eval_count: Some(11),
                prompt_eval_duration_ns: Some(400_000_000),
                eval_count: Some(7),
                eval_duration_ns: Some(1_000_000_000),
                tokens_per_second: Some(7.0),
                ..Default::default()
            }),
        ]);
    }
    let response_artifact = format!("{chunks:?}");
    let gateway = ScriptedGateway::replying(wire, chunks);
    let output = crate::recovery::TEST_CREDENTIALS.sync_scope(vec![CREDENTIAL.into()], || {
        execute(
            legacy_binary(),
            &[
                "run",
                "--experiment",
                workspace.base.to_str().unwrap(),
                "--provider",
                wire.name(),
                "--model",
                "synthetic-model",
                "--max-iterations",
                "1",
                "--max-model-interactions",
                "1",
                "--reasoning-effort",
                "enabled",
                "--preserve-reasoning",
                "--max-tokens",
                "8192",
            ],
            &gateway,
            &workspace,
        )
    });
    assert!(output.status.success(), "{}", stderr(&output));
    let mut messages = initial_messages();
    messages[1]["content"] = json!(messages[1]["content"].as_str().unwrap().replace(
        "Inspect the project with list_tree, then finish. Unicode 世界.",
        "Finish with DONE.\n",
    ));
    let mut expected = expected_request(wire, "Some(Enabled)", true, messages);
    expected["model"] = json!("synthetic-model");
    expected["num_ctx"] = json!(32768);
    expected["completion"] = json!(
        "CompletionConfig { temperature: Some(1.0), num_ctx: Some(32768), max_tokens: Some(8192), top_p: None, top_k: None, response_format: None, reasoning_effort: Some(Enabled), reasoning_profile: None, preserve_reasoning: true, max_tool_iterations: 1 }"
    );
    assert_eq!(gateway.requests(), [expected], "complete gateway input");
    assert_eq!(gateway.request_count(), 1);
    let report = summary(&output);
    assert_eq!(report["final_summary"], "DONE");
    assert_eq!(report["max_model_interactions"], 1);
    let trace_path = std::path::Path::new(report["trace_file"].as_str().unwrap());
    let traces: Vec<_> = std::fs::read_dir(trace_path.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
        .collect();
    assert_eq!(traces, [trace_path]);
    let trace = std::fs::read_to_string(trace_path).unwrap();
    let events: Vec<Value> = trace
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let packets: Vec<_> = events
        .iter()
        .filter(|event| event["kind"] == "llm.provider_request.assembled")
        .collect();
    assert_eq!(packets.len(), 1);
    assert_eq!(
        packets[0]["payload"]["schema_version"],
        "provider_request.v1"
    );
    assert_eq!(packets[0]["payload"]["provider"], wire.name());
    assert_eq!(
        packets[0]["payload"]["provider_controls"],
        match wire {
            Wire::Ollama => json!({"num_predict":8192, "num_ctx":32768, "think":true,
            "context_window_source":"request_options"}),
            Wire::Omlx =>
                json!({"max_tokens":8192, "enable_thinking":true, "reasoning_effort":null,
            "chat_template_kwargs":{"preserve_thinking":true}, "context_window_source":"server_model_configuration"}),
        }
    );
    let finished: Vec<_> = events
        .iter()
        .filter(|event| event["kind"] == "agent.stream.finished")
        .map(|event| event["payload"].clone())
        .collect();
    assert_eq!(
        finished,
        [json!({"turn":1, "chunks":1, "chars":4,
        "thinking_chunks":1, "thinking_chars":20, "llm_call_count":1})]
    );
    let telemetry: Vec<_> = events
        .iter()
        .filter(|event| {
            matches!(
                event["kind"].as_str().unwrap(),
                "llm.stream.progress"
                    | "llm.stream.thinking"
                    | "agent.stream.chunk"
                    | "llm.stream.metrics"
            )
        })
        .collect();
    let kinds: Vec<_> = telemetry
        .iter()
        .map(|event| event["kind"].as_str().unwrap())
        .collect();
    if wire == Wire::Ollama {
        assert_eq!(
            kinds,
            [
                "llm.stream.progress",
                "llm.stream.progress",
                "llm.stream.thinking",
                "llm.stream.progress",
                "agent.stream.chunk",
                "llm.stream.progress",
                "llm.stream.metrics"
            ]
        );
        let payloads: Vec<_> = telemetry
            .iter()
            .filter(|event| event["kind"] == "llm.stream.progress")
            .map(|event| event["payload"].clone())
            .collect();
        let expected_progress: Vec<_> = [
            (1, false, 0, 0, "WaitingForFirstToken"),
            (2, false, 0, 20, "Generating"),
            (3, false, 4, 0, "Generating"),
            (4, true, 0, 0, "DoneFrame"),
        ]
        .into_iter()
        .map(|(index, done, content, thinking, state)| {
            json!({
                "turn":1, "llm_call_depth":0, "provider":"ollama", "frame_index":index,
                "done":done, "content_chars":content, "thinking_chars":thinking,
                "tool_call_count":0, "accumulated_tool_call_count":0,
                "stream_progress_frame_count":index, "call_stream_progress_frame_count":index,
                "tool_call_progress_frame_count":0, "call_tool_call_progress_frame_count":0,
                "progress_state":state,
            })
        })
        .collect();
        assert_eq!(payloads, expected_progress);
        let metrics = &telemetry.last().unwrap()["payload"];
        assert_eq!(
            metrics,
            &json!({
                "provider":"ollama", "total_duration_ns":2_000_000_000u64,
                "load_duration_ns":100_000_000, "prompt_eval_count":11,
                "prompt_eval_duration_ns":400_000_000, "eval_count":7,
                "eval_duration_ns":1_000_000_000, "tokens_per_second":7.0,
                "turn":1, "llm_call_depth":0, "turn_thinking_chars":20, "call_thinking_chars":20,
                "packet_type":"multi-file-edit", "expected_output_tokens":4096, "context_band":"unknown",
            })
        );
        let samples: Vec<_> = events
            .iter()
            .filter(|event| event["kind"] == "llm.throughput.sample")
            .collect();
        assert_eq!(samples.len(), 1);
        let sample = &samples[0]["payload"];
        for (key, value) in json!({"model":"synthetic-model", "provider":"ollama",
            "prompt_eval_count":11, "prompt_eval_duration_ns":400_000_000,
            "eval_count":7, "eval_duration_ns":1_000_000_000, "tokens_per_second":7.0,
            "turn":1, "llm_call_depth":0, "packet_type":"multi-file-edit",
            "expected_output_tokens":4096, "context_band":"unknown"})
        .as_object()
        .unwrap()
        {
            assert_eq!(&sample[key], value, "throughput field {key}");
        }
        let persisted =
            std::fs::read_to_string(workspace.base.join("model-throughput.jsonl")).unwrap();
        assert_eq!(persisted.lines().count(), 1);
        assert_eq!(serde_json::from_str::<Value>(&persisted).unwrap(), *sample);
    } else {
        assert_eq!(kinds, ["llm.stream.thinking", "agent.stream.chunk"]);
        assert!(!workspace.base.join("model-throughput.jsonl").exists());
    }
    let thinking = telemetry
        .iter()
        .find(|event| event["kind"] == "llm.stream.thinking")
        .unwrap();
    assert_eq!(thinking["payload"]["preview"], "Synthetic reasoning.");
    for artifact in [
        stdout(&output),
        stderr(&output),
        trace,
        response_artifact,
        serde_json::to_string(&gateway.requests()).unwrap(),
    ] {
        assert!(!artifact.contains(CREDENTIAL), "credential in artifact");
    }
}

#[test]
fn ollama_baseline_preserves_ordered_progress_and_metrics() {
    baseline(Wire::Ollama);
}

#[test]
fn omlx_baseline_preserves_native_reasoning_and_controls() {
    baseline(Wire::Omlx);
}
