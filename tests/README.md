# Outcome coverage

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

All injection sites and both new modules are guarded by `cfg(test)`. Release
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

Remaining campaign work: `everyday_runs`, `run_report`, `tui`, `termination`,
and `benchmark_continuity` still use `tests/support/provider.rs`. Their server
support and coverage remain in place. Benchmark request-continuity assertions,
registered Python recovery/progress harnesses and experiment inputs are unchanged.
This migration does not claim those suites have become in-process tests.
