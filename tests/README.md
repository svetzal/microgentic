# In-process CLI coverage

## Gateway request migration

`src/cli/gateway_requests.rs` drives the real argument parser, dispatcher and
public `run_agent` through the existing task-scoped `LlmGateway` double.
`src/semantic_advisory.rs` exercises public `request_semantic_advisory` with its
in-process structured-completion gateway. Neither replacement opens sockets,
starts a provider, invokes Python, sleeps, or asserts wall-clock timing. The
remaining `tests/omlx_gateway.rs` case is intentionally socket-free serialization
coverage. The private server, HTTP reader, metadata polling, response encoders
and process launch helpers from that file have been removed; shared support in
`tests/support/provider.rs`, `terminal.rs` and `fixtures.rs` remains in use.

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
claiming to verify Mojentic's HTTP transport. The remaining `omlx_gateway`
integration cases still need `mock_server` and `run_cli_options`; both helpers
remain. No external application contract changed.

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
The shared end-to-end modules `provider.rs`, `fixtures.rs`, `terminal.rs`
and `mod.rs` retain callers in the remaining suites.

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
stream, fixture and terminal support retain their callers.

No application/repository boundary, contract version, dependency, feature or
public API changes. The existing executable outcome contract remains
`microgentic_terminal_outcome_contract.v1`, SHA-256
`6c8afe8fe658944da93d4b8f582d74e20a16d8ef59b1dfa3f128592df1183f37`.
Both the Rust producer suite and Python consumer suite verify it in this
repository's required gates. No external provider verification is claimed.

Remaining campaign work: `tui` still uses `tests/support/provider.rs` and
`tests/support/terminal.rs`; their shared server and terminal support remains. Registered
Python recovery/progress harnesses and experiment inputs are unchanged.

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
| Provider-specific HTTP serialization, SSE/NDJSON stream parsing and encoding of tool IDs | Transport-only coverage retired from this suite. Actual normalized LlmGateway messages, IDs, descriptors and completion controls are retained exactly. Mojentic owns HTTP serialization/parsing; no external Mojentic verification is claimed. TUI still retains its existing server; termination uses the scripted gateway below. |

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

The feature defaults off. `build.rs` permits its hook only in the development
profile, and the module and injection site additionally require that feature
and Unix. Release-profile builds exclude the hook even with `--all-features`
and even if a builder enables release debug assertions. Neither environment
variables nor CLI arguments can activate code absent from the binary. Release
verification builds both normal and all-feature release binaries and checks
that the activation environment-variable name and scripted-provider diagnostics
are absent from both executables. TUI's shared provider and terminal helpers
remain intact; registered Python harnesses are untouched.
