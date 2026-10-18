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
double before any transport. Stdin is empty and state lives in the temporary
workspace. The test I/O scope supplies the tool root after CLI path validation, so tests
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
No shared end-to-end support became unused: `provider.rs`, `fixtures.rs`,
`terminal.rs` and `mod.rs` retain their existing callers and remain intact.

Remaining campaign work: `everyday_runs`, `tui`, `termination`,
and `benchmark_continuity` still use `tests/support/provider.rs`. Their server
support and coverage remain in place. Benchmark request-continuity assertions,
registered Python recovery/progress harnesses and experiment inputs are unchanged.
This migration does not claim those suites have become in-process tests.
