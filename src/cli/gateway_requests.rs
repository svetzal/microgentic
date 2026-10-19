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

// Literal conversational specification: do not call production guidance or
// descriptor builders here. The shared benchmark fixture remains unchanged.
fn everyday_request(wire: Wire, prompt: &str) -> Value {
    let system = [
        "You are Microgentic, a helpful conversational assistant with coding tools available.",
        "Respond to the user's message in the context of this conversation. No engineering task is assumed.",
        "Greetings, questions, explanations, and discussion can receive ordinary conversational replies without tools or validation.",
        "Perform work only when the user requests it, including clear follow-up authorization in the conversation. Discussed possibilities and quoted examples are not instructions to execute. Ask a concise question if the intended work is unclear.",
        "Use tools when the requested work or answer requires them. Do not inspect the project, create files, change code, or run commands just because this conversation is open.",
        "All file tool paths and shell commands are rooted at the current project directory.",
        "When working on code, inspect relevant files before guessing their contents. Use bounded file reads and focused edits; preserve unrelated work and existing Git stashes.",
        "Use edit_file for existing source edits. Use write_file for new files, or for existing source only after reading the complete current file and preserving unrelated content.",
        "After source changes, obtain fresh deterministic validation before claiming the work is complete. Repair reported failures narrowly and rerun the appropriate check. Earlier conversation observations are not fresh validation.",
        "For requested new projects, use the requested language and layout, and create an appropriate .gitignore. Do not assume Rust or create Cargo files without a request for that project.",
        "Ignore generated build, dependency, cache, and virtual-environment directories unless the request requires them.",
        "Answer naturally with useful results, explanations, or concrete blockers. A visible conversational reply ends the exchange; DONE and FAIL tokens are not required.",
        "For arithmetic with more than one trivial step, including sums, ratios, percentages and unit conversions, call calculate instead of working it out in your head, and use its results.",
    ].join("\n");
    let mut request = expected_request(
        wire,
        "Some(Low)",
        false,
        json!([
            {"role":"system", "content":system},
            {"role":"user", "content":prompt}
        ]),
    );
    request["model"] = json!("custom-model");
    request["completion"] = json!(
        "CompletionConfig { temperature: Some(1.0), num_ctx: Some(32768), max_tokens: Some(16384), top_p: None, top_k: None, response_format: None, reasoning_effort: Some(Low), reasoning_profile: None, preserve_reasoning: false, max_tool_iterations: 75 }"
    );
    request["max_tokens"] = json!(16384);
    request["num_ctx"] = json!(32768);
    request["tools"].as_array_mut().unwrap().push(json!({
        "type":"function",
        "function":{
            "name":"calculate",
            "description":r#"Exact arithmetic. Do sums, ratios, percentages and conversions here instead of in your head; models make arithmetic slips. Pass one expression, or several separated by newlines or ";"; a line such as "total = 140+90" names a value for later lines. Supports + - * / // % ^ and parentheses, sqrt abs round floor ceil min max ln log10 log2 exp, and pi e. Other names must be defined that way first. Example: {"expression": "140+90+70\n1474/1.1"} returns "140 + 90 + 70 = 300" and "1474 / 1.1 = 1340"."#,
            "parameters":{
                "type":"object",
                "properties":{"expression":{"type":"string", "description":r#"One or more arithmetic expressions, separated by newlines or ";"."#}},
                "required":["expression"]
            }
        }
    }));
    request
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
        // Each path must match a specification, not the other path's capture.
        let expected = everyday_request(wire, prompt);
        assert_eq!(
            gateway.requests().as_slice(),
            std::slice::from_ref(&expected),
            "saved defaults/stdin"
        );
        assert_eq!(explicit.requests(), [expected], "explicit arguments");
        assert_eq!(gateway.metadata_models(), ["custom-model"]);
        assert_eq!(explicit.metadata_models(), ["custom-model"]);
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
                let output = std::process::Command::new("git")
                    .args(["stash", "list", "--format=%H"])
                    .current_dir(&workspace.project)
                    .output()
                    .unwrap();
                assert!(output.status.success());
                output.stdout
            };
            let before = inventory();
            let directory = std::process::Command::new("git")
                .args(["rev-parse", "--absolute-git-dir"])
                .current_dir(&workspace.project)
                .output()
                .unwrap();
            assert!(directory.status.success());
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
                std::str::from_utf8(&after).unwrap().lines().count(),
                if create_new { 2 } else { 1 }
            );
            // Git, not a captured request or the runtime inventory builder, supplies
            // the only dynamic values in this complete request oracle.
            let before_ids: Vec<_> = std::str::from_utf8(&before).unwrap().lines().collect();
            let entry = format!(
                "{{\"state\":\"captured\",\"git_directory\":{},\"object_ids\":{}}}",
                json!(std::str::from_utf8(&directory.stdout).unwrap().trim()),
                json!(before_ids)
            );
            let mut messages = initial_messages();
            let user = messages[1]["content"].as_str().unwrap();
            messages[1]["content"] = json!(format!(
                "{user}\n\nGit stash inventory at entry (captured before agent tools): {entry}. Preserve pre-existing stashes. Any new unresolved stash or removed baseline stash withholds DONE; restoration/disposal is an explicit agent decision, never automatic. Object IDs, not shifting stash indices, identify entries. Shared-repository attribution is uncertain."
            ));
            let mut expected = vec![expected_request(wire, "None", false, messages.clone())];
            if create_new {
                // These IDs are deliberately literal, independent of the scripted reply.
                let call = json!([{"id":"stash-tool", "name":"shell_command", "arguments":{"command":"printf new > tracked.txt && git stash push -q"}}]);
                let shell_result = json!({
                    "cwd":"", "command":"printf new > tracked.txt && git stash push -q",
                    "command_family":"printf", "validation_probe":false,
                    "validation_probe_clears_pending_source_writes":false,
                    "total_shell_probes":0, "total_write_operations":0,
                    "status":0, "success":true,
                    "stdout":"", "stdout_truncated":false,
                    "stderr":"", "stderr_truncated":false,
                    "shell_mutation_sensed":false, "shell_mutation_paths":[],
                    "shell_mutation_requires_validation":false,
                    "shell_mutation_snapshot_error":null, "repair_required":null
                });
                messages.as_array_mut().unwrap().extend([
                    json!({"role":"assistant", "content":"", "tool_calls":call}),
                    json!({"role":"tool", "tool_calls":call, "content":shell_result.to_string()}),
                ]);
                expected.push(expected_request(wire, "None", false, messages.clone()));
                let after_ids: Vec<_> = std::str::from_utf8(&after).unwrap().lines().collect();
                let added: Vec<_> = after_ids
                    .into_iter()
                    .filter(|id| !before_ids.contains(id))
                    .collect();
                assert_eq!(added.len(), 1);
                messages.as_array_mut().unwrap().extend([
                    json!({"role":"assistant", "content":"DONE"}),
                    json!({"role":"user", "content":format!("DONE withheld: Git stash state differs from entry or cannot be verified. New unresolved stash object IDs: {added:?}. Missing baseline stash object IDs: []. Inventory unknown/repository changed: false. Compare the current workspace with the entry inventory. Resolve new stashes explicitly and validate the actual working tree before DONE. Do not automatically pop/drop pre-existing stashes or discard work. If ownership or restoration is unclear, report FAIL. Stashes appeared during this run; shared-repository attribution is uncertain.")}),
                ]);
                expected.push(expected_request(wire, "None", false, messages));
            }
            assert_eq!(
                gateway.requests(),
                expected,
                "complete stash request sequence for {} (new={create_new})",
                wire.name()
            );
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
