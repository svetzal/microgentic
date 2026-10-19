//! Gateway inputs through the production CLI dispatcher, without HTTP transport.
use super::test_support::*;
use mojentic::llm::gateway::StreamChunk;
use serde_json::{Value, json};

const PROMPT: &str = "Inspect the project with list_tree, then finish. Unicode 世界.";

// Reuse the reviewed, versioned continuity specification, never a capture.
fn expected_request(wire: Wire, effort: &str, preserve: bool, messages: Value) -> Value {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/benchmark_continuity.json"
    ))
    .unwrap();
    json!({
        "model": fixture["models"][wire.name()],
        "messages": messages,
        "tools": fixture["tools"],
        "completion": format!("CompletionConfig {{ temperature: Some(1.0), num_ctx: Some(262144), max_tokens: Some(8192), top_p: None, top_k: None, response_format: None, reasoning_effort: {effort}, reasoning_profile: None, preserve_reasoning: {preserve}, max_tool_iterations: 75 }}"),
        "max_tokens": 8192, "num_ctx": 262144, "temperature": 1.0
    })
}

fn initial_messages() -> Value {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/benchmark_continuity.json"
    ))
    .unwrap();
    fixture["messages"]["goal"][0].clone()
}

fn run(workspace: &Workspace, gateway: &ScriptedGateway, extra: &[&str]) -> Output {
    std::fs::write(workspace.base.join("task.md"), PROMPT).unwrap();
    let mut args = vec![
        "run",
        "--experiment",
        workspace.base.to_str().unwrap(),
        "--provider",
        gateway.wire().name(),
        "--max-tokens",
        "8192",
        "--max-iterations",
        "3",
        "--transcript-policy",
        "full-transcript",
    ];
    args.extend_from_slice(extra);
    execute(legacy_binary(), &args, gateway, workspace)
}

#[test]
fn literal_ids_and_reasoning_survive_tools_and_outer_turns() {
    for wire in [Wire::Omlx, Wire::Ollama] {
        for (preserve, profile) in [(false, false), (true, false), (false, true), (true, true)] {
            let workspace = Workspace::new();
            let gateway = ScriptedGateway::turns(
                wire,
                vec![
                    vec![
                        StreamChunk::Thinking("FIRST_REASONING_SENTINEL".into()),
                        content("Inspecting now."),
                        tool_with_id("inspect-1", "list_tree", json!({})),
                    ],
                    vec![
                        StreamChunk::Thinking("SECOND_REASONING_SENTINEL".into()),
                        content("The inspection is complete; one more step."),
                    ],
                    vec![content("DONE")],
                ],
            );
            let mut controls = vec!["--reasoning-effort", "medium"];
            if preserve {
                controls.push("--preserve-reasoning");
            }
            if profile {
                controls.extend(["--reasoning-profile", "qwen3.8"]);
            }
            let output = run(&workspace, &gateway, &controls);
            assert!(output.status.success(), "{}", stderr(&output));
            let mut messages = initial_messages();
            let mut expected = vec![expected_request(
                wire,
                "Some(Medium)",
                preserve,
                messages.clone(),
            )];
            let mut assistant = json!({"role":"assistant", "content":"Inspecting now.", "tool_calls":[{"id":"inspect-1", "name":"list_tree", "arguments":{}}]});
            if preserve {
                assistant["reasoning_content"] = json!("FIRST_REASONING_SENTINEL");
            }
            messages.as_array_mut().unwrap().extend([
                assistant,
                json!({"role":"tool", "content":"{\"entries\":[],\"entry_count\":0,\"include_hidden\":false,\"max_depth\":4,\"max_entries\":200,\"path\":\"\",\"truncated\":false}", "tool_calls":[{"id":"inspect-1", "name":"list_tree", "arguments":{}}]}),
            ]);
            expected.push(expected_request(
                wire,
                "Some(Medium)",
                preserve,
                messages.clone(),
            ));
            let mut assistant =
                json!({"role":"assistant", "content":"The inspection is complete; one more step."});
            if preserve {
                assistant["reasoning_content"] = json!("SECOND_REASONING_SENTINEL");
            }
            messages.as_array_mut().unwrap().extend([
                assistant,
                json!({"role":"user", "content":"Continue from the current experiment state. Use tools as needed. Run deterministic validation before replying DONE. Reply FAIL only if blocked."}),
            ]);
            expected.push(expected_request(wire, "Some(Medium)", preserve, messages));
            if profile {
                for request in &mut expected {
                    request["completion"] = json!(request["completion"].as_str().unwrap().replace(
                        "reasoning_profile: None",
                        "reasoning_profile: Some(ReasoningProfile { name: \"qwen3.8\", supported_efforts: [Disabled, Low, Medium, XHigh], supports_reasoning_history: true })",
                    ));
                }
            }
            assert_eq!(
                gateway.requests(),
                expected,
                "complete gateway requests, literal IDs and optional reasoning fields"
            );
            let report = summary(&output);
            assert_eq!(report["preserve_reasoning"], preserve);
            let trace = std::fs::read_to_string(report["trace_file"].as_str().unwrap()).unwrap();
            assert!(trace.contains("FIRST_REASONING_SENTINEL"));
            assert!(trace.contains("SECOND_REASONING_SENTINEL"));
            assert!(!trace.contains("test-key"));
            let path = std::path::Path::new(report["trace_file"].as_str().unwrap());
            let ledgers = trace_payloads(path, "llm.context_assembly.ledger");
            assert_eq!(ledgers.len(), 3);
            assert_eq!(
                ledgers[1]["components"][2]["reasoning_chars"],
                if preserve { 24 } else { 0 }
            );
            assert_eq!(
                ledgers[2]["components"][2]["reasoning_chars"],
                if preserve { 24 } else { 0 }
            );
            assert_eq!(
                ledgers[2]["components"][4]["reasoning_chars"],
                if preserve { 25 } else { 0 }
            );
            let packets = trace_payloads(path, "llm.provider_request.assembled");
            assert_eq!(packets[0]["completion"]["preserve_reasoning"], preserve);
            assert_eq!(
                packets[0]["provider_controls"]["chat_template_kwargs"],
                if wire == Wire::Omlx && preserve {
                    json!({"preserve_thinking":true})
                } else {
                    Value::Null
                }
            );
            if profile {
                assert_eq!(
                    packets[0]["completion"]["reasoning_profile"],
                    json!({
                        "name":"qwen3.8", "supported_efforts":["disabled","low","medium","xhigh"], "supports_reasoning_history":true
                    })
                );
            }
        }
    }
}

#[test]
fn worker_efforts_and_both_token_flags_reach_the_gateway() {
    for wire in [Wire::Omlx, Wire::Ollama] {
        for (flag, effort, expected) in [
            ("--max-tokens", None, "None"),
            ("--num-predict", Some("disabled"), "Some(Disabled)"),
            ("--max-tokens", Some("low"), "Some(Low)"),
            ("--num-predict", Some("medium"), "Some(Medium)"),
            ("--max-tokens", Some("high"), "Some(High)"),
            ("--num-predict", Some("xhigh"), "Some(XHigh)"),
            ("--max-tokens", Some("enabled"), "Some(Enabled)"),
        ] {
            let workspace = Workspace::new();
            std::fs::write(workspace.base.join("task.md"), PROMPT).unwrap();
            let gateway = ScriptedGateway::replying(wire, vec![content("DONE")]);
            let mut args = vec![
                "run",
                "--experiment",
                workspace.base.to_str().unwrap(),
                "--provider",
                wire.name(),
                flag,
                "8192",
            ];
            if let Some(effort) = effort {
                args.extend(["--reasoning-effort", effort]);
            }
            let output = execute(legacy_binary(), &args, &gateway, &workspace);
            assert!(output.status.success(), "{}", stderr(&output));
            assert_eq!(
                gateway.requests(),
                [expected_request(wire, expected, false, initial_messages())]
            );
            let report = summary(&output);
            let trace = std::fs::read_to_string(report["trace_file"].as_str().unwrap()).unwrap();
            assert!(!trace.contains("test-key"));
            let packets = trace_payloads(
                std::path::Path::new(report["trace_file"].as_str().unwrap()),
                "llm.provider_request.assembled",
            );
            assert_eq!(packets[0]["provider"], wire.name());
            assert_eq!(packets[0]["completion"]["reasoning_effort"], json!(effort));
            assert_eq!(packets[0]["completion"]["max_tokens"], 8192);
            if wire == Wire::Omlx {
                assert_eq!(
                    packets[0]["provider_controls"],
                    json!({
                        "max_tokens":8192, "enable_thinking":match effort {Some("disabled")=>json!(false),Some("enabled")=>json!(true),_=>Value::Null},
                        "reasoning_effort":effort.filter(|e| !matches!(*e,"disabled"|"enabled")), "chat_template_kwargs":null, "context_window_source":"server_model_configuration"
                    })
                );
            } else {
                assert_eq!(
                    packets[0]["provider_controls"],
                    json!({
                        "num_predict":8192, "num_ctx":262144, "think":match effort {Some("disabled")=>json!(false),Some("enabled")=>json!(true),_=>json!(effort)}, "context_window_source":"request_options"
                    })
                );
            }
        }
    }
}

#[test]
fn qwen_profile_rejects_high_before_gateway_invocation() {
    for wire in [Wire::Omlx, Wire::Ollama] {
        let workspace = Workspace::new();
        let gateway = ScriptedGateway::replying(wire, vec![content("DONE")]);
        std::fs::write(workspace.base.join("task.md"), PROMPT).unwrap();
        let result = execute_input_result(
            legacy_binary(),
            &[
                "run",
                "--experiment",
                workspace.base.to_str().unwrap(),
                "--provider",
                wire.name(),
                "--reasoning-profile",
                "qwen3.8",
                "--reasoning-effort",
                "high",
            ],
            &gateway,
            &workspace,
            b"",
        );
        let error = result.err().expect("unsupported profile effort must fail");
        assert!(error.to_string().contains("unsupported by profile qwen3.8"));
        assert!(gateway.requests().is_empty());
        assert!(gateway.metadata_models().is_empty());
    }
}

#[test]
fn saved_setup_and_unicode_stdin_keep_stdout_and_workspace_clean() {
    for wire in [Wire::Omlx, Wire::Ollama] {
        let workspace = Workspace::new();
        let gateway = ScriptedGateway::replying(wire, vec![content("DONE")]);
        let setup = execute(
            microgentic(),
            &[
                "setup",
                "--provider",
                wire.name(),
                "--model",
                "custom-model",
                "--reasoning-effort",
                "low",
                "--apply",
                "--json",
            ],
            &gateway,
            &workspace,
        );
        assert!(setup.status.success(), "{}", stderr(&setup));
        assert!(gateway.requests().is_empty());
        let prompt = "Hello!\nUnicode: 世界\n";
        let trace_dir = workspace.base.join("traces");
        let piped = execute_input(
            microgentic(),
            &["run", "--trace-dir", trace_dir.to_str().unwrap()],
            &gateway,
            &workspace,
            prompt.as_bytes(),
        );
        assert!(piped.status.success(), "{}", stderr(&piped));
        assert_eq!(stdout(&piped), "DONE\n");
        let explicit = ScriptedGateway::replying(wire, vec![content("DONE")]);
        let output = execute(
            microgentic(),
            &[
                "run",
                "--prompt",
                prompt,
                "--trace-dir",
                trace_dir.to_str().unwrap(),
                "--provider",
                wire.name(),
                "--model",
                "custom-model",
                "--reasoning-effort",
                "low",
            ],
            &explicit,
            &workspace,
        );
        assert!(output.status.success(), "{}", stderr(&output));
        assert_eq!(stdout(&output), "DONE\n");
        // Compare entire inputs, including all descriptors and absent fields.
        assert_eq!(gateway.requests(), explicit.requests());
        let requests = gateway.requests();
        assert_eq!(requests[0]["model"], "custom-model");
        assert_eq!(
            requests[0]["messages"][1],
            json!({"role":"user", "content":prompt})
        );
        assert_eq!(
            requests[0]["completion"],
            "CompletionConfig { temperature: Some(1.0), num_ctx: Some(32768), max_tokens: Some(16384), top_p: None, top_k: None, response_format: None, reasoning_effort: Some(Low), reasoning_profile: None, preserve_reasoning: false, max_tool_iterations: 75 }"
        );
        assert_eq!(gateway.metadata_models(), ["custom-model"]);
        let traces: Vec<_> = std::fs::read_dir(&trace_dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(traces.len(), 2);
        for trace in traces {
            let packets = trace_payloads(&trace, "llm.provider_request.assembled");
            assert_eq!(
                packets[0]["provider"],
                wire.name(),
                "saved provider is honored"
            );
        }
        assert_eq!(std::fs::read_dir(&workspace.project).unwrap().count(), 0);
    }
}

#[test]
fn entry_stash_is_preserved_and_new_stash_rejects_done_both_providers() {
    for wire in [Wire::Omlx, Wire::Ollama] {
        for create_new in [false, true] {
            let workspace = Workspace::new();
            workspace.write("tracked.txt", "seed");
            workspace.git_init();
            workspace.write("tracked.txt", "prior");
            git(&workspace.project, &["stash", "push", "-q"]);
            let inventory = || {
                std::process::Command::new("git")
                    .args(["stash", "list", "--format=%H"])
                    .current_dir(&workspace.project)
                    .output()
                    .unwrap()
                    .stdout
            };
            let before = inventory();
            let mut turns = Vec::new();
            if create_new {
                turns.push(vec![tool_with_id(
                    "stash-tool",
                    "shell_command",
                    json!({"command":"printf new > tracked.txt && git stash push -q"}),
                )]);
            }
            turns.push(vec![content("DONE")]);
            if create_new {
                turns.push(vec![content("FAIL unresolved stash")]);
            }
            let gateway = ScriptedGateway::turns(wire, turns);
            let output = run(&workspace, &gateway, &[]);
            assert!(output.status.success(), "{}", stderr(&output));
            let report = summary(&output);
            assert_eq!(
                report["final_summary"],
                if create_new {
                    "FAIL unresolved stash"
                } else {
                    "DONE"
                }
            );
            let after = inventory();
            assert!(after.ends_with(&before), "the original stash must survive");
            assert_eq!(
                String::from_utf8(after).unwrap().lines().count(),
                if create_new { 2 } else { 1 }
            );
            let requests = gateway.requests();
            assert!(
                requests[0]["messages"]
                    .to_string()
                    .contains("Git stash inventory at entry")
            );
            if create_new {
                assert_eq!(
                    requests[1]["messages"][2]["tool_calls"][0]["id"],
                    "stash-tool"
                );
                assert_eq!(
                    requests[1]["messages"][3]["tool_calls"][0]["id"],
                    "stash-tool"
                );
                assert!(
                    requests.last().unwrap()["messages"]
                        .to_string()
                        .contains("DONE withheld: Git stash state")
                );
            }
            let trace = std::fs::read_to_string(report["trace_file"].as_str().unwrap()).unwrap();
            assert_eq!(trace.contains("agent.git_stash.done_rejected"), create_new);
        }
    }
}

#[test]
fn modern_and_legacy_contract_requests_are_complete_and_equal() {
    for wire in [Wire::Omlx, Wire::Ollama] {
        let workspace = Workspace::new();
        let contract = workspace.base.join("contract.json");
        std::fs::write(&contract, json!({"profile":{"id":"terminal_work", "version":"terminal_work_profile.v1"}, "guidance":PROMPT,"read_scope":["./**"],"write_scope":["./**"],"probes":[]}).to_string()).unwrap();
        std::fs::write(&workspace.config, "malformed personal configuration").unwrap();
        let mut captures = vec![];
        for binary in [legacy_binary(), microgentic()] {
            let gateway = ScriptedGateway::turns(
                wire,
                vec![
                    vec![
                        content("Inspecting."),
                        tool_with_id("inspect-1", "list_tree", json!({})),
                    ],
                    vec![content("DONE")],
                ],
            );
            let mut args = vec![
                "run",
                "--experiment",
                workspace.base.to_str().unwrap(),
                "--contract",
                contract.to_str().unwrap(),
                "--provider",
                wire.name(),
                "--reasoning-effort",
                "low",
                "--max-iterations",
                "3",
                "--json",
            ];
            if binary == microgentic() {
                args.extend([
                    "--prompt",
                    PROMPT,
                    "--max-tokens",
                    "8192",
                    "--max-model-interactions",
                    "75",
                ]);
            } else {
                args.extend(["--num-predict", "8192", "--max-tool-iterations", "75"]);
            }
            let output = execute(binary, &args, &gateway, &workspace);
            assert!(output.status.success(), "{}", stderr(&output));
            let report = summary(&output);
            assert_eq!(report["tool_root"], workspace.project.to_str().unwrap());
            assert_eq!(report["final_summary"], "Inspecting.\nDONE");
            let fixture: Value = serde_json::from_str(include_str!(
                "../../tests/fixtures/benchmark_continuity.json"
            ))
            .unwrap();
            let mut messages = fixture["messages"]["contract"][0].clone();
            let first = expected_request(wire, "Some(Low)", false, messages.clone());
            messages.as_array_mut().unwrap().extend([
                json!({"role":"assistant","content":"Inspecting.","tool_calls":[{"id":"inspect-1","name":"list_tree","arguments":{}}]}),
                json!({"role":"tool","content":"{\"entries\":[],\"entry_count\":0,\"include_hidden\":false,\"max_depth\":4,\"max_entries\":200,\"path\":\"\",\"truncated\":false}","tool_calls":[{"id":"inspect-1","name":"list_tree","arguments":{}}]}),
            ]);
            assert_eq!(
                gateway.requests(),
                [first, expected_request(wire, "Some(Low)", false, messages)]
            );
            captures.push(gateway.requests());
        }
        assert_eq!(captures[0], captures[1]);
    }
}
