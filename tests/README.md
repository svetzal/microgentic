# CLI and terminal coverage

## Recovery caller boundary and worker characterization

Stacey's revised 2026-10-19 boundary supersedes the earlier retirement of
Microgentic's recovery consumption. `src/recovery_test_io.rs` calls public
`agent::run_agent`, through its shared settings validation, completion controls,
actual `WorkerGateway`, tool dispatch, trace binding and evidence settlement.
The task-local gateway and clock exist only under `cfg(test)`. This path does
not use the older `outcome_test_io` early return. The private completion adapter
passes production calls straight to Mojentic's existing recovery gateway;
policy construction is shared, including the actual operator-admission callback.
No public API, provider contract, runtime, dependency, feature or stop policy changes.

The double emits Mojentic 3.0.1 lifecycle-bearing completions and wire-observer
inputs in process. It never opens a connection or decodes provider HTTP. Opaque
`Failure` fixtures start from a rejected request preparation (no transport call),
then specify public metadata. This is consumer fixture construction, not a
Mojentic transport seam or provider verification. The registered harnesses in
`fixtures/recovery/` and `fixtures/progress/` remain unchanged and are no longer
invoked by repository tests; `tests/recovery_worker.rs` is removed.

Focused command: `cargo test --locked --lib recovery_test_io`.
All R labels below name tests in that module:

| Label | Test suffix after `public_run_` |
| --- | --- |
| R1 | `rejects_fragmented_credentials_before_tool_execution` |
| R2 | `retains_complete_lifecycle_identity_and_exact_capture_without_optional_metrics` |
| R3 | `capture_failure_rejects_completed_tools` |
| R4 | `dropped_reasoning_stream_retains_ordered_cancellation_and_capture` |
| R5 | `settlement_uses_exactly_five_seconds_of_injected_time` |
| R6 | `propagates_all_terminal_outcomes_with_full_failure_payloads_both_providers` |
| R7 | `tool_fragments_are_not_executed_and_completed_tool_executes_once` |
| R8 | `filters_credentials_in_every_semantic_and_tool_fragment_channel` |
| R9 | `admission_binds_full_failure_digest_and_attempt_identity` |
| R10 | `validates_every_recovery_setting_before_dispatch_or_trace_creation` |
| R11 | `wire_credentials_are_redacted_in_exactly_identified_captures` |
| R12 | `retains_retry_identities_ordered_lifecycle_metrics_and_both_capture_bodies` |
| R13 | `safe_prefix_mismatch_and_empty_fragments_preserve_visible_bytes` |
| R14 | `failure_without_identity_uses_history_for_settlement_and_retains_absent_evidence` |
| R15 | `preserves_permanent_status_and_partial_output_failure_evidence` |
| R16 | `safe_tool_key_boundaries_keep_truncated_fragments_inert` |
| R17 | `pending_admission_expires_on_injected_time_without_replaying` |
| R18 | `cancellation_before_terminal_delivery_prevents_completed_tool_execution` |

The following inventory characterizes every `CASES` entry, including expansion
of `CREDENTIAL_REGRESSIONS`, `BOUNDARY_CREDENTIALS` and `BOUNDARY_CONTROLS`.
The original runs each case for both provider HTTP adapters. The replacement
checks shared Microgentic consumption; R6 also selects each provider explicitly.
Provider-specific parsing, retry policy and remote behavior are explicitly
retired here, not proved by the consumer double.

| Former `worker_conformance.py` case | Replacement evidence | Ownership/disposition |
| --- | --- | --- |
| `503_success` | R12; R9 | Exact ordered lifecycle, distinct attempt identities, stable logical identity, two captured bodies and metrics retained. HTTP resend policy and identical provider-encoded retry requests retired. |
| `disabled` | R6; R10; R7 | Default no-recovery settings, exhausted error, no capture and actual tool journey retained; provider one-attempt dispatch policy retired. |
| `one_attempt_capture` | R2; R6 | Capture with recovery disabled, exact metadata/bodies, absent optional metrics and exhausted error retained; HTTP attempt selection retired. |
| `persistent504` | R6; R12 | Exhausted typed error and multi-attempt identity/accounting/capture retained; selection of three HTTP attempts and retry history construction retired. |
| `reject` | R9; R6 | Actual Microgentic operator callback rejects proof; full input/digest and typed admission-rejected error retained. |
| `plain_allow` | R9 | Unbound allow proof rejected by actual operator callback; exact callback input retained. |
| `wrong_digest` | R9 | Incorrect request digest rejected by actual operator callback; exact callback input retained. |
| `pending` | R17; R6 | Actual operator callback remains pending on FIFO; injected five-second timeout rejects, with no replay. Provider admission scheduling retired. |
| `no_admission` | R6 | Absent admission settings and full admission-required error retained; provider requirement classification retired. |
| `retry_seconds_inside` | R12; R6 | Full delay/retry lifecycle and terminal delay-ceiling/budget errors retained. Parsing Retry-After seconds/dates, eligibility and scheduling belong to Mojentic; retired here. |
| `retry_date_inside` | R12; R6 | Full delay/retry lifecycle and terminal delay-ceiling/budget errors retained. Parsing Retry-After seconds/dates, eligibility and scheduling belong to Mojentic; retired here. |
| `retry_seconds_outside_ceiling` | R12; R6 | Full delay/retry lifecycle and terminal delay-ceiling/budget errors retained. Parsing Retry-After seconds/dates, eligibility and scheduling belong to Mojentic; retired here. |
| `retry_date_outside_ceiling` | R12; R6 | Full delay/retry lifecycle and terminal delay-ceiling/budget errors retained. Parsing Retry-After seconds/dates, eligibility and scheduling belong to Mojentic; retired here. |
| `retry_seconds_outside_budget` | R12; R6 | Full delay/retry lifecycle and terminal delay-ceiling/budget errors retained. Parsing Retry-After seconds/dates, eligibility and scheduling belong to Mojentic; retired here. |
| `retry_date_outside_budget` | R12; R6 | Full delay/retry lifecycle and terminal delay-ceiling/budget errors retained. Parsing Retry-After seconds/dates, eligibility and scheduling belong to Mojentic; retired here. |
| `cancel_request` | R4; R6; R18; executable `termination` tests | Ordered failed/cancelled payloads on stream drop, cancellation token before terminal tools, settlement and process signals retained. HTTP request/admission/backoff/terminal-body cancellation checkpoint selection belongs to Mojentic; retired here. |
| `cancel_admission` | R4; R6; R18; executable `termination` tests | Ordered failed/cancelled payloads on stream drop, cancellation token before terminal tools, settlement and process signals retained. HTTP request/admission/backoff/terminal-body cancellation checkpoint selection belongs to Mojentic; retired here. |
| `cancel_backoff` | R4; R6; R18; executable `termination` tests | Ordered failed/cancelled payloads on stream drop, cancellation token before terminal tools, settlement and process signals retained. HTTP request/admission/backoff/terminal-body cancellation checkpoint selection belongs to Mojentic; retired here. |
| `cancel_terminal` | R4; R6; R18; executable `termination` tests | Ordered failed/cancelled payloads on stream drop, cancellation token before terminal tools, settlement and process signals retained. HTTP request/admission/backoff/terminal-body cancellation checkpoint selection belongs to Mojentic; retired here. |
| `cancel_terminal_body` | R4; R6; R18; executable `termination` tests | Ordered failed/cancelled payloads on stream drop, cancellation token before terminal tools, settlement and process signals retained. HTTP request/admission/backoff/terminal-body cancellation checkpoint selection belongs to Mojentic; retired here. |
| `partial_reasoning` | R4; R6; R15; R16 | Partial text/progress and interrupted error retention; incomplete tools remain inert. Provider partial-frame decoding and interruption classification retired. |
| `partial_content` | R4; R6; R15; R16 | Partial text/progress and interrupted error retention; incomplete tools remain inert. Provider partial-frame decoding and interruption classification retired. |
| `partial_tool` | R4; R6; R15; R16 | Partial text/progress and interrupted error retention; incomplete tools remain inert. Provider partial-frame decoding and interruption classification retired. |
| `keepalive` | R12; `cli::recovery_prerequisites` | Ordered normalized progress/metrics, no invented semantic progress and exact terminal accounting retained. Parsing provider keepalive frames retired. |
| `protocol` | R6; R15 | Typed failure propagation, optional evidence and partial output retained; malformed provider frame decoding/classification retired. |
| `permanent_400` | R15; R6 | Exact status, permanent classification, partial-output progress, typed failure and rendered partial text retained. HTTP status/framing/truncation classification retired. |
| `permanent_401` | R15; R6 | Exact status, permanent classification, partial-output progress, typed failure and rendered partial text retained. HTTP status/framing/truncation classification retired. |
| `permanent_403` | R15; R6 | Exact status, permanent classification, partial-output progress, typed failure and rendered partial text retained. HTTP status/framing/truncation classification retired. |
| `permanent_400_truncated` | R15; R6 | Exact status, permanent classification, partial-output progress, typed failure and rendered partial text retained. HTTP status/framing/truncation classification retired. |
| `permanent_401_truncated` | R15; R6 | Exact status, permanent classification, partial-output progress, typed failure and rendered partial text retained. HTTP status/framing/truncation classification retired. |
| `permanent_403_truncated` | R15; R6 | Exact status, permanent classification, partial-output progress, typed failure and rendered partial text retained. HTTP status/framing/truncation classification retired. |
| `credential_header` | R9 | Actual wire observer marks echoed header as identifying metadata redacted; operator callback is not launched. HTTP authentication/header validation belongs to Mojentic. |
| `credential_raw` | R11 | Fragmented raw bytes rejected and capture replaced by exact credential-withheld bytes and metadata; no completed tool execution. |
| `credential_semantic` | R1; R8 | Cross-fragment and cross-channel credentials reject public run before completed tools; trace and reconstructed renderer text remain safe. |
| `credential_error` | R11; R6 | Credential-bearing error body withheld; safe typed error serialized. Provider error-body classification is retired. |
| `capture_failure` | R3 | Destination becomes unwritable after accepted wire evidence; actual finish/settlement rejects completion, persists observer-failed outcome and blocks tools. |
| `tool_once` | R7; R12 | Actual shell marker equals exactly one execution; partial fragments never dispatch; literal call ID and complete assistant tool call preserved in follow-up request. HTTP retry placement retired. |
| `stream_drop_cap` | R4; R5 | Actual reasoning cap drops gateway stream; full ordered failed/cancelled lifecycle, exact capture, progress and next-turn completion retained. Missing terminal evidence rejects after exactly five injected seconds. |
| `empty_content_prefix_control` | R13 | Empty content fragments and prefix mismatch preserve exact visible text without contamination or retry. |
| `credential_reasoning_interleaved` | R8; R1; R11 | Gateway fragments, completed tools and renderer privacy retained; provider SSE/NDJSON decoding retired. |
| `credential_content_interleaved` | R8; R1; R11 | Gateway fragments, completed tools and renderer privacy retained; provider SSE/NDJSON decoding retired. |
| `credential_tool_arguments` | R8; R1; R11 | Gateway fragments, completed tools and renderer privacy retained; provider SSE/NDJSON decoding retired. |
| `credential_tool_name` | R8; R1; R11 | Gateway fragments, completed tools and renderer privacy retained; provider SSE/NDJSON decoding retired. |
| `credential_tool_id` | R8; R1; R11 | Gateway fragments, completed tools and renderer privacy retained; provider SSE/NDJSON decoding retired. |
| `credential_tool_completed` | R8; R1; R11 | Gateway fragments, completed tools and renderer privacy retained; provider SSE/NDJSON decoding retired. |
| `credential_tool_escaped` | R8; R1; R11 | Gateway fragments, completed tools and renderer privacy retained; provider SSE/NDJSON decoding retired. |
| `credential_tool_mixed` | R8; R1; R11 | Gateway fragments, completed tools and renderer privacy retained; provider SSE/NDJSON decoding retired. |
| `credential_tool_key_boundary_truncated` | R8; R1; R11 | Gateway fragments, completed tools and renderer privacy retained; provider SSE/NDJSON decoding retired. |
| `credential_tool_key_boundary_completed` | R8; R1; R11 | Gateway fragments, completed tools and renderer privacy retained; provider SSE/NDJSON decoding retired. |
| `credential_tool_key_boundary_escape_tail` | R8; R1; R11 | Gateway fragments, completed tools and renderer privacy retained; provider SSE/NDJSON decoding retired. |
| `tool_key_boundary_control_truncated` | R16; R7 | Safe key-boundary fragments remain inert; completed tool executes once. Provider completion-marker/parser classification retired. |
| `tool_key_boundary_control_completed` | R16; R7 | Safe key-boundary fragments remain inert; completed tool executes once. Provider completion-marker/parser classification retired. |

`validate_cli` scenarios `disabled_tuning`, `zero_attempts`, `zero_budget`,
`delay_order`, `tool_admission`, `tool_capture` and `parent_capture` map one for
one to R10: exact diagnostic, empty gateway requests and no trace creation.
`tests/microgentic_cli.rs::recovery_help_explains_wire_attempts_and_examples`
retains executable help success, empty stderr, Wire attempts and Examples.

The original assertions after `run_case` dispatch are characterized as follows:

| Meaningful original assertion family | Replacement or explicit retirement |
| --- | --- |
| No server-handler failures; exact POST route, HTTP body length/authentication; worker SHA and sanitized child environment | Mojentic/OS harness diagnostics retired. No network or old-binary equivalence is claimed. |
| Request count, identical retry bytes, stream field, nonempty tools and provider-specific control fields | HTTP resend/encoding belongs to Mojentic and is retired. R1 captures complete model/control debug fields; `cli::gateway_requests`, `cli::benchmark_continuity` and `cli::recovery_prerequisites` retain exact complete messages, descriptors and controls. R7 retains tool result wiring. |
| Exactly one trace; recovery flags/attempt count; logical IDs stable and attempt IDs distinct; wire-attempt ledger | R2/R12 compare complete ordered lifecycle payloads with literal full UUIDs and wire-attempt numbers, exact capture paths/bytes and accounting. R9 checks actual configured attempt limit in run.started; R10 rejects settings before traces. |
| Admission input has exactly failure and digest; prior identity, digest and zero observed progress; reject bare allow, wrong digest, pending and contaminated identifying metadata | R9 checks the complete independently specified input and actual callback decisions; R17 controls its pending deadline. Mojentic's decision to request admission is retired. |
| Exact final failure, permanent HTTP statuses/classification, meaningful observed partial progress | R6 compares full typed errors and trace payloads for all ten outcomes and both providers. R15 retains exact 400/401/403, observed/delivered bytes and tool progress; R14 retains absent identity/status/phase/metrics and history fallback. Provider classification is an input, not a verified result. |
| One attempt-failed before cancelled, no success for dropped attempt; terminal raw byte count, no partial/contaminated tool dispatch | R4 compares the entire lifecycle sequence, full progress and both distinct request identities. R18 covers cancellation after provider success before queued terminal tools are consumed. R1/R3/R7/R8/R16 check real marker-file effects and inert fragments. Executable signal/process-group tests remain. |
| DONE, one or two successful completions, request depths 0/1, native reasoning, provider metrics eval 7/prompt 11/TPS 7.0 | R2/R4/R7/R12 assert actual final summaries; R12 checks final metrics once with exact duration/counts and no duplicate raw metrics. Existing CLI request/prerequisite tests retain ordered progress, native history and depths. |
| Renderer succeeds, Request recovery cards, partial content/reasoning, no reconstructed secret across text/tool fields, safe prefix mismatch | R8 renders actual caller traces and reconstructs complete semantic text; R15 checks exact rendered partial text. R13 checks exact safe visible text; existing ordinary-noop/UTF-8 filter tests remain supplemental. |
| Credential-free stdout/stderr/artifacts, no replay/admission after contamination, meaningful progress for contaminated fields | Public errors, actual trace/renderer text, inert tool effects and one incoming completion are checked in R1/R8/R11. R9 blocks admission on contaminated headers. R4/R15 retain observed progress. Raw provider observation construction is retired; `cli::recovery_prerequisites` retains captured CLI output privacy. |
| Completed captures count, exact/redacted flags, original request SHA, exact request/response bytes, cancellation prefix length, no pending files | R2/R12 assert complete literal capture identities, request/response bytes and exact metadata. R11 asserts exact withheld bytes, hash and redacted/nonexact metadata. R4 asserts exact partial capture bytes matching terminal progress. R3 tests actual persistence failure. |

No cross-repository boundary changed. The consumer fixtures are not a
provider-verified Mojentic contract set. Mojentic provider verification is
outside this repository and is not claimed. Retained executable contracts are
`microgentic_benchmark_continuity.v1`, SHA-256
`510915b0d21f86f7263ff5c29399f6a0fd358fb00ac72af2ce5771df96eb31af`,
and `microgentic_terminal_outcome_contract.v1`, SHA-256
`6c8afe8fe658944da93d4b8f582d74e20a16d8ef59b1dfa3f128592df1183f37`.
The first runs in the Rust gate; the second is verified by its Rust producer
and Python consumer tests in this same repository. Both sides are tested on the
same working revision, based on delivered trunk
`19d98ce2789f6918947868e8587a3614e35d83f9`; Foundry owns committing the changes.
These unchanged local contracts do not verify the Mojentic dependency.

The hardest acceptance probe disables only WorkerGateway's content credential
filter, observes a completed shell tool write its marker (rejection), then
restores the filter and observes the same public run reject before execution.
The five-second evidence drain and operator wait use a task-local manual clock;
production keeps its existing Tokio timeout and durations.

## Remaining timing and I/O audit

No suite here is designated integration or end-to-end. Cargo's `tests/` layout
is not permission to use real network infrastructure. The required Rust and
benchmark Python test sources were searched for sockets/servers, real sleeps
and elapsed-time assertions. Registered experiment fixtures are preserved and
excluded from repository test invocation.

| Existing test | Retained deterministic evidence |
| --- | --- |
| `tests/microgentic_cli.rs` nonterminal launch | Actual executable completes with status 1, empty stdout and run diagnostic; no host-speed threshold. |
| `tools::tests::timed_out_{declared_probe,shell_command}_kills_descendant_process_group` | Descendant FIFO readiness precedes manual 999 ms + 1 ms advancement. Exact one-second diagnostic, released-survivor marker absence and writer EOF prove group cleanup; no sleep. |
| `tui::input::tests::an_empty_pipe_is_idle_until_the_timeout` | Injected monotonic instants and poll gateway require 30 ms, then 18 ms after interruption; real empty pipe remains idle under a zero-duration readiness check. |
| `trace::tests::runs_at_the_same_instant_get_distinct_files_in_order` | Fixed timestamp, twelve unique files and exact creation/name order; no throughput threshold. |
| Executable signal, TUI and FIFO support | Real signal/group delivery, descriptor readiness, FIFO gates and terminal restoration retained. `tokio::net::unix::pipe` in the two test providers is Tokio's FIFO API, not a socket; explicit FifoReceiver/FifoSender aliases identify it. Existing condition-variable deadlines are missing-event watchdogs, not behavioral elapsed-time criteria. |

The TUI evidence inventory's shell example now names
`read -r release < tool-control`, matching the actual synchronized journey.
Production monotonic clocks, signal grace periods and observational command
strings are not test sleeps. Disabling only the process-group kill makes both
descendant timeout tests reject the surviving process marker; restoring SIGKILL
retains the same checks. No new dependency or Tokio feature is needed.

## Recovery prerequisite characterization

This inventory was made before replacing `tests/recovery_prerequisites.rs`.
The replacement lives in `src/cli/recovery_prerequisites.rs` under `cfg(test)`.

| Original evidence | Disposition |
| --- | --- |
| `reviewed_recovery_stream_preserves_worker_frame_progress`: clean child environment; HTTP POST route, body length and framing; three identical encoded requests | Retire provider-owned transport diagnostics under Stacey's 2026-10-18 removal of real-connection tests. |
| Legacy stream: four progress frames, initial zero content/thinking/tools and not done; one metrics frame with eval count 7 and 7 tokens/s | Preserve Microgentic's consumption of independently specified progress and metrics through the CLI gateway replacement. |
| Raw recovery: stable identity and wire attempt 1 on progress/metrics/completion; exact reasoning and DONE; no tools or failure; raw byte count; terminal usage 7/11 and eval duration 1,000,000,000 ns; ordered progress/progress/reasoning/progress/content/progress/metrics/completed | Provider parsing belongs to Mojentic. Microgentic identity, byte accounting, terminal evidence and recovery event consumption are retained by the recovery caller tests above; normalized telemetry remains in this prerequisite suite. |
| Raw and convenience progress/metrics equal legacy after discarding client timing; persisted response/request bodies and assertions identifying reviewed revision | Retire cross-adapter comparisons and diagnostic artifacts. A scripted gateway does not verify Mojentic, recovery conformance, HTTP encoding or stream decoding. |
| `actual_worker_provider_paths_preserve_progress_prerequisites`: Python subprocess succeeds; both provider cases complete | Replace with separate Ollama and oMLX cases through the production CLI parser/dispatcher and public agent API. |
| Python handler: expected route, oMLX Bearer credential, no handler failures; subprocess exit 0; exactly one request; DONE summary; exactly one trace | Preserve dispatcher success, complete single gateway request, DONE and single trace. Route/authentication and process/environment plumbing are provider/OS diagnostics, retired. |
| Python artifact loop: secret absent from stdout, stderr, trace, response and every request | Preserve absence from captured stdout/stderr, trace, gateway input and scripted response artifacts. No HTTP headers or environment are serialized by the replacement. |
| Python reasoning/tools/controls: native thinking event, stream true, nonempty tools, interaction limit 1; Ollama think true and num_predict 8192; oMLX max_tokens 8192, enable_thinking true, preserve_thinking true | Preserve native reasoning, limit and actual interaction count 1, complete tool descriptors and completion controls; verify Microgentic's provider-control trace. HTTP stream and field encoding remain Mojentic-owned. |
| Python Ollama telemetry: four progress payloads, first content/thinking/tools all zero; one metrics payload, eval 7, prompt 11, throughput 7.0 | Preserve full ordered progress payloads and provider metrics with independent literal expectations, including durations and optional fields. |
| Python response/request/stdout/stderr/assertion files and worker executable SHA-256 | Use temporary CLI artifacts and Rust assertions; no frozen binary provenance claim. Registered Python harness and its inputs remain unchanged. |

The worker matrix is replaced by the recovery caller tests above. No
application/repository boundary changes or Mojentic provider verification are
claimed by these consumer tests.

Replacement evidence (`cargo test --locked --lib cli::recovery_prerequisites`):

| Replacement case | Independently specified evidence |
| --- | --- |
| `ollama_baseline_preserves_ordered_progress_and_metrics` | One complete request for `synthetic-model`, all tool descriptors, literal system/user messages for `Finish with DONE.`, temperature 1.0, context 32768, output limit 8192, enabled reasoning, preserved reasoning, interaction limit 1; every other completion option specified. One actual interaction and one trace, final summary DONE. Provider controls: `think: true`, `num_predict: 8192`, `num_ctx: 32768`. |
| Same Ollama case, ordered telemetry | Progress frames 1–4: `(content_chars, thinking_chars, done)` = `(0,0,false)`, `(0,20,false)`, `(4,0,false)`, `(0,0,true)`; both tool counts zero throughout. States WaitingForFirstToken, Generating, Generating, DoneFrame; cumulative and per-call frame counts 1–4. Event order: progress, progress, thinking, progress, content (`agent.stream.chunk`), progress, metrics. Native reasoning preview is `Synthetic reasoning.` (20 chars). |
| Same Ollama case, metrics and throughput | Exact metrics payload: prompt count 11, eval count 7, total/load/prompt/eval durations 2,000,000,000 / 100,000,000 / 400,000,000 / 1,000,000,000 ns, throughput 7.0; turn 1, depth 0, thinking 20, packet multi-file-edit, expected output 4096, context band unknown. The throughput trace and single persisted sample retain independently expected provider/model/count/duration/budget values. Timestamp and host identity are not fixed; persisted sample must equal its trace record. |
| `omlx_baseline_preserves_native_reasoning_and_controls` | Same complete request, DONE, one interaction/trace and native reasoning; provider controls `max_tokens: 8192`, `enable_thinking: true`, absent named effort (`null`), `chat_template_kwargs: {preserve_thinking: true}`. Thinking precedes content; no progress/metrics fabricated for this provider and no throughput file. |
| Both cases, credential boundary | A task-scoped credential source supplies a private canary without process environment mutation. Stdout, stderr, trace, scripted response and complete captured request exclude it. The seam and both cases are `cfg(test)` only. This proves credential-free Microgentic artifacts at this boundary, not HTTP authentication. |

The complete messages and descriptors reuse the existing versioned
`tests/fixtures/benchmark_continuity.json` request specification
(`microgentic_benchmark_continuity.v1`), without changing that fixture or its
expectations. Provider-control trace version is `provider_request.v1`.
The gateway replies are consumer inputs, not provider-verified contracts.
There is no changed external boundary and no new paired contract verification
claim. The recovery caller tests above retain Microgentic identity, byte
accounting, terminal evidence and optional fields. Provider decoding and
raw/convenience/legacy equivalence remain outside this suite. The registered
conformance harness files are unchanged.

## Gateway request migration

`src/cli/gateway_requests.rs` drives the real argument parser, dispatcher and
public `run_agent` through the existing task-scoped `LlmGateway` double.
`src/semantic_advisory.rs` exercises public `request_semantic_advisory` with its
in-process structured-completion gateway. Neither replacement opens sockets,
starts a provider, invokes Python, sleeps, or asserts wall-clock timing. The
remaining `tests/omlx_gateway.rs` case is intentionally socket-free serialization
coverage. The private server, HTTP reader, metadata polling, response encoders
and process launch helpers from that file have been removed; shared support in
`tests/support/fixtures.rs` remains in use; the TUI server and old terminal
driver have now been removed (see the TUI characterization below).

Characterization of every case remaining before this migration:

| Original `tests/omlx_gateway.rs` case | Preserved assertions and replacement evidence | Transport-only assertions retired |
| --- | --- | --- |
| `microgentic_and_legacy_entry_points_preserve_exact_provider_requests` | `modern_and_legacy_contract_requests_are_complete_and_equal`: both providers, malformed personal config ignored, Unicode guidance, explicit workspace, DONE, modern/legacy token and interaction aliases. Both complete requests match independent expectations, including literal `inspect-1` on assistant and tool result, then match across dispatch paths. Existing `benchmark_continuity` also fixes complete requests, packets and ledgers for contract/goal/sequential modes. | Executable startup and HTTP route/auth/framing; no claim of testing an old external executable. |
| `saved_defaults_and_stdin_prompt_reach_the_public_worker_without_stdout_noise` | `saved_setup_and_unicode_stdin_keep_stdout_and_workspace_clean`: real setup persists provider/model/effort; saved-default/stdin and explicit-argument runs each independently match a complete expected request, including literal conversational system guidance, multiline Unicode input, all descriptors (including calculate), model, controls and optional-field presence/absence. Exact plain stdout, metadata model, two traces and empty workspace remain checked for both providers. | OS stdin pipe plumbing and HTTP metadata/chat exchange. Intake is task-scoped at the I/O boundary. |
| `built_worker_preserves_entry_stash_and_rejects_new_stash_both_providers` | `entry_stash_is_preserved_and_new_stash_rejects_done_both_providers`: real dispatcher, Git inventory and shell tool; complete expected sequences for the preserved-entry case (one request) and new-stash case (initial, tool continuation, repair). Exact entry inventory, literal `stash-tool` IDs in assistant and tool messages, full serialized shell result, DONE history and repair text are specified independently. Original object IDs, final summary and rejection trace remain checked for both providers. Existing `run_outcomes` separately fixes withheld-stash outcome/exit policy. | Provider encoding of the shell tool response. |
| `worker_effort_and_token_budgets_reach_omlx_on_the_wire` | `worker_efforts_and_both_token_flags_reach_the_gateway`: absent, disabled, low, medium, high, xhigh and enabled effort; both output-token aliases; exact complete requests including every completion control and descriptor, model, temperature and context. Trace packets separately fix provider-control mappings and exclude `test-key`. | HTTP `stream`, oMLX field placement and omission of Ollama fields; Mojentic owns those translations. |
| `omlx_tool_results_survive_but_native_reasoning_is_only_traced` | `literal_ids_and_reasoning_survive_tools_and_outer_turns`: literal `inspect-1`, exact tool result, reasoning absent by default from complete subsequent requests and retained in trace. Covers both token aliases in the controls test. | SSE parsing and wire `tool_call_id` spelling. Normalized gateway messages carry the literal ID in `tool_calls`. |
| `ollama_named_efforts_reach_the_wire_without_collapsing` | Direct Mojentic-only test deleted under the mission instruction. The worker effort test independently retains Microgentic's exact named/boolean/absent effort choices for both providers at its gateway boundary. | Entire direct `OllamaGateway::complete` HTTP test: no Microgentic behavior exercised. No fake is claimed to verify Mojentic's HTTP implementation. |
| `native_reasoning_history_and_xhigh_round_trip` | Retained unchanged in `tests/omlx_gateway.rs`: native reasoning serialization, absence of `thinking`, xhigh deserialization. | None. |
| `structured_advisories_use_omlx_output_budgets` | `semantic_advisory::tests::structured_advisory_preserves_complete_request_and_response`: public API, complete model/messages/schema/completion captured at gateway, 256 output tokens, 0.2 temperature, no effort/profile, zero tool iterations, accepted proposal returned. Existing advisory tests retain input-limit rejection before invocation and trace isolation. | oMLX JSON-schema HTTP encoding, wire-only field names/omissions. The gateway's structured method has no tools parameter. |
| `native_reasoning_survives_tools_outer_turns_and_context_accounting` | `literal_ids_and_reasoning_survive_tools_and_outer_turns`: three complete independently specified requests, both providers, full transcript, retention off/on with and without qwen3.8 profile, two distinct reasoning sentinels and no aggregate assistant duplicate, exact reasoning character accounting in tool and outer-turn ledgers, preserve-thinking trace controls. | SSE reasoning delta decoding and provider-specific history field naming. |
| `qwen_profile_rejects_high_before_connecting` | `qwen_profile_rejects_high_before_gateway_invocation`: actual dispatch returns unsupported-profile error for both providers, with no gateway request or metadata discovery. The driver returns the real error rather than fabricating an outcome/status. | Connection-refused sentinel address and subprocess error printing. |
| `ollama_worker_preserves_native_reasoning_and_records_provider_controls` | Same full three-request reasoning matrix and effort test retain off/on behavior, exact model/context/output/temperature/effort, literal tool IDs and separate Ollama trace-control assertions. | NDJSON decoding, `thinking` wire name and absence of oMLX-only wire fields. |

The already landed `cli::capped_reasoning` coverage below remains unchanged.
Expected messages and tool descriptors reuse
`tests/fixtures/benchmark_continuity.json` (`microgentic_benchmark_continuity.v1`,
SHA-256 `510915b0d21f86f7263ff5c29399f6a0fd358fb00ac72af2ce5771df96eb31af`).
Additional expected IDs, empty-tree and shell results, continuation/repair text,
controls and optional native reasoning fields are literal test inputs. Everyday
system guidance and the additional calculate descriptor are literal specifications
in `gateway_requests.rs`, independent of production builders. Stash expectations
derive only repository paths and object IDs from separate Git queries; they never
use runtime inventory/message builders or captured requests. Expectations are
never copied from captures or masked. Complete gateway requests, rather than trace
snapshots, establish request behavior. Traces additionally establish observability
and context accounting. Sensitivity probes reject independently introduced
conversational system-message and calculate-descriptor regressions, a changed
scripted stash-tool ID, and separate corruption of assistant and tool stash IDs.
Restoring each input or
implementation passes the focused suite. This proves request-oracle sensitivity
at the in-process gateway boundary; it does not verify HTTP encoding.

Focused verification: `cargo test --locked --lib cli::gateway_requests`,
`cargo test --locked --lib semantic_advisory::tests`,
`cargo test --locked --lib cli::capped_reasoning`,
`cargo test --locked --lib cli::benchmark_continuity`, and
`cargo test --locked --test omlx_gateway`. The full required test gate runs all
of these. Run the gateway focus with `OMLX_API_KEY=test-key` to additionally check
credential exclusion with the original sentinel configured in the environment.

These are internal Microgentic regression specifications, not a new or changed
cross-repository executable contract. Mojentic remains pinned by `Cargo.lock`
at 3.0.1; no dependency sources or external repositories are changed. External
provider verification is **not available here**: this suite does not verify
Mojentic's HTTP serialization, auth, SSE/NDJSON decoding or provider-server
behavior. Coordinating a shared versioned executable contract and provider-side
verification in Mojentic remains necessary before claiming HTTP compatibility.
The existing terminal-outcome cross-language contract and its Rust/Python gates
remain unchanged.

## Capped reasoning

`src/cli/capped_reasoning.rs` replaces only
`tests/omlx_gateway.rs::capped_partial_reasoning_is_not_retained_as_completed_native_history`.
The test parses the compatibility executable's `run --experiment` arguments and
uses the real dispatcher and public `run_agent` with the existing task-scoped
`LlmGateway` double. It opens no sockets and starts no subprocess or provider.

| Former evidence | Replacement evidence |
| --- | --- |
| SSE reasoning delta containing `PARTIAL_REASONING_SENTINEL`, followed by a DONE reply | Gateway `Thinking` chunk with the identical text, followed by `Content("DONE")`; HTTP decoding is outside this runtime test's boundary. |
| `--preserve-reasoning --max-thinking-only-tokens 1`, medium effort, 8192 output tokens, three iterations | Identical CLI controls through the dispatcher; summary also confirms reasoning preservation is enabled. |
| Two HTTP requests, sentinel absent from the second request's messages | Two captured complete gateway requests; sentinel absent from the entire second request, including native history. |
| Trace retains sentinel and `llm.thinking_only_stream.action_transitioned` | Both trace assertions retained against the actual run trace. |
| Successful subprocess exit and DONE line | Successful dispatcher exit and exact `DONE` final summary. |

Run `cargo test --locked --lib cli::capped_reasoning`; the required all-targets
test gate includes it. The acceptance probe rejected the same stream with a cap
of 8192 because completed reasoning entered the second request, then passed with
cap 1. This characterizes runtime interruption and history retention, without
claiming to verify Mojentic's HTTP transport. The remaining `omlx_gateway` case tests serialization without connections;
the obsolete server helpers are gone. No external application contract changed.

`src/cli/run_outcomes.rs` replaces the former `tests/run_outcomes.rs` integration
suite. All 13 test names are retained under `cli::run_outcomes`. They parse real
CLI arguments, run the same dispatcher as `cli::run`, call public `run_agent`,
and capture its actual output writers and returned `ExitCode`. They read terminal
JSONL traces written by the agent. No test constructs an agent summary or calls
an outcome classifier as a substitute for execution.

| Former behavior (test name stem) | Replacement evidence |
| --- | --- |
| `a_completed_reply_exits_zero_with_a_replied_outcome_{omlx,ollama}` | Everyday prompt: JSON outcome and terminal trace, plain reply, no diagnostic, exit 0; metadata lookup hits the double. |
| `an_accepted_done_exits_zero_with_a_done_outcome_{omlx,ollama}` | Task DONE: contract outcome and terminal trace, no diagnostic, exit 0; legacy JSON and exit 0. |
| `a_fail_reply_exits_three_except_in_benchmark_and_compatibility_modes_{omlx,ollama}` | FAIL: exact plain and JSON output, evidence path and diagnostic, exit 3; benchmark and legacy retain JSON and exit 0 without the public diagnostic. |
| `an_unvalidated_edit_before_done_exits_four_as_withheld_by_validation_{omlx,ollama}` | Real file edit before DONE: validation withholding, exact detail and diagnostic, terminal trace, exit 4. |
| `a_new_stash_before_done_exits_four_as_withheld_by_the_stash_gate` | Real Git stash: repeated DONE withheld, exact request count, stash identity detail, terminal trace, exit 4; stash remains. |
| `an_unrun_probe_before_done_exits_four_as_withheld_by_the_probe_gate` | Explicit contract with an unrun probe: named missing probe, withholding trace, exit 4. |
| `an_exhausted_iteration_budget_exits_five_{omlx,ollama}` | Actual list/read tools and replies: two-turn budget, exact call count, stop detail and diagnostic, terminal trace, exit 5. |
| `repeated_empty_responses_exit_five_as_an_empty_response_hard_stop` | Three empty gateway streams: exact call count and stop detail, terminal trace, exit 5. |

`src/outcome_test_io.rs` implements the Mojentic `LlmGateway` boundary. The last
scripted stream repeats, matching the former mock server. Tool calls run through
the real tool dispatch. Expected terminal kinds and identifying outcome fields
come from `benchmarks/fixtures/terminal_outcome_contract.json`, version
`microgentic_terminal_outcome_contract.v1`: `done`, `failed`,
`withheld_validation`, `withheld_stash`, `withheld_probes`, `iteration_budget`,
and `empty_response`. The Python consumer verifies the same cases and labels;
its existing tests retain old untyped traces, optional detail/reason fields,
unknown outcomes and error handling.

The gateway is installed with a Tokio task-local scope, including metadata
discovery. Unexpected advisory, listing and embedding requests fail in the
double before any transport. Stdin defaults to empty and can be supplied per
execution; state lives in the temporary workspace. Metadata answers are normalized
`ModelContextWindow` values. The driver captures model names, actual messages,
complete tool descriptors, completion controls and metadata model arguments. The test I/O scope supplies the tool root after CLI path validation, so tests
never change the process working directory or environment variables.
Streams are immediately ready. There are no sockets, servers, Python harnesses,
sleeps, or assertions about elapsed time in this suite.

All injection sites and the CLI test modules are guarded by `cfg(test)`. Release
builds cannot select the double through flags, configuration or environment.
The production dispatcher only gains output writers; runtime policy, provider
requests and public API signatures remain unchanged.

Run producer verification with `cargo test --locked --lib cli::run_outcomes`
and consumer verification with
`python3 -m unittest discover -s benchmarks -p 'test_terminal_outcome.py'`.
Both are included in the required gates (the producer through the all-targets
Rust test gate). Record the contract SHA-256 and tested source revision alongside
the gate results. Foundry tasks report the base commit plus their uncommitted
source changes until Foundry finalizes them.

`src/cli/run_report.rs` replaces `tests/run_report.rs`. All ten original case
names are retained under `cli::run_report`. Before migration, the cases below
were characterized by their assertions; the replacement retains those assertions
and adds direct filesystem evidence. Both suites share
`src/cli/test_support.rs`, extracted from the landed outcome driver without
changing its execution path. The report suite uses actual CLI argument parsing,
dispatch, tool handling, output writers and returned exit codes. No report or
summary is fabricated, and no renderer is called directly.

| Former behavior (test name stem) | Replacement evidence |
| --- | --- |
| `an_edit_then_a_check_reports_the_change_and_the_command_{omlx,ollama}` | Actual edit and grep; exact `changes` and `checks` objects including null fields; exit 0; JSON has no plain report; exact plain reply and stderr report. Edited file contains the replacement text. |
| `edits_already_in_the_working_tree_are_flagged_and_untouched_ones_are_not_changes` | Temporary Git baseline, tracked dirty `a.txt` and untracked `scratch.txt`; real edit and resulting contents; exact pre-run dirt list; only `a.txt` reported, with attribution; exact plain report. |
| `files_a_shell_command_creates_and_deletes_are_reported` | Real shell creates `new.txt` and removes `old.txt`; exact sorted change records and observational command/exit 0. Contents, deletion and untouched `keep.txt` checked on disk. |
| `declared_probes_report_which_passed_and_which_did_not_run` | Actual `execute_probe`: passed command and unrun command with null exit; exact checks object and no changes; exit 4, probe gate; plain report precedes withholding diagnostic. |
| `a_source_edit_without_a_passing_check_is_unchecked` | Real source edit and successful observational grep; edited source read from disk; exact changed path and unchecked path; plain unchecked warning survives successful command. |
| `a_plain_reply_reports_nothing_{omlx,ollama}` | Exact empty file/check arrays and complete snapshot; JSON exit 0; plain reply with no report. |
| `a_trace_directory_inside_the_project_is_not_a_change` | Nested and project-root trace directories; actual trace exists under selected path; actual edited file contents and exact singleton change list exclude trace files. |
| `an_unrun_file_assertion_probe_is_reported_as_not_run` | Passed command probe and unrun `file_text_equals` probe; exact declared records including null exit; JSON exit 4 and exact plain report. |

Run report verification with `cargo test --locked --lib cli::run_report`.
The all-targets Rust test gate runs both migrated suites. The gateway double,
state/intake scope and filesystem fixtures are shared; neither migrated suite
opens sockets, starts servers or Python harnesses, sleeps, or measures wall time.
Provider selection coverage is retained for both oMLX and Ollama; these are
gateway conformance tests, not HTTP wire-format tests. The provider transport
and cross-repository executable contracts are unchanged by this migration.
Only `tests/support/fixtures.rs` retains callers, including the unchanged
termination suite; the obsolete server, terminal driver and module were removed.

`src/cli/everyday_runs.rs` replaces all 19 cases from `tests/everyday_runs.rs`.
The following characterization was made from the original assertions before
migration. All original test names are retained under `cli::everyday_runs`.
Each case parses CLI arguments and dispatches through the same entry as the
public CLI, reaching public `run_agent` (or the conversation worker entry).
The shared driver supplies stdin and gateways without changing process cwd or
environment. User guidance lives beside the explicitly selected temporary config;
its complete absolute identity is asserted, rather than masked or counted alone.

| Former behavior (test name stem) | Replacement evidence |
| --- | --- |
| `calculate_answers_batches_and_stays_out_of_benchmark_runs_{omlx,ollama}` | Exact calculation batch values, malformed bracket error with columns, mixed successful/error batch, and tool-result success sequence in the real trace. Captured calculator descriptor includes expression type and required field. Exact final reply; benchmark captured descriptors exclude calculate. |
| `project_guidance_reaches_everyday_runs_and_conversations_{omlx,ollama}` | Actual system/user messages: ordered user/root guidance, pending nested index, nested content delivered after a real file read and only once, and exact candidate paths/statuses/hash presence. Exact residency activation identities, nested reference path/via/turn, pinned root/tool, current prompt and standing-context accounting. Guidance-disabled request keeps identical full tool descriptors and has no guidance trace. Two CLI worker exchanges use the same on-disk session, retain the first assistant response, and deliver changed root guidance plus the update notice on the second request. Real session history and unchanged notes checked on disk. |
| `a_plain_question_takes_one_request_{omlx,ollama}` | Both explicit prompt and stdin: literal captured user message, everyday system prompt, available read tool, exact plain stdout (leading blank stream chunks removed), no task continuation. |
| `a_question_needing_one_tool_takes_two_requests_{omlx,ollama}` | Real read_file result containing the file's pace reaches the next captured request; exact plain reply and no task instructions. |
| `a_work_request_edits_checks_and_reports_naturally_{omlx,ollama}` | Real edit_file and shell grep; file contains the new greeting, check output reaches the next request, exact plain reply and everyday request policy. |
| `a_review_keeps_stdin_as_reference_material_{omlx,ollama}` | Exact labelled reference message from stdin, exact plain reply, no adapter probe, and fenced shell command leaves no file behind. |
| `the_legacy_binary_keeps_the_task_path_for_a_plain_prompt_{omlx,ollama}` | Compatibility entry: actual first task-template message, later continuation message, two allowed iterations and captured tool set without calculate. |
| `an_everyday_run_discovers_the_context_window_{omlx,ollama}` | Captured metadata calls name test-model; absent metadata yields unknown window and discovery error. Normalized provider window produces exact source/field/tokens, Ollama requested-window cap and advertised capacity, and real ledger accounting. Entire captured requests (messages, descriptors, model and all completion controls) stay equal with/without metadata; explicit assertions retain num_ctx 32768, max_tokens 16384 and temperature 1. Gateway timing metrics reach the real trace with exact first-token kind and numeric timing fields. |
| `an_ollama_num_ctx_parameter_is_the_discovered_window` | Normalized gateway metadata with model_num_ctx alone or alongside a larger maximum: actual CLI runs account against 16384, retain parameters.num_ctx as source field, and omit advertised_tokens. This verifies Microgentic's precedence after normalization. |
| `residency_settings_come_from_flags_then_config_then_defaults` | Actual CLI runs with explicit config, individual flag overrides and missing config; exact residency limits/fractions and explicit-setting trace presence agree with the ledger. |
| `benchmark_runs_record_explicit_residency_settings_only` | Explicit setting recorded only when passed; ledger has neither residency nor residency_settings, and no metadata calls occur. Personal config settings/model, user/project guidance and nonempty stdin are supplied and excluded from captured benchmark requests; calculate stays excluded. |

Run this suite with `cargo test --locked --lib cli::everyday_runs`.
The initial guidance-reload acceptance probe rejects the old test injection,
which dropped conversation state, and passes when the `cfg(test)` entry forwards
that state into the existing gateway execution path. Production behavior is frozen.
The landed outcome and report suites still share the same driver and fixtures,
and remain part of the all-targets gate.

Requests here are snapshots at `LlmGateway`, not fabricated HTTP bodies.
Completion controls include the full `CompletionConfig` debug representation
(for fields without serde) plus directly asserted scalar controls. Metadata and
`StreamMetrics` fixtures are normalized gateway inputs: these tests prove
Microgentic consumes and traces them, not that Mojentic parses HTTP correctly.
Removed server-only assertions are the exact `/v1/models` GET and `/api/show`
JSON body, and parsing of Ollama's textual `parameters.num_ctx` alongside
`model_info` capacity. Those belong to Mojentic's transport coverage; this fake
provides no evidence about that parsing. SSE/NDJSON parsing and client timing
measurement are likewise outside the replacement evidence. The exclusively
unused server num_ctx setter and plumbing were removed; shared window/listing,
fixtures retain their callers. The TUI migration below replaces the remaining
server and terminal support.

No application/repository boundary, contract version, dependency, feature or
public API changes. The existing executable outcome contract remains
`microgentic_terminal_outcome_contract.v1`, SHA-256
`6c8afe8fe658944da93d4b8f582d74e20a16d8ef59b1dfa3f128592df1183f37`.
Both the Rust producer suite and Python consumer suite verify it in this
repository's required gates. No external provider verification is claimed.

The TUI migration below completes removal of that server and terminal support.
Registered Python recovery/progress harnesses and experiment inputs are unchanged.

## Benchmark continuity

`src/cli/benchmark_continuity.rs` replaces `tests/benchmark_continuity.rs`.
All six permanent test names are retained under `cli::benchmark_continuity`.
Each provider (oMLX and Ollama) runs contract, goal, and sequential cases through
real argument parsing and CLI dispatch into public `run_agent`/`run_sequential`.
Both the normal executable's historical top-level verbs and the compatibility
executable are compared with `benchmark run` or `benchmark run-sequential`.
No socket, server, child binary, Python harness, sleep, process environment/cwd
mutation or wall-clock assertion is used by this suite.

The original cases and ignored comparison were characterized before migration:

| Original assertion / guarantee | Executable replacement |
| --- | --- |
| Contract/goal/sequential × two providers (six permanent tests): full HTTP JSON equality between legacy and benchmark commands | Same named cases compare **complete actual gateway inputs**, independently captured by `OutcomeIo::complete_stream`, across both dispatch paths and the compatibility executable. Each input also equals a fixed expectation, including exact Unicode messages, `inspection` assistant and tool-result IDs, arguments, descriptors, provider default model, and every completion control. |
| Full assembled packets and context ledgers equal | Complete values equal across dispatches **and** fixed expectations for each case/provider/call; packet content is checked separately from captured gateway requests. |
| Ambient invalid personal config, nonempty stdin, project AGENTS.md, and nonexistent unused goal must not affect benchmark requests | Actual CLI receives each input; the contract wins over the unused goal. Invalid config cannot be loaded successfully. Complete fixed requests exclude ambient input and project guidance (the list_tree result still legitimately lists AGENTS.md). No personal state directory exists. |
| No session events, discovered-window/residency trace events, or residency/known_window ledger fields | Same trace/ledger exclusions, fixed full ledgers, and zero captured metadata-discovery calls. |
| Sequential attempts share a trace directory but never a trace file | Exact path distinctness and common parent, without timing assumptions. Each real trace is read. |
| Sequential fresh context: first two requests equal the next two | Same whole-request assertion plus four fixed request/packet/ledger expectations; the second attempt restarts with the initial messages and no earlier assistant/tool history. |
| Summary keys cannot shrink; only outcome/changes/checks may be added | Fixed required legacy summary keys, allowing only those three additive keys; sequential summary keys are independently fixed. Final text is exactly `Inspecting.\nDONE`. |
| Sequential final_summary, accepted_done, independent_exact, terminal_error, pre/post artifact, artifact_unchanged agree; stop reason and confirmed success fixed | Original comparisons retained. Fixed expectations additionally check every deterministic attempt field: ordinal, unchanged streak, repair counters, probes, explicit failure, observed tokens and outcome. Runtime values are not asserted; trace files are verified separately. |
| Ignored historical comparison's benchmark mode: all six cases, request/packet/ledger equality, additive summary compatibility, attempt fields | Permanent cases plus fixed expectations retain these Microgentic guarantees at the gateway boundary. Arbitrary old executables are no longer run or claimed verified. |
| Ignored historical comparison's conversation mode: contract `run --prompt` isolation for both providers | Now permanent `contract_conversation_isolation_both_providers`, with full fixed request/packet/ledger assertions, metadata/state/trace exclusions and compatibility-executable dispatch. This is task isolation when a contract/experiment is supplied, not an everyday conversation session. |
| Old driver probes --help for --json, selects binaries via CONTINUITY_* variables and prints binary SHA-256/report schema | Retired with the historical driver. These identify external executable comparisons, not runtime compatibility behavior; there is no replacement historical-binary report or claim of verifying an unavailable old revision. |
| Provider-specific HTTP serialization, SSE/NDJSON stream parsing and encoding of tool IDs | Transport-only coverage retired from this suite. Actual normalized LlmGateway messages, IDs, descriptors and completion controls are retained exactly. Mojentic owns HTTP serialization/parsing; no external Mojentic verification is claimed. TUI and termination now use socket-free gateway doubles as described below. |

Fixed expectations live in `tests/fixtures/benchmark_continuity.json`, version
`microgentic_benchmark_continuity.v1`, SHA-256
`510915b0d21f86f7263ff5c29399f6a0fd358fb00ac72af2ce5771df96eb31af`.
Shared descriptors and completion controls
are stored once and expanded into complete requests; contract and goal messages
are literal, provider model names and packet controls are explicit. No value is
masked or dynamically copied from a captured request. Sequential expectations
repeat the contract's two exact inputs. The fixture was checked against every
case/provider capture through the dispatcher; traces never substitute for
requests observed at the gateway. Update expectations only after reviewing the
entire behavior change, not by automatically accepting a new capture.

Run `cargo test --locked --lib cli::benchmark_continuity`; it is included in
`cargo test --locked --all-targets --all-features`, the repository's required gate.
The acceptance probe first rejected a helper replacing `inspection` with
`call-list_tree`, then passed with the explicit ID preserved. Existing callers
of `tool()` retain their `call-{name}` IDs. Only the unused server-side
`last_tool_result` and `tool_names` definitions were removed; their live
replacements remain in `src/cli/everyday_runs.rs`.

The module registration and driver are `cfg(test)`, as is `OutcomeIo` in
`src/lib.rs`. Sequential stdout capture and retry worker-root injection also use
`cfg(test)`; release builds keep the original println and cwd path. No runtime,
dependency, edition, MSRV, feature or public API behavior changes are introduced.
This fixture is an internal regression specification, not a changed contract
between repositories. The existing terminal-outcome contract and its Rust/Python
producer/consumer verification remain unchanged and run in the required gates.


## Real-process termination

`tests/termination.rs` retains all six CLI process cases. Run them with
`cargo test --locked --test termination --features termination-test-provider`.
The required `cargo test --locked --all-targets --all-features` gate includes
all six. Without the nondefault feature this test target has no cases.

| Original case | Replacement evidence |
| --- | --- |
| SIGTERM during oMLX streaming | A real `microgentic run --provider omlx` consumes content, then the gateway reports stream readiness through a FIFO. SIGTERM must produce status 143 and `cancelled by SIGTERM`. |
| SIGTERM during Ollama streaming | The same handshake through `--provider ollama`, with exact status 143 and cancellation diagnostic. |
| SIGHUP during oMLX streaming | The same content/readiness handshake, delivered SIGHUP, exact status 129 and `cancelled by SIGHUP`. |
| SIGTERM during an oMLX project command | The real shell tool starts a shell and a blocked descendant. The descendant reports readiness through a separate FIFO; both retain its writer. After SIGTERM, require status 143, the cancellation diagnostic, and EOF with no survivor message. |
| SIGTERM during an Ollama project command | The same command/descendant handshake and cleanup assertions through `--provider ollama`. |
| Inherited SIGHUP-ignore, then SIGTERM | The spawned CLI inherits `SIG_IGN` before exec. After stream readiness, deliver SIGHUP and release another content chunk. A second event is emitted only after that chunk is consumed. Then require SIGTERM status 143 and its cancellation diagnostic. This proves progress after SIGHUP, beyond an immediate liveness check. |

The command descendant blocks on another FIFO. After CLI completion the test
releases that read: any survivor writes `survived` and exits, producing a
behavioral failure rather than leaving the test waiting forever for EOF.
Temporarily replacing the production process-group SIGKILL with signal 0
rejected with `a project process survived cancellation: survived`; restoring
the group kill passed. Discarding inherited SIGHUP-ignore also rejected with
status 129 instead of 143. The production dispatcher, signal registration,
signal-to-cancellation token, tool execution, process-group kill, and two-second
shutdown bound are retained. No timing assertion, polling, sleep, socket,
server, Python process, or local model is used. No timing behavior is asserted,
so these tests need no simulated time. The CI job timeout remains the outer
watchdog for a broken handshake; the tests themselves have no elapsed-time
success or failure criterion.

`src/termination_test_provider.rs` supplies a cancellation-aware `LlmGateway`
inside the spawned process. It reads a command script and uses named pipes for
coordination. It substitutes the model boundary after signal registration;
the existing `cfg(test)` dispatcher driver would bypass that registration.
It reuses the workspace fixture and the existing normalized tool-call shape
and literal `call-shell_command` ID. An explicit context window avoids provider
metadata I/O. The double honors the same cancellation token as the production
gateway, including a request after a cancelled tool. It does not verify
Mojentic's recovery engine, HTTP serialization, SSE/NDJSON decoding, or remote
provider behavior. Both provider selections are exercised against normalized
gateway behavior, with no claim of external provider contract verification.
No cross-repository contract changes are introduced; existing benchmark request
oracles and the terminal-outcome contract remain unchanged.

Readiness delivery is also exercised through `LlmGateway::complete_stream` in
`termination_test_provider::tests`, with an in-process replacement for the pipe
I/O boundary. The regression supplies false writable readiness (`WouldBlock`),
partial writes, and backpressure after partial progress for both `stream-ready`
and `stream-advanced`. Each write must receive exactly the unwritten suffix;
the accumulated bytes must equal `stream-ready\nstream-advanced\n` exactly
once and in order. Pending readiness prevents the next content from reaching
the caller, and the control read checks the complete first event before allowing
continued content. Readiness futures yield via the executor without timers,
real pipes, sleeps, or wall-clock assertions. Separate cases propagate terminal
readiness errors, terminal write errors, and zero-byte writes, including errors
after partial progress on each event. The backpressure case fails against the
single-attempt emitter and passes with readiness waits and suffix retries.
Run `cargo test --locked --lib --features termination-test-provider
termination_test_provider::tests`; the required all-feature test gate includes
these regressions as well as the six unchanged process cases above.

The feature defaults off. `build.rs` permits its hook only in the development
profile, and the module and injection site additionally require that feature
and Unix. Release-profile builds exclude the hook even with `--all-features`
and even if a builder enables release debug assertions. Neither environment
variables nor CLI arguments can activate code absent from the binary. Release
verification builds both normal and all-feature release binaries and checks
that the activation environment-variable name and scripted-provider diagnostics
are absent from both executables. The TUI test gateway uses the same release
exclusion, and its activation names are checked too. Registered Python harnesses
are untouched.

## Executable TUI journeys

`tests/tui.rs` retains all 28 original cases and their names. Characterization
was made from every original assertion before changing the suite. These tests
still launch the actual `microgentic` or `adaptive-agent-harness` executable on
a PTY, send terminal input, render its output through vt100, execute the real
conversation worker and tools, and inspect native state and traces. They do
not substitute direct calls to a renderer for a CLI journey.

The double lives at the public Mojentic `LlmGateway` boundary inside the
executable. `tests/tui_support/gateway.rs` supplies normalized stream chunks over
named pipes; there is no HTTP server, socket, external provider or local model.
Both the child conversation worker and the independent TUI model-listing path
use it, as does model-context discovery. Captured requests contain actual
serialized native messages, descriptors and every completion control, rather
than reconstructed provider wire bodies. A reply pipe belongs to one request:
unread frames from cancellation cannot enter the next exchange. Pending reads
select against runtime cancellation. Blocking pipe setup runs on Tokio's
blocking pool. Metadata lookups record their operation and complete model input.

The following mapping covers every original case. `{omlx,ollama}` denotes the
two retained provider cases; both executable names means all four retained
`[legacy_]tui_streams_and_keeps_native_history_{omlx,ollama}` cases.

| Original case or family | Assertion-to-evidence mapping |
| --- | --- |
| `tui_opens_without_a_provider_and_restores_the_terminal` | Actual welcome and ready label, green/bold marker, colour sequences, complete DEC synchronized frames, active raw mode, clean Esc exit and restored termios remain asserted. Resize to 5×20 now waits for and asserts rendered `世界`, replacing the former 100 ms pause with stronger evidence. Both gateway requests and metadata operations must be empty. |
| `an_empty_no_color_keeps_colour` | Empty `NO_COLOR` still produces colour on the actual terminal, then restores its modes on clean exit. |
| `no_color_removes_colour_but_keeps_labels_and_attributes` | Nonempty `NO_COLOR` removes foreground codes; the ready label retains default foreground and bold attributes; clean exit/restoration remain. |
| `tui_chats_naturally_and_can_perform_work_when_requested_{omlx,ollama}` | Idle startup makes no conversation call. Greetings have no assumed engineering task or evidence footer. An empty reply continues; discussing a quoted command executes nothing. Explicit work writes the exact Rust source, real offline `cargo check` succeeds, its trace records the write/check, project work has an evidence footer, later thanks has none. Native prompts and conversational system policy remain asserted. |
| Both executable names: streamed native history | A long untruncated live reply is visible while a stream gate is closed. Real `read_file` output and Unicode survive into the complete next request. Read-only project work retains its evidence footer. Ctrl-C leaves an error in native state, excludes partial content, and the next request retains the codename and stopped-request notice. Wrong model/project state is rejected by the actual worker before gateway invocation. An ordinary `run` starts with fresh history and identical completion controls; compatibility `run` retains its historical descriptor set. A second TUI starts a separate conversation. |
| `tui_slash_commands_stay_local_{omlx,ollama}` | Actual `/help`, `/guidance`, invalid command/effort, model and effort pickers, keyboard selection/cancel, unchanged-settings notices, clear, default effort, `//` escaping and commands during a held reply retain their rendered assertions. Requests prove model/effort changes, history reset/retention, preserved saved config and retained session files. Full listing operation is recorded; picker asserts both actual model names. Native completion config replaces HTTP-specific effort fields. |
| `tui_shows_each_tool_call_as_one_live_line_{omlx,ollama}` | A real shell waits on a FIFO until both batch tool lines are rendered running; a manually advanced spinner tick produces a different glyph before release. Success, missing-file failure, arithmetic result, green/red cells, one line per call, no residual spinner/Using/finished lines and the evidence footer remain asserted. The command label is `read -r release < tool-control`, matching the FIFO gate. |
| `tui_status_bar_shows_git_state_and_a_known_context_window` | Complete identity/branch/modified/untracked/empty-gauge row and its semantic colours remain exact. Actual ledger token usage determines the displayed percentage, and new untracked work appears after the next exchange. Real Git operations remain. |
| `tui_status_bar_shows_only_what_it_knows_outside_git` | Complete identity-only initial row, colours and unknown-window compact token count remain asserted against the actual trace ledger; no invented branch or capacity. |
| `tui_launch_animation_settles_on_the_resting_screen` | Manual time advances through seed, burst/assembled letters and final welcome. At least six painted frames, seed/wordmark, final resting frame, current resting screen and no provider operations remain asserted. The wall-clock `<2.4s` threshold and 300 ms quiet period are deleted: the injected timeline reaches the final screen at 2000 ms without measuring host performance. |
| `tui_launch_animation_is_skipped_by_a_key_that_still_lands` | At the first observed seed, `x` is delivered to the real terminal. The first frame containing it and every subsequent observed frame remain resting; the exact input row is `> x`. Advancing to 2000 ms and sending `y` preserves the resting screen and `> xy`. No real delay or provider operation. |
| `tui_streams_thinking_live_and_collapses_it_{omlx,ollama}` | Stream gates preserve live multi-chunk thinking before tool execution, dim/italic cells, accumulated text, tool-result evidence and collapse to the final reply; full thinking no longer occupies the screen. |
| `tui_wraps_replies_at_words_and_keeps_indentation_{omlx,ollama}` | Real terminal rows preserve complete word sequences, blank lines, bullets and indentation without wrapping into the input/chrome. Long no-space tokens and explicit breaks retain their original assertions. |
| `tui_context_panel_shows_the_latest_request_{omlx,ollama}` | Ctrl-O and `/context` open/close the real side panel. Empty state, current exchange/request/model, system/tools/prompt components, prior native turns in order, retained/new labels, scrolling, held-request contents and tool-loop updates remain asserted. |
| `tui_shows_the_discovered_window_and_residency_{omlx,ollama}` | Discovered 100k capacity, Ollama's 32768 request cap, pinned root guidance/system/tools/prompt, standing-item count and percentage remain asserted. Exact metadata operation/model plus native state prove the second exchange reuses discovery. Optional absent capacity remains covered by status/history cases. |
| `tui_ends_when_its_terminal_hangs_up` | Close every controlling PTY handle after observing the CLI's input hint. Real SIGHUP/terminal EOF behavior must return exit 1, rather than the forced signal exit 129. No sleep, polling loop or elapsed-time threshold. |
| `tui_ends_on_a_termination_signal_and_restores_the_terminal` | Dispatch real SIGTERM and SIGHUP to the idle CLI, wait for the actual child status, assert success and restored termios. |
| `tui_ends_on_sigterm_while_a_reply_streams` | Observe partial content while held, dispatch real SIGTERM, wait for successful child exit and assert restored termios. Also now runs both providers and checks native state records interruption without retaining partial content. |

The additional `tui_model_listing_errors_leave_the_conversation_usable_both_providers`
case drives `/model` through a gateway error, asserts the exact rendered provider
error and complete listing input, then sends a normal message and verifies the
unchanged model, actual prompt, successful answer and context-discovery input.
This covers the listing boundary independently of the conversation worker.

`tests/fixtures/tui_gateway.json` is the internal regression specification
`microgentic_tui_gateway.v1` (SHA-256
`29c923b7ec46df41c3a20209d15bbd5a017e29ac07a06a8f6b205350ef72a875`). It specifies the entire initial request and
follow-up native history, including literal consolidated-observation ID, read
signature, Unicode content and optional-field omissions. The descriptor list
reuses `microgentic_benchmark_continuity.v1` from
`tests/fixtures/benchmark_continuity.json`, adding the literal conversational
calculator descriptor. All four executable/provider history journeys compare
complete requests with this specification. Expectations are reviewed literals;
identifiers are never masked or copied from the observed request. Stream chunks
are gateway inputs, not a claim of verification of Mojentic's HTTP implementation.

`tests/tui_support/pty.rs` records completed DEC synchronized updates and wakes
waiters with a condition variable. Its timeout is a missing-event watchdog,
never a settling delay or a behavioral wall-clock assertion. Child completion
uses blocking process wait, hangup uses descriptor readiness, and stream/tool
progress uses explicit gates/FIFOs. Launch and spinner time are injected through
a manually advanced clock; ordinary sessions retain their real monotonic clock.
The old `tests/support/{mod,provider,terminal}.rs` files and unused terminal
methods are deleted. `tests/support/fixtures.rs` and its helpers are retained for
`tests/termination.rs` and existing in-process suites. Registered recovery and
progress fixtures and the landed termination tests are unchanged.

Transport-only HTTP routing, SSE/NDJSON encoding, provider JSON effort-field
placement and provider metadata parsing are retired from this suite. Native
provider selection, IDs, completion controls, history and Ollama accounting cap
remain checked. No public application/repository contract changed and no
external provider verification is claimed. The existing versioned benchmark
continuity and Rust/Python terminal-outcome contracts are unchanged and remain
in the required gates. This suite proves executable wiring and the terminal
journey in addition to the existing gateway conformance coverage.

Run the focused journeys with:

```sh
cargo test --locked --features termination-test-provider --test tui
cargo test --locked --lib cli::benchmark_continuity
cargo test --locked --test termination --features termination-test-provider
```

The test feature defaults off. The TUI gateway, clock activation and injection
sites require Unix, the feature and the development-profile cfg from `build.rs`.
They are absent from release builds, including release builds with all features.
The ordinary idle smoke case can still use `MICROGENTIC_TEST_BINARY` to launch a
copied release executable; no gateway activation is needed for that journey.

Required final verification (also defined in `.hone-gates.json`):

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
cargo build --locked --release --bins
python3 -m unittest discover -s benchmarks -p 'test_collection_validation.py'
python3 -m unittest discover -s benchmarks -p 'test_terminal_outcome.py'
mdbook build book
```

Also build `cargo build --locked --release --all-features --bins` and inspect
both binaries for absence of `MICROGENTIC_TUI_SCRIPT`, `MICROGENTIC_TUI_CLOCK`
and `MICROGENTIC_TERMINATION_SCRIPT` to check release exclusion.
