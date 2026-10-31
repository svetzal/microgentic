# Changelog

<!-- markdownlint-configure-file {"MD024": {"siblings_only": true}} -->

All notable changes to Microgentic are recorded in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
The release workflow publishes each version's section as its GitHub release notes.

## [Unreleased]

### Changed

- Task-mode prompts (benchmark runs and `run --goal`/`run --contract`) no
  longer assume a Rust project built with Cargo. The system prompt and the
  first task message now speak of the project root, project files, a first
  build or full test run, and a validation ladder run with the project's own
  tools: format check, lint, focused tests, then the full test suite.
- Validation-repair prompts in every mode, including everyday `run` and the
  TUI, now name that general ladder instead of `cargo fmt --check` and
  `cargo clippy`.
- Benchmark provider requests change as a result. Earlier benchmark baselines
  are not request-identical to this version; the continuity fixture was
  re-recorded.

## [0.6.0] - 2026-10-18

Microgentic now builds on the published Mojentic 3.0.1 crate and lets it do
two jobs the runtime had kept for itself: discovering a model's context window
and timing the first token. The provider requests, assembled packets and
context ledgers are unchanged from 0.5.0. Benchmark adapters can classify
every run ending, including hard stops raised inside a model stream, from
typed fields in the trace.

### Changed

- The provider window query moved into Mojentic. `ContextWindow::discover`
  asks the provider gateway's `get_model_context_window` (Mojentic 3.0.1 or
  later, which reports an Ollama model whose only window is its `num_ctx`
  parameter) and `src/model_metadata.rs` is gone. The trace's
  `context.window.resolved` record keeps its `tokens` and `field`: the model's
  `num_ctx` parameter with field `parameters.num_ctx` when the model has one,
  otherwise the advertised maximum with Mojentic's field path
  (`max_model_len` on oMLX, `model_info.<arch>.context_length` on Ollama).
  Only `discovery_error` wording changes, to Mojentic's gateway error text.
- The first-token clock moved into Mojentic. `llm.context_assembly.response`
  takes `time_to_first_token_ms`, `first_token_kind` and
  `request_duration_ms` from the client timing on the completion's terminal
  `StreamMetrics`, and `src/first_token.rs` is gone. The recovery stream shim
  forwards the completion report's metrics, which carry that timing, as the
  one `Metrics` chunk per completion, as Mojentic's own adapter does. The
  three fields are `null` when a stream ended before its terminal metrics,
  such as a stream Microgentic interrupted; `TRACE_SCHEMA.md` describes the
  measurement.
- Microgentic now depends on the published Mojentic 3.0 crate from crates.io
  instead of a pinned Git revision, so dependency maintenance can see when it
  falls behind. Mojentic 3.0 sends a request control only when the caller sets
  it, so Microgentic now sets temperature (1.0), the Ollama context size
  (32768 when no window is chosen) and the output limit (16384 when none is
  chosen) explicitly. They are the values every earlier release sent. The
  benchmark continuity test confirms provider requests, assembled packets and
  context ledgers equal to 0.5.0 in benchmark and conversation modes on both
  providers. Source builds no longer need Git access to Mojentic.

### Added

- The trace's `run.failed` record carries `outcome` when a hard stop raised
  inside a model stream ended the run (an inspection loop or a no-content
  stream), so benchmark adapters classify every run ending from typed
  fields. It is `null` for provider and environment errors. `TRACE_SCHEMA.md`
  gains a section on the terminal events. The live benchmark classifier,
  `benchmarks/terminal_outcome.py`, reads `outcome` from `run.finished` and
  `run.failed` and falls back to its earlier rules for traces without it. A
  withheld completion now gets its own `withheld_<gate>` label; the text rules
  had called it `fail` or `outer_turn_limit`. One contract file,
  `benchmarks/fixtures/terminal_outcome_contract.json`, lists each outcome and
  its label: the end-to-end tests check that the binary writes it, and the
  classifier's tests, now a required gate and a CI step, check the labels.
  The Pier and Harbor adapters' matching change is recorded as a patch with
  file hashes in `benchmarks/cli-migration/structured-outcome-*`.

## [0.5.0] - 2026-10-17

Every run now says how it ended and what it did. The runtime decides a typed
outcome, `microgentic run` exits with it, and the summary lists the files the
run changed and the checks it ran. Traces gain the model's discovered context
window, time to first token, and context residency. The conversation ends with
its terminal and on `kill`. Benchmark mode's provider requests are unchanged.

### Added

- One typed outcome for every run, decided by the runtime rather than by the
  model's prose: `replied`, `done`, `failed`, `withheld` (by the Git stash,
  validation, probe, or acceptance-ledger gate), `hard_stop` (action boundary,
  thinking only, empty response, repair without action, repair depth,
  inspection loop, no-content stream, or iteration budget), `cancelled`,
  `error`, and `usage`. The run summary gains an `outcome` field, in `--json`,
  in benchmark JSON, and in the `run.finished` trace event; the existing
  fields are unchanged. Each attempt record of `run-sequential` carries the
  same field. When a `run` does not succeed, stderr gets one `microgentic:`
  line naming the outcome and the trace, and a conversation exchange that
  ends without a completed reply shows the same line and a `Stopped` status.
  The two stops raised inside a model stream (inspection loop, no-content
  stream) still end the run as an error, and their `run.failed` trace
  record's `error` text now opens with the outcome line before the
  original summary.
- Every run reports what it changed and what was checked. The summary gains
  `changes` (each file created, modified, or deleted compared with the project
  at the start of the run, whether by a tool or a shell command, with the
  paths that already had uncommitted Git edits before the run) and `checks`
  (declared probes and requested validation commands as passed, failed, not
  run, or stale; the shell commands the model chose to run, with their exit
  status, kept apart because they do not accept work; and source files changed
  after the last passing check). Plain `run` writes the same report to stderr
  as `microgentic: changed …`, `checks: …`, `commands run: …`, and
  `unchecked: …` lines, and the conversation view shows them before the
  `Evidence:` line. A run that changed nothing and ran no checks adds no
  lines. The trace gains a `run.workspace.baseline` event. Prompts, provider
  requests, gates, and exit codes are unchanged.
- Everyday `run` and the conversation ask the provider for the selected
  model's context window: oMLX's `max_model_len` from `/v1/models`, and
  Ollama's `num_ctx` parameter or `<arch>.context_length` from `/api/show`.
  An explicit `--context-window-tokens` comes first and the built-in model
  table last. The trace records the window and its source in
  `context.window.resolved`, and each context ledger gains `known_window`
  with utilization and pressure against it. The conversation asks once and
  keeps the answer for later exchanges. A discovered window is used for
  accounting only: Ollama's `num_ctx` and the run policies that read the
  window are unchanged. On Ollama the accounting window is at most the
  `num_ctx` each request already sends, because Ollama runs the model with
  it. Benchmark mode makes no discovery request.
- Everyday `run` and the conversation record context residency in the
  trace: each ledger lists every item in the request (base prompt, tool
  descriptors, guidance, conversation turns, tool results, file observations)
  with its kind, owner, reason, tokens, position, activation turn, last
  reference, and whether it is pinned or demoted, plus standing guidance
  tokens against the 15% standing-context limit. `context.residency.*`
  events mark guidance activation, nested guidance references, and each
  existing compaction as a demotion. This is measurement only: nothing sent
  to the model changes, no limit is enforced, and benchmark traces are
  unchanged.
- Advanced settings `standing_context_limit` and `guidance_fraction` in
  `config.json`, with the run flags `--standing-context-limit` and
  `--guidance-fraction`. A flag overrides the file, and the file overrides
  the defaults of 0.15 and 0.125. Traces record the effective values.
- Each `llm.context_assembly.response` trace event records the request's time
  to first token, the kind of that token (content, thinking, or tool call),
  and its total duration, in every mode.

### Changed

- `microgentic run` exits with its outcome: 0 for a completed reply or an
  accepted `DONE`, 3 for a returned `FAIL`, 4 for a withheld completion, 5
  for a runtime stop, 1 for an error, 2 for a usage error, and 128 plus the
  signal number when a termination signal ends the run (130, 143, 129), the
  same status the forced exit already used. The run's own usage errors (an
  empty or missing prompt, a `--dir` that is not a directory) now exit 2
  rather than 1. `benchmark run`, `benchmark run-sequential`,
  `run-sequential`, and the `adaptive-agent-harness` executable keep exit 0
  on a summary and 1 on an error.
- The context panel shows each element's inclusion reason and residency
  state (pinned, the turn that added it, the last reference, demoted), and
  marks the tools as pinned. With a known window, the panel header and the
  status gauge use it instead of "window unknown".
- The status bar's `📜` count is now the standing items in the latest
  request, read from the worker's residency record; today these are the
  guidance files. It still sits beside the gauge and drops first.
- The conversation view draws each frame as one synchronized update (DEC mode
  2026). Terminals that support it no longer show a half-drawn frame; others
  ignore it.

### Fixed

- Runs that start within the same second in one trace directory no longer
  collide. Trace file names were stamped to the second and created
  exclusively, so the second run failed before any work, which hit rapid
  `run-sequential` and benchmark attempts and scripted loops of `run`. Names
  now carry nanoseconds (`run-20261017T093005.123456789Z.jsonl`), and a run
  that still finds its name taken adds `_01`, `_02`, and so on. Creation stays
  exclusive, so no trace is ever overwritten, and nothing waits. Names still
  sort in start order, which transcripts rely on.
- The conversation view ends when its terminal goes away. crossterm's input
  reader never returns once standard input is at end of file or in error, so
  a conversation whose terminal emulator quit, or whose parent process exited,
  spun on one core until it was killed. The event loop now waits for input
  with `poll(2)`, which reports the hang-up without reading, and exits with an
  error. An idle screen also wakes five times a second instead of twenty.
- `kill` ends the conversation view and `microgentic run`. Both caught SIGINT
  and SIGTERM to cancel their work, which removed the signal's default effect,
  so a process whose work did not notice the cancellation (the conversation
  view stuck in the reader above, or a run inside a project command) lived on.
  SIGINT, SIGTERM and SIGHUP now cancel the model request and kill a running
  project command with its process group. If the process is still alive two
  seconds later, or a second signal arrives, it restores the terminal and
  exits with the signal's conventional status; recovery evidence is not
  settled on that path. A run started with SIGHUP ignored, as under `nohup`,
  keeps ignoring it.

## [0.4.0] - 2026-10-15

Everyday `microgentic run` now behaves like the conversation: it ends on the
model's first visible answer. The conversation view gains a context panel.
Benchmark mode and task runs with a contract, goal, or experiment are
unchanged; their provider requests are identical to 0.3.0.

### Added

- A context panel in the conversation view. Ctrl-O or `/context` opens it to
  the right of the conversation. It lists every message and tool descriptor in
  the latest model request, in order, with its kind, an estimated token count,
  and its first lines. It says whether the model's earlier thinking was sent
  (by default it is not) and how many older exchanges were left out. It marks
  summarized and consolidated tool output as not verbatim, and marks each
  message `new` or `kept` against the previous request. It updates with every
  request, including tool loops. Alt-Page Up/Down scroll it. Provider requests
  are unchanged.
- A user guide, published at vetzal.ca/microgentic. It covers install, setup,
  the conversation, shell use, providers, project guidance, tools, commands,
  file locations, traces, and benchmark mode.

### Changed

- An everyday `microgentic run` sends your prompt as written and finishes on
  the first visible reply. It no longer wraps the prompt in the benchmark task
  template or asks the model to continue after it has answered. A plain
  question now takes one model request instead of ten. Tools still work, and a
  request for work still does the work.
- Shell code blocks in an everyday `--prompt` are treated as part of the
  discussion, as in the conversation. They no longer become checks the run must
  pass. Task runs keep the previous behaviour.
- Plain `run` output has no blank lines around the reply. `--json` output is
  unchanged.
- The launch animation plays in about 1.7 seconds.

## [0.3.0] - 2026-10-15

The agent gets a calculator and reads project guidance, and the conversation
view is easier to read. Benchmark mode is unchanged: it offers no calculator and
reads no project guidance, and its provider requests are identical to 0.2.0.

### Added

- Project guidance. Microgentic reads `AGENTS.md` files from the project
  directory up to the Git root, with `CLAUDE.md` as a fallback where a directory
  has no `AGENTS.md`. A personal file at `~/.config/microgentic/AGENTS.md`
  applies everywhere. Deeper files win when guidance conflicts. A nested file
  below the project directory is delivered when the agent first works in its
  directory. Guidance cannot grant permissions or change the available tools.
  `microgentic run --no-project-guidance` turns discovery off.
- `/guidance` shows what guidance loaded, and the status bar shows a 📜 count.
- A `calculate` tool evaluates arithmetic exactly, so the model does not have
  to work sums out in its head. It accepts batches, and a line can name a value
  for later lines. The conversation view shows each result on the tool line.

### Changed

- Text wraps at words and no row starts with a stray space. Indentation and
  list items keep their shape. Tokens too long for a row are split at a natural
  break or shortened in the middle with `…`. The Evidence path is never
  shortened.
- Reply text that follows thinking or tool lines has a blank row above it.
- The launch animation plays in about 1.3 seconds.

## [0.2.0] - 2026-10-15

The conversation view is easier to read and to steer while a small model works.
Benchmark mode, `microgentic run`, and trace records are unchanged.

### Added

- Colour that follows the terminal theme. Set `NO_COLOR` to turn it off; labels
  and glyphs keep their meaning without it.
- Local slash commands that never reach the model. `/model` and `/effort` open
  arrow-key pickers or take a value directly, `/clear` starts a new
  conversation, and `/help` lists the commands. The footer suggests matching
  commands as you type, and Tab completes them. Changes from `/model` and
  `/effort` last for the session; `microgentic setup` changes the defaults.
- One line per tool call, with a short summary of its parameters such as the
  file and line range it read. A spinner marks a running call, then a check or
  a cross shows its result.
- Streamed thinking in dim italics. When the model finishes, thinking that led
  to tool calls shrinks to its last line above those calls, and thinking before
  a reply is removed.
- A status bar with the model, reasoning effort, project folder, Git branch and
  changes, and how full the context window is. The context figure is the
  runtime's own estimate; without a known window size it shows a token count.
- A short launch animation. Any key skips it, and the key still reaches the
  input. Set `MICROGENTIC_NO_ANIMATION=1` to turn it off.

### Changed

- Changing the model or effort after the first message starts a new
  conversation, because a conversation keeps the model and effort it began with.

### Fixed

- Whitespace-only output from the model no longer opens blank rows.

## [0.1.0] - 2026-10-13

The first public package. Microgentic is an adaptive coding agent optimized for
small models that need help staying on task, and this release brings the
experimental harness into a small terminal interface for everyday use.

### Added

- An ongoing, streamed conversation with `microgentic`.
- `microgentic setup` configures oMLX or Ollama, a default model, and reasoning
  effort.
- Prompts through stdin or `--prompt`, with results and diagnostics redirected
  separately.
- Project inspection, editing, and checks through the existing scoped runtime.
- `microgentic benchmark run` keeps benchmark tasks isolated.
- The native trace analyzer and transcript renderer for preserved traces.
- Packages for Apple Silicon, Intel Mac, and Linux x86-64, with checksums,
  binary manifests, and embedded source revisions. `mg` remains an optional
  shell alias, and the `adaptive-agent-harness` compatibility executable remains
  available.

### Known limits

- Work permissions and approval previews, durable session resume, additional
  gateways, and richer tool inspection are not built yet.
- The runtime can edit files and run commands when you request work.
- Provider credentials and server URLs stay in their existing environment
  variables.
- A successful process exit still needs inspection of the native terminal
  result when classifying benchmark outcomes.

[Unreleased]: https://github.com/svetzal/microgentic/compare/v0.6.0...HEAD
[0.6.0]: https://github.com/svetzal/microgentic/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/svetzal/microgentic/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/svetzal/microgentic/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/svetzal/microgentic/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/svetzal/microgentic/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/svetzal/microgentic/releases/tag/v0.1.0
