# In-process CLI coverage

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

Remaining campaign work: `tui` and `termination` still use
`tests/support/provider.rs`; their shared server support remains. Registered
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
| Provider-specific HTTP serialization, SSE/NDJSON stream parsing and encoding of tool IDs | Transport-only coverage retired from this suite. Actual normalized LlmGateway messages, IDs, descriptors and completion controls are retained exactly. Mojentic owns HTTP serialization/parsing; no external Mojentic verification is claimed. TUI/termination still retain their existing server. |

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
